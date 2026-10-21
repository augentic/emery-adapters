//! How many arguments an event carries, where the composition says how many it must.
//!
//! The argument an event carries from a control is a fact about the control. Two such facts are
//! mechanical, and each is checked here:
//!
//! - A control that produces a value sends it. The contract reads an argument that names no field
//!   as *the value the control just produced*, so `QueryChanged(value)` on a `search-field` is how
//!   the typed text reaches the app, and a bare `QueryChanged` is a search box whose text never
//!   does.
//! - A row of a `list` or `grid` is one of many, so what it raises has to say which. A bare
//!   `CityOpened` on a row template opens no city in particular.
//!
//! Each message spells out the argument it expects. One event name carrying one number of
//! arguments wherever it is raised is a fact about the document rather than one site:
//! [`crate::contract::derive`] reports a disagreement as a `Clash` and [`crate::design::vue`]
//! refuses it.
//!
//! A hole (`TODO(event)`) is not checked: it has no arity yet, and it is its own finding.

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// Item types whose interaction produces a value, which their `event` carries.
///
/// A subset of [`super::interaction::CONTROL_ITEM_TYPES`]; the unit tests below pin it so.
/// `switch` and `checkbox` are absent on purpose. A two-state control can raise a flip, whose new
/// value is the negation of the one the app already holds, and the row it sits in raises the same
/// event without being able to produce a value at all. Requiring a flag on the switch would split
/// one event into two arities, which the contract rejects.
pub const VALUE_ITEM_TYPES: [&str; 9] = [
    "date-picker",
    "picker",
    "radio",
    "search-field",
    "segmented-control",
    "slider",
    "stepper",
    "text-area",
    "text-field",
];

/// What the value a control produces is, in the words a message uses.
fn produces(item_type: &str) -> &'static str {
    match item_type {
        "text-field" | "search-field" | "text-area" => "the text typed into it",
        "picker" | "segmented-control" | "radio" => "the option chosen",
        "slider" | "stepper" => "the number it was set to",
        "date-picker" => "the date picked",
        _ => "the value it produced",
    }
}

/// Where an event sits, which decides what it must carry.
#[derive(Clone, Copy)]
enum Site<'a> {
    /// An `event:` on an item of this type.
    Item(&'a str),
    /// A container's own `event:` or `interaction.event`.
    Container,
    /// An `interaction.controls[]` entry, naming a control by `ref`.
    NamedControl,
    /// An `interaction.swipe_leading[]` / `swipe_trailing[]` entry.
    Swipe,
}

impl Site<'_> {
    fn describe(self) -> String {
        match self {
            Self::Item(item_type) => format!("the `{item_type}`"),
            Self::Container => "the container's own tap".to_string(),
            Self::NamedControl => "a control named under `interaction.controls`".to_string(),
            Self::Swipe => "a swipe action".to_string(),
        }
    }
}

/// Check every event in the document.
pub fn check_arguments(composition: &Value, errors: &mut Vec<Finding>) {
    for (root, path) in super::items::region_roots(composition) {
        // A body that is a collection arrives already unwrapped to its row template.
        let row = ["/list/item", "/grid/item"]
            .iter()
            .find(|suffix| path.ends_with(*suffix))
            .map(|suffix| collection(composition, &path[..path.len() - "/item".len()], suffix));
        walk(root, &path, row.as_deref(), errors);
    }
}

/// The collection a row template iterates, as a message names it.
fn collection(composition: &Value, pattern_path: &str, suffix: &str) -> String {
    let pattern = if suffix.starts_with("/grid") { "grid" } else { "list" };
    let each = composition
        .pointer(pattern_path)
        .and_then(|entry| entry.get("each"))
        .and_then(Value::as_str);
    named(pattern, each)
}

/// A collection as a message names it: by the field it iterates, once that is known.
fn named(pattern: &str, each: Option<&str>) -> String {
    match each {
        Some(each) if crate::spec::holes::Kind::of(each).is_none() => {
            format!("the `{pattern}` over `{each}`")
        }
        _ => format!("a `{pattern}`"),
    }
}

/// `row` names the collection when this is inside a row template.
fn walk(node: &Value, path: &str, row: Option<&str>, errors: &mut Vec<Finding>) {
    match node {
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{path}/{index}"), row, errors);
            }
        }
        Value::Object(map) if map.len() == 1 => {
            let Some((key, props)) = map.iter().next() else {
                return;
            };
            let child = format!("{path}/{}", escape_pointer_token(key));
            match key.as_str() {
                "list" | "grid" => {
                    if let Some(items) = props.get("item") {
                        let row = named(key, props.get("each").and_then(Value::as_str));
                        walk(items, &format!("{child}/item"), Some(&row), errors);
                    }
                }
                "form" => walk(props, &child, row, errors),
                "group" => {
                    check_event(props, &child, Site::Container, row, errors);
                    check_interaction(props, &child, row, errors);
                    if let Some(items) = props.get("items") {
                        walk(items, &format!("{child}/items"), row, errors);
                    }
                }
                item_type => check_event(props, &child, Site::Item(item_type), row, errors),
            }
        }
        _ => {}
    }
}

fn check_interaction(props: &Value, path: &str, row: Option<&str>, errors: &mut Vec<Finding>) {
    let Some(interaction) = props.get("interaction") else {
        return;
    };
    let path = format!("{path}/interaction");
    check_event(interaction, &path, Site::Container, row, errors);
    for (key, site) in [
        ("controls", Site::NamedControl),
        ("swipe_leading", Site::Swipe),
        ("swipe_trailing", Site::Swipe),
    ] {
        let Some(entries) = interaction.get(key).and_then(Value::as_array) else {
            continue;
        };
        for (index, entry) in entries.iter().enumerate() {
            check_event(entry, &format!("{path}/{key}/{index}"), site, row, errors);
        }
    }
}

/// The `event:` on `props`, if it has one, against what its site requires.
fn check_event(
    props: &Value, path: &str, site: Site<'_>, row: Option<&str>, errors: &mut Vec<Finding>,
) {
    let Some(event) = props.get("event").and_then(Value::as_str) else {
        return;
    };
    if crate::spec::holes::Kind::of(event).is_some() {
        return;
    }
    let Some((name, arguments)) = parse(event) else {
        // Not the event syntax at all, which the schema reports in its own words.
        return;
    };
    let screen = screen(path);
    let path = format!("{path}/event");

    let value = match site {
        Site::Item(item_type) if VALUE_ITEM_TYPES.contains(&item_type) => Some(item_type),
        _ => None,
    };
    // A row contributes which row, a value control contributes its value: those are the only
    // arguments either can supply, so they bound the count from both sides.
    let most = usize::from(value.is_some()) + usize::from(row.is_some());

    if let Some(item_type) = value {
        if arguments == 0 {
            let fix = match row {
                Some(_) => format!("`{name}(id, value)`"),
                None => format!("`{name}(value)`"),
            };
            errors.push(Finding::new(
                path,
                format!(
                    "on screen `{screen}`, the `{item_type}` raises `{event}` with no argument, \
                     so {} never reaches the app. A {item_type}'s event carries the value it \
                     produced: write {fix}{}",
                    produces(item_type),
                    row.map_or(String::new(), |row| format!(
                        ", naming the row as well because it sits in a row of {row}"
                    )),
                ),
            ));
            return;
        }
        if arguments > most {
            errors.push(Finding::new(
                path,
                format!(
                    "on screen `{screen}`, the `{item_type}` raises `{event}` with {arguments} \
                     arguments, and it produces one value: {}. A {item_type}'s event carries \
                     that value{}: write `{name}({})`",
                    produces(item_type),
                    if row.is_some() { " and which row it is in" } else { " and nothing else" },
                    if row.is_some() { "id, value" } else { "value" },
                ),
            ));
            return;
        }
    }

    if let Some(row) = row
        && arguments == 0
    {
        errors.push(Finding::new(
            path,
            format!(
                "on screen `{screen}`, {} raises `{event}` from a row of {row} with no argument, \
                 so the app cannot tell which row it came from. An event raised from inside a \
                 row template carries the field that identifies the row: write `{name}(id)`, \
                 using the identifying field the rows have",
                site.describe(),
            ),
        ));
    }
}

/// An event's name and how many arguments it carries, or `None` when it is not event syntax.
fn parse(event: &str) -> Option<(&str, usize)> {
    let event = event.trim();
    let Some(open) = event.find('(') else {
        return Some((event, 0));
    };
    let inner = event[open + 1..].strip_suffix(')')?;
    let arguments = inner.split(',').filter(|argument| !argument.trim().is_empty()).count();
    Some((&event[..open], arguments))
}

/// The screen slug a pointer is inside, as the messages name it.
fn screen(path: &str) -> String {
    let mut segments = path.split('/').skip(1);
    let slug = match segments.next() {
        Some("screens") => segments.next(),
        Some("delta") => segments.nth(1),
        _ => None,
    };
    slug.unwrap_or("?").replace("~1", "/").replace("~0", "~")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn findings(composition: &Value) -> Vec<Finding> {
        let mut errors = Vec::new();
        check_arguments(composition, &mut errors);
        errors
    }

    #[test]
    fn every_value_control_is_a_control() {
        for item_type in VALUE_ITEM_TYPES {
            assert!(
                super::super::interaction::CONTROL_ITEM_TYPES.contains(&item_type),
                "`{item_type}` produces a value but is not a control that owns its tap"
            );
        }
    }

    #[test]
    fn parse_counts_arguments() {
        assert_eq!(parse("QueryChanged"), Some(("QueryChanged", 0)));
        assert_eq!(parse("QueryChanged()"), Some(("QueryChanged", 0)));
        assert_eq!(parse("QueryChanged(value)"), Some(("QueryChanged", 1)));
        assert_eq!(parse("TitleEdited(id, value)"), Some(("TitleEdited", 2)));
        assert_eq!(parse("Broken(value"), None);
    }

    #[test]
    fn a_value_control_in_a_row_may_name_the_row_too() {
        let composition = json!({"screens": {"tasks": {"body": {"list": {
            "each": "tasks",
            "item": [{"text-field": {"event": "TitleEdited(id, value)"}}],
        }}}}});
        assert_eq!(findings(&composition), Vec::new());
    }

    #[test]
    fn a_value_control_outside_a_row_carries_exactly_one() {
        let composition = json!({"screens": {"new": {"body": [
            {"slider": {"event": "VolumeChanged(id, value)"}},
        ]}}});
        let errors = findings(&composition);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].message().contains("`VolumeChanged(value)`"), "{errors:?}");
    }

    #[test]
    fn a_hole_is_not_checked() {
        let composition = json!({"screens": {"new": {"body": {"list": {
            "each": "cities",
            "item": [{"search-field": {"event": "TODO(event)"}}],
        }}}}});
        assert_eq!(findings(&composition), Vec::new());
    }

    #[test]
    fn a_flip_on_a_switch_may_be_bare() {
        let composition = json!({"screens": {"settings": {"body": [
            {"switch": {"bind": "night_palette", "event": "NightPaletteToggled"}},
        ]}}});
        assert_eq!(findings(&composition), Vec::new());
    }

    #[test]
    fn the_screen_is_read_from_the_pointer() {
        assert_eq!(screen("/screens/add-city/header/trailing/0/search-field"), "add-city");
        assert_eq!(screen("/delta/added/add-city/body/0"), "add-city");
    }
}
