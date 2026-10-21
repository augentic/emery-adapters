//! Display copy and test identifiers, lifted out of the composition into the UI contract.
//!
//! Every user-visible string in a composition is authored in one place — the Figma file — and
//! ends up in three shells. Emitting it as a literal in each of them means three copies of the
//! same words, none of which a translator can find, and a Maestro identifier spelled by hand in
//! both the test and the view.
//!
//! So the strings come out first. `ui-contract/ui-strings.yaml` and `ui-contract/test-ids.yaml`
//! are generated from the surface, the template's own `generate-bindings` turns them into
//! `UiStrings` and `MaestroTestIds` for each shell, and the view emitters reference those
//! constants rather than repeating the words. The contract is a projection of the composition
//! rather than a document anybody maintains, which is what makes it safe to overwrite.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{A11y, Arg, Body, Component, Header, Item, Node, Overlay, Screen, Surface};
use crate::naming::upper_snake;
use crate::{GeneratedFile, HEADER};

/// Where the strings contract lives.
pub const STRINGS: &str = "ui-contract/ui-strings.yaml";

/// Where the test-id contract lives.
pub const TEST_IDS: &str = "ui-contract/test-ids.yaml";

/// The longest key a literal is allowed to produce, before disambiguation.
///
/// A key is read in a diff and typed in a Maestro flow, and the words past the first handful
/// stop distinguishing anything. Truncation can collide, which is what the numeric suffix is for.
const LONGEST: usize = 40;

/// The copy a composition asks for, keyed the way the shells will spell it.
#[derive(Debug, Clone, Default)]
pub struct Contract {
    /// Literal to its `UiStrings` constant.
    strings: BTreeMap<String, String>,

    /// Kebab-case test id to its `MaestroTestIds` constant.
    test_ids: BTreeMap<String, String>,
}

impl Contract {
    /// Collects every literal and test id the surface declares.
    ///
    /// The walk is in render order — a screen's header before its body, screens before the
    /// components they call — so that two runs over the same composition assign the same keys.
    #[must_use]
    pub fn derive(surface: &Surface) -> Self {
        let mut contract = Self::default();

        for screen in &surface.screens {
            contract.screen(screen);
        }
        for component in &surface.components {
            contract.component(component);
        }
        // Last, so that adding a menu to a composition cannot renumber a key the screens already
        // had. Each tab's label is the same in both looks, so one look is all there is to walk.
        for tab in surface.footer_menu.iter().flat_map(|menu| &menu.tabs) {
            contract.nodes(&[Node::Group(Box::new(tab.unselected.clone()))]);
        }
        contract
    }

    /// The `UiStrings` constant for a literal, when the contract holds one.
    #[must_use]
    pub fn string(&self, literal: &str) -> Option<&str> {
        self.strings.get(literal).map(String::as_str)
    }

    /// Every literal and the constant it is keyed by, in literal order.
    pub fn strings(&self) -> impl Iterator<Item = (&str, &str)> {
        self.strings.iter().map(|(literal, key)| (literal.as_str(), key.as_str()))
    }

    /// The `MaestroTestIds` constant for a test id, when the contract holds one.
    #[must_use]
    pub fn test_id(&self, id: &str) -> Option<&str> {
        self.test_ids.get(id).map(String::as_str)
    }

    /// Whether the composition declares any copy at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty() && self.test_ids.is_empty()
    }

    /// The two contract documents.
    ///
    /// Both are emitted even when empty. The template's binding generator reads them
    /// unconditionally, and an absent file is a build failure where an empty mapping is a project
    /// that has no copy yet.
    #[must_use]
    pub fn files(&self) -> Vec<GeneratedFile> {
        vec![
            GeneratedFile::new(STRINGS, document("strings", &self.strings)),
            GeneratedFile::new(TEST_IDS, document("test_ids", &self.test_ids)),
        ]
    }

    /// A bar's literal title and everything in its rows. A bound title is a field, not copy.
    fn header(&mut self, header: &Header) {
        self.text(header.copy());
        for row in [&header.leading, &header.trailing, &header.below] {
            self.nodes(row);
        }
    }

    fn screen(&mut self, screen: &Screen) {
        if let Some(header) = &screen.header {
            self.header(header);
        }
        if let Some(body) = &screen.body {
            self.body(body);
        }
        self.nodes(&screen.footer);
        if let Some(fab) = &screen.fab {
            self.item(fab);
        }

        for state in &screen.states {
            if let Some(header) = &state.header {
                self.header(header);
            }
            if let Some(body) = &state.body {
                self.body(body);
            }
            self.nodes(&state.footer);
            if let Some(fab) = &state.fab {
                self.item(fab);
            }
        }

        for overlay in &screen.overlays {
            self.overlay(overlay);
        }
    }

    fn component(&mut self, component: &Component) {
        // The lifted tree is its first placement's, `args:` and all, minus the `component:` that
        // would have said so — so what that placement formatted is passed in by hand.
        let formatted = component.group.formats.keys().map(String::as_str).collect();
        self.nodes_within(&[Node::Group(Box::new(component.group.clone()))], &formatted);
    }

    fn overlay(&mut self, overlay: &Overlay) {
        self.text(overlay.title.as_deref());
        self.nodes(&overlay.content);
    }

    fn body(&mut self, body: &Body) {
        self.nodes(body.nodes());
    }

    fn nodes(&mut self, nodes: &[Node]) {
        self.nodes_within(nodes, &BTreeSet::new());
    }

    /// `formatted` is the properties the enclosing placement passes a typed display value, whose
    /// drawn words are a sample of a value the view formats, not copy anything shows.
    fn nodes_within(&mut self, nodes: &[Node], formatted: &BTreeSet<&str>) {
        for node in nodes {
            match node {
                Node::Group(group) => {
                    if let Some(interaction) = &group.interaction {
                        for control in &interaction.controls {
                            self.text(control.a11y_action.as_deref());
                        }
                        for swipe in
                            interaction.swipe_leading.iter().chain(&interaction.swipe_trailing)
                        {
                            self.text(swipe.label.as_deref());
                        }
                    }
                    self.a11y(&group.a11y);
                    // A placement's literal arguments are copy it shows, even when its drawn tree
                    // (an instance compiled from its definition) holds the default words instead.
                    for arg in group.args.values() {
                        if let Arg::Literal(words) = arg {
                            self.text(Some(words));
                        }
                    }
                    // A placement decides for the properties it passes; a plain group passes on what
                    // its placement decided.
                    let inner: BTreeSet<&str> = if group.component.is_some() {
                        group.formats.keys().map(String::as_str).collect()
                    } else {
                        formatted.clone()
                    };
                    self.nodes_within(&group.items, &inner);
                }
                Node::Item(item) => {
                    // A formatted value's drawn words are a sample of a typed value — a weekday, a
                    // unit name — which the view formats with the device locale. Kept, they
                    // would be the one value the designer happened to draw, as a string nothing
                    // reads.
                    let sample = item.format.is_some()
                        || item.arg.as_deref().is_some_and(|arg| formatted.contains(arg));
                    if !sample {
                        self.text(item.content.as_deref());
                    }
                    self.item_rest(item);
                }
                Node::Collection(collection) => self.nodes_within(&collection.item, formatted),
            }
        }
    }

    fn item(&mut self, item: &Item) {
        self.text(item.content.as_deref());
        self.item_rest(item);
    }

    /// Everything an item says but its words.
    fn item_rest(&mut self, item: &Item) {
        self.text(item.placeholder.as_deref());
        for option in &item.options {
            self.text(Some(option));
        }
        self.a11y(&item.a11y);
    }

    fn a11y(&mut self, a11y: &A11y) {
        self.text(a11y.label.as_deref());
        self.text(a11y.hint.as_deref());

        if let Some(id) = &a11y.test_id
            && !self.test_ids.contains_key(id)
        {
            let key = unique(&format!("MAESTRO_{}", upper_snake(id)), &self.test_ids);
            self.test_ids.insert(id.clone(), key);
        }
    }

    fn text(&mut self, literal: Option<&str>) {
        let Some(literal) = literal.filter(|text| !text.trim().is_empty()) else {
            return;
        };
        if self.strings.contains_key(literal) {
            return;
        }

        let key = unique(&constant(literal), &self.strings);
        self.strings.insert(literal.to_owned(), key);
    }
}

/// A `SCREAMING_SNAKE` constant name for a literal.
///
/// The words are taken as written and upper-cased whole, rather than run through the identifier
/// casing: this is prose, not a symbol, and reading `FRI` as three words is what happens when you
/// look for camel-case boundaries in something that never had any.
///
/// Only letters and digits survive. A literal that is entirely punctuation — an ellipsis, an
/// arrow — has no words to name it, so it falls back to something that says as much.
fn constant(literal: &str) -> String {
    let spaced: String = literal
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { ' ' })
        .collect();

    let words: Vec<String> = spaced.split_whitespace().map(str::to_ascii_uppercase).collect();
    if words.is_empty() {
        return "SYMBOL".to_owned();
    }

    // Whole words only. A key cut mid-word reads as a typo, and the reader cannot tell which word
    // it was without going back to the value.
    let mut key = String::new();
    for word in words {
        if !key.is_empty() && key.len() + 1 + word.len() > LONGEST {
            break;
        }
        if !key.is_empty() {
            key.push('_');
        }
        key.push_str(&word);
    }

    // A key has to start with a letter, and a literal that opens with a digit would produce one
    // that does not.
    if key.starts_with(|character: char| character.is_ascii_digit()) {
        key.insert_str(0, "N_");
    }
    key
}

/// `key`, or the first `key_2`, `key_3`… that nothing has taken.
///
/// Bounded by how many entries there are: with `n` keys already assigned, one of the first `n + 2`
/// candidates is necessarily free.
fn unique(key: &str, taken: &BTreeMap<String, String>) -> String {
    let free = |candidate: &String| !taken.values().any(|existing| existing == candidate);

    (1..=taken.len() + 2)
        .map(|nth| if nth == 1 { key.to_owned() } else { format!("{key}_{nth}") })
        .find(free)
        .unwrap_or_else(|| key.to_owned())
}

/// One contract document: a comment, the section key, and the entries by constant name.
fn document(section: &str, entries: &BTreeMap<String, String>) -> String {
    use std::fmt::Write as _;

    let mut yaml = format!("# {HEADER}\n{section}:");
    if entries.is_empty() {
        yaml.push_str(" {}\n");
        return yaml;
    }
    yaml.push('\n');

    // The file is keyed by constant name, which is the order a reader scans it in, but the map is
    // keyed by value. Inverting here rather than storing both keeps one source of truth.
    let mut by_key: Vec<(&str, &str)> =
        entries.iter().map(|(value, key)| (key.as_str(), value.as_str())).collect();
    by_key.sort_unstable();

    for (key, value) in by_key {
        let _ = writeln!(yaml, "  {key}: {}", quoted(value));
    }
    yaml
}

/// A YAML scalar that cannot be mistaken for anything else.
///
/// Always quoted. Deciding when a plain scalar is safe means reproducing the YAML resolver, and
/// getting it wrong turns `NO` into `false` and `1.0` into a number somewhere downstream.
fn quoted(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model;

    fn contract(document: &serde_json::Value) -> Contract {
        Contract::derive(&model::parse(document).expect("the document parses"))
    }

    #[test]
    fn a_literal_becomes_a_constant_named_for_its_words() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "task-list": {
                    "name": "TaskList",
                    "header": { "title": "Your lists" },
                    "body": [{ "button": { "content": "Add task" } }]
                }
            }
        }));

        assert_eq!(contract.string("Your lists"), Some("YOUR_LISTS"));
        assert_eq!(contract.string("Add task"), Some("ADD_TASK"));
        assert_eq!(contract.string("Never authored"), None);
    }

    #[test]
    fn the_same_words_in_two_places_are_one_constant() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": { "name": "A", "body": [{ "button": { "content": "Save" } }] },
                "b": { "name": "B", "body": [{ "button": { "content": "Save" } }] }
            }
        }));

        assert_eq!(contract.files()[0].body.matches("SAVE").count(), 1);
    }

    #[test]
    fn two_literals_that_would_share_a_name_are_told_apart() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": {
                    "name": "A",
                    "body": [
                        { "text": { "content": "Save" } },
                        { "text": { "content": "Save!" } }
                    ]
                }
            }
        }));

        assert_eq!(contract.string("Save"), Some("SAVE"));
        assert_eq!(contract.string("Save!"), Some("SAVE_2"));
    }

    #[test]
    fn a_test_id_keeps_the_prefix_the_template_requires() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": {
                    "name": "A",
                    "body": [{ "button": { "content": "Add", "test_id": "add-task" } }]
                }
            }
        }));

        assert_eq!(contract.test_id("add-task"), Some("MAESTRO_ADD_TASK"));
    }

    #[test]
    fn a_swipe_label_and_a_custom_action_are_copy_like_anything_else() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": {
                    "name": "A",
                    "body": [{ "group": {
                        "items": [],
                        "interaction": {
                            "archetype": "swipe-row",
                            "controls": [{ "ref": "box", "event": "Toggle", "a11y_action": "Mark done" }],
                            "swipe_trailing": [{ "event": "Delete", "label": "Delete" }]
                        }
                    }}]
                }
            }
        }));

        assert_eq!(contract.string("Mark done"), Some("MARK_DONE"));
        assert_eq!(contract.string("Delete"), Some("DELETE"));
    }

    #[test]
    fn a_composition_with_no_copy_still_emits_both_documents() {
        let files = Contract::default().files();

        assert_eq!(files[0].path, STRINGS);
        assert!(files[0].body.ends_with("strings: {}\n"), "{}", files[0].body);
        assert!(files[1].body.ends_with("test_ids: {}\n"), "{}", files[1].body);
    }

    #[test]
    fn a_document_is_ordered_by_constant_and_quotes_every_value() {
        let contract = contract(&serde_json::json!({
            "screens": {
                "a": {
                    "name": "A",
                    "body": [
                        { "text": { "content": "Zebra" } },
                        { "text": { "content": "Say \"hello\"" } }
                    ]
                }
            }
        }));

        assert_eq!(
            contract.files()[0].body,
            format!(
                "# {HEADER}\nstrings:\n  SAY_HELLO: \"Say \\\"hello\\\"\"\n  ZEBRA: \"Zebra\"\n"
            ),
        );
    }

    #[test]
    fn a_literal_with_no_words_in_it_still_gets_a_name() {
        assert_eq!(constant("…"), "SYMBOL");
        assert_eq!(constant("3 items"), "N_3_ITEMS");
    }

    #[test]
    fn a_long_literal_is_cut_at_a_word_rather_than_through_one() {
        assert_eq!(
            constant("The quick brown fox jumps over the lazy dog"),
            "THE_QUICK_BROWN_FOX_JUMPS_OVER_THE_LAZY",
        );
    }

    #[test]
    fn a_word_that_was_already_shouting_stays_one_word() {
        assert_eq!(constant("FRI"), "FRI", "not F_R_I");
        assert_eq!(constant("My Lists"), "MY_LISTS");
    }

    #[test]
    fn one_word_longer_than_the_limit_is_still_a_key() {
        assert_eq!(constant("Antidisestablishmentarianismisverylongindeed").len(), 44);
    }
}
