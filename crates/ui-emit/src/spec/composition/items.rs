//! The item vocabulary and the `style` closure.
//!
//! Two checks the schema cannot express on its own: an item key is only
//! legal when it is in the closed vocabulary *or* declared in this
//! document's `custom_items`, and `style` on a text-bearing item names a
//! `tokens.typography` entry rather than an arbitrary word. Types whose
//! `style` is a variant (see `VARIANT_STYLED`) close in the schema;
//! every other known type has no `style` at all, which is what stops
//! "outlined" and "title" meaning the same key on the same document.

use std::collections::BTreeSet;

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// The closed item vocabulary. Mirrors `$defs/knownItemType` in
/// `composition.schema.json`; `tests/validate.rs` pins them equal.
pub const KNOWN_ITEM_TYPES: [&str; 24] = [
    "avatar",
    "badge",
    "button",
    "checkbox",
    "chip",
    "date-picker",
    "divider",
    "fab",
    "icon",
    "icon-button",
    "image",
    "link",
    "picker",
    "progress-indicator",
    "radio",
    "search-field",
    "segmented-control",
    "slider",
    "spacer",
    "stepper",
    "switch",
    "text",
    "text-area",
    "text-field",
];

/// Item types whose `style` names a `tokens.typography` entry.
///
/// Every type here renders author-supplied text as its primary content,
/// so the type scale is the only thing `style` could sensibly mean. The
/// editable fields are included because a field's placeholder and value
/// are set in the same scale as body copy.
const TYPOGRAPHY_STYLED: [&str; 4] = ["link", "text", "text-area", "text-field"];

/// Item types whose `style` is a closed variant set, enumerated in the
/// schema. Listed here only so the "no `style` on this type" check can
/// tell them apart from the types that genuinely have none.
const VARIANT_STYLED: [&str; 9] = [
    "badge",
    "button",
    "chip",
    "date-picker",
    "divider",
    "fab",
    "picker",
    "progress-indicator",
    "switch",
];

/// Content-node keys that hold other content rather than being items
/// themselves, paired with the property their children live under.
const CONTAINER_KEYS: [(&str, &str); 3] = [("group", "items"), ("list", "item"), ("grid", "item")];

/// One item occurrence: its type key, its props, and where it sits.
struct Item<'a> {
    type_name: &'a str,
    props: &'a Value,
    path: String,
}

/// Check every item's type against the vocabulary and its `style`
/// against the closure. `tokens` is `None` when the document has no
/// sibling `tokens.yaml` — typography references then go unchecked,
/// the same way every other token reference does.
pub(crate) fn check_items(composition: &Value, tokens: Option<&Value>, errors: &mut Vec<Finding>) {
    let custom = declared_custom_items(composition);
    let mut items = Vec::new();
    for (root, path) in region_roots(composition) {
        collect_items(root, &path, &mut items);
    }

    for item in &items {
        check_type(item, &custom, errors);
        check_style(item, tokens, errors);
        if custom.contains(item.type_name) {
            check_custom_props(item, errors);
        }
    }
}

/// A custom item's `raises` and `binds` are the contract the annotation declared, carried through
/// `figma-compile` untouched. They are the seam `contract::derive` reads the `Event` enum and the
/// view-model fields off, so a malformed entry there is an event that never compiles or a field the
/// core never grows — invisible until the shell fails to build. The schema cannot reach them: a
/// custom item is `additionalProperties`, so nothing but this says what shape its props take.
fn check_custom_props(item: &Item<'_>, errors: &mut Vec<Finding>) {
    check_string_list(
        item,
        "raises",
        is_event_value,
        "an event, as in `DateSelected(value)`",
        errors,
    );
    check_string_list(item, "binds", is_field_name, "a field name, as in `selected_date`", errors);
}

/// Validates a custom item's `raises` / `binds`: present means a non-empty list of strings, each
/// well-formed by `valid`.
fn check_string_list(
    item: &Item<'_>, key: &str, valid: fn(&str) -> bool, shape: &str, errors: &mut Vec<Finding>,
) {
    let Some(value) = item.props.get(key) else {
        return;
    };
    let path = format!("{}/{key}", item.path);

    let Some(entries) = value.as_array() else {
        errors.push(Finding::new(
            path,
            format!(
                "`{key}` on custom item `{}` must be a list of strings, each {shape}",
                item.type_name
            ),
        ));
        return;
    };

    for (index, entry) in entries.iter().enumerate() {
        match entry.as_str() {
            Some(text) if valid(text) => {}
            Some(text) => errors.push(Finding::new(
                format!("{path}/{index}"),
                format!("`{text}` in `{key}` on custom item `{}` is not {shape}", item.type_name),
            )),
            None => errors.push(Finding::new(
                format!("{path}/{index}"),
                format!("`{key}` on custom item `{}` must be a list of strings", item.type_name),
            )),
        }
    }
}

/// An event value: a `PascalCase` name with an optional argument list, matching the schema's
/// `eventValue` pattern so a custom item's `raises` reads the same as any other `event`.
fn is_event_value(value: &str) -> bool {
    let (name, args) = match value.split_once('(') {
        Some((name, rest)) => match rest.strip_suffix(')') {
            Some(args) => (name, Some(args)),
            None => return false,
        },
        None => (value, None),
    };

    let name_ok = name.starts_with(|c: char| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_alphanumeric());
    let args_ok = args.is_none_or(|args| {
        args.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ',' || c == ' ')
    });
    name_ok && args_ok
}

/// A field name: a `snake_case` identifier, optionally dot-qualified once, matching `bindValue` less
/// its `TODO(binding)` marker — a custom item binds a real field, never a hole.
fn is_field_name(value: &str) -> bool {
    let mut segments = value.split('.');
    let (Some(first), second, None) = (segments.next(), segments.next(), segments.next()) else {
        return false;
    };
    let is_ident = |segment: &str| {
        segment.starts_with(|c: char| c.is_ascii_lowercase() || c == '_')
            && segment.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    is_ident(first) && second.is_none_or(is_ident)
}

fn declared_custom_items(composition: &Value) -> BTreeSet<String> {
    composition
        .get("custom_items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn check_type(item: &Item<'_>, custom: &BTreeSet<String>, errors: &mut Vec<Finding>) {
    if KNOWN_ITEM_TYPES.contains(&item.type_name) || custom.contains(item.type_name) {
        return;
    }
    errors.push(Finding::new(
        item.path.clone(),
        format!(
            "unknown item type `{}` -- it is outside the composition item vocabulary ({}) and is \
             not declared in this document's `custom_items`. Use a vocabulary item, or declare \
             the type under `custom_items` so every shell writer knows it is deliberate",
            item.type_name,
            KNOWN_ITEM_TYPES.join(", "),
        ),
    ));
}

fn check_style(item: &Item<'_>, tokens: Option<&Value>, errors: &mut Vec<Finding>) {
    let Some(style) = item.props.get("style").and_then(Value::as_str) else {
        return;
    };
    let path = format!("{}/style", item.path);

    if VARIANT_STYLED.contains(&item.type_name) {
        // The schema owns these enums; a bad value already errored there.
        return;
    }

    if !TYPOGRAPHY_STYLED.contains(&item.type_name) {
        if !KNOWN_ITEM_TYPES.contains(&item.type_name) {
            // A custom item is the extension seam: it owns its own props.
            return;
        }
        errors.push(Finding::new(
            path,
            format!(
                "`style` is not a property of item type `{}` -- `style` names a \
                 tokens.typography entry on {}, and a closed variant on {}. Express this with \
                 the type's own properties or a token reference",
                item.type_name,
                TYPOGRAPHY_STYLED.join(" / "),
                VARIANT_STYLED.join(" / "),
            ),
        ));
        return;
    }

    let Some(tokens) = tokens else {
        return;
    };
    let resolved = tokens
        .get("typography")
        .and_then(Value::as_object)
        .is_some_and(|entries| entries.contains_key(style));
    if !resolved {
        errors.push(Finding::new(
            path,
            format!(
                "composition references unknown typography token `{style}` -- not present in \
                 tokens.yaml under `typography.{style}`. On `{}`, `style` is a type scale \
                 reference, not a free-form variant name",
                item.type_name,
            ),
        ));
    }
}

/// Every sub-tree that holds content nodes, paired with its pointer.
///
/// Walking from the regions rather than over the whole document is what
/// keeps `provenance.sources[]` and other single-key maps from being
/// read as items.
pub(crate) fn region_roots(composition: &Value) -> Vec<(&Value, String)> {
    let mut roots = Vec::new();
    for (screens, prefix) in screen_maps(composition) {
        for (slug, screen) in screens {
            collect_screen_roots(
                screen,
                &format!("{prefix}/{}", escape_pointer_token(slug)),
                &mut roots,
            );
        }
    }
    roots
}

fn screen_maps(composition: &Value) -> Vec<(&serde_json::Map<String, Value>, String)> {
    let mut maps = Vec::new();
    if let Some(screens) = composition.get("screens").and_then(Value::as_object) {
        maps.push((screens, "/screens".to_string()));
    }
    if let Some(delta) = composition.get("delta").and_then(Value::as_object) {
        for section in ["added", "modified"] {
            if let Some(screens) = delta.get(section).and_then(Value::as_object) {
                maps.push((screens, format!("/delta/{section}")));
            }
        }
    }
    maps
}

/// A screen's own regions, its states' and overlays' regions, and every
/// `platforms.<platform>` override of the same.
fn collect_screen_roots<'a>(screen: &'a Value, path: &str, out: &mut Vec<(&'a Value, String)>) {
    collect_region_roots(screen, path, out);

    for (container, child_key) in [("states", None), ("overlays", Some("content"))] {
        let Some(entries) = screen.get(container).and_then(Value::as_object) else {
            continue;
        };
        for (name, entry) in entries {
            let entry_path = format!("{path}/{container}/{}", escape_pointer_token(name));
            match child_key {
                Some(key) => {
                    if let Some(content) = entry.get(key) {
                        out.push((content, format!("{entry_path}/{key}")));
                    }
                }
                None => collect_region_roots(entry, &entry_path, out),
            }
        }
    }

    if let Some(platforms) = screen.get("platforms").and_then(Value::as_object) {
        for (platform, override_entry) in platforms {
            collect_region_roots(
                override_entry,
                &format!("{path}/platforms/{}", escape_pointer_token(platform)),
                out,
            );
        }
    }
}

/// `header` (leading / trailing / below), `body` (list / grid / form / bare
/// array), `footer`, and `fab`.
fn collect_region_roots<'a>(node: &'a Value, path: &str, out: &mut Vec<(&'a Value, String)>) {
    if let Some(header) = node.get("header") {
        for side in ["leading", "trailing", "below"] {
            if let Some(content) = header.get(side) {
                out.push((content, format!("{path}/header/{side}")));
            }
        }
    }
    if let Some(body) = node.get("body") {
        let body_path = format!("{path}/body");
        match body {
            Value::Array(_) => out.push((body, body_path)),
            Value::Object(map) => {
                for (pattern, key) in [("list", "item"), ("grid", "item"), ("form", "")] {
                    if let Some(entry) = map.get(pattern) {
                        let pattern_path = format!("{body_path}/{pattern}");
                        if key.is_empty() {
                            out.push((entry, pattern_path));
                        } else if let Some(items) = entry.get(key) {
                            out.push((items, format!("{pattern_path}/{key}")));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(footer) = node.get("footer") {
        out.push((footer, format!("{path}/footer")));
    }
    // `fab` is a bare itemProps object, not a content node: wrap it so
    // the collector sees one `fab` item.
    if let Some(fab) = node.get("fab") {
        out.push((fab, format!("{path}/fab")));
    }
}

fn collect_items<'a>(node: &'a Value, path: &str, out: &mut Vec<Item<'a>>) {
    match node {
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                collect_items(entry, &format!("{path}/{index}"), out);
            }
        }
        Value::Object(map) => {
            // A content node is single-key: `{ group: … }` or
            // `{ <item-type>: … }`. The `fab` region is the one place a
            // bare props object appears; it carries no single item key,
            // so it never reaches the item branch.
            if map.len() != 1 {
                if path.ends_with("/fab") {
                    out.push(Item {
                        type_name: "fab",
                        props: node,
                        path: path.to_string(),
                    });
                }
                return;
            }
            let Some((key, value)) = map.iter().next() else {
                return;
            };
            let child = format!("{path}/{}", escape_pointer_token(key));
            // Layout patterns nest anywhere a content node may appear, so
            // a `grid` inside a group's `items` is a container to descend
            // through, not an item type to look up in the vocabulary.
            if let Some(children) = CONTAINER_KEYS.iter().find(|(name, _)| *name == key) {
                if let Some(nested) = value.get(children.1) {
                    collect_items(nested, &format!("{child}/{}", children.1), out);
                }
                return;
            }
            if key == "form" {
                collect_items(value, &child, out);
                return;
            }
            out.push(Item {
                type_name: key,
                props: value,
                path: child,
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn findings(composition: &Value, tokens: Option<&Value>) -> Vec<String> {
        let mut errors = Vec::new();
        check_items(composition, tokens, &mut errors);
        errors.iter().map(|finding| finding.message().to_owned()).collect()
    }

    #[test]
    fn provenance_entries_are_not_read_as_items() {
        let doc = json!({
            "version": 1,
            "provenance": { "sources": [{ "kind": "manual" }] },
            "screens": { "home": { "name": "Home" } },
        });
        assert!(findings(&doc, None).is_empty());
    }

    #[test]
    fn a_custom_item_declares_itself_out_of_the_vocabulary() {
        let undeclared = json!({
            "version": 1,
            "screens": { "home": { "name": "Home", "body": [{ "sparkline": {} }] } },
        });
        assert_eq!(findings(&undeclared, None).len(), 1);

        let declared = json!({
            "version": 1,
            "custom_items": [{ "name": "sparkline" }],
            "screens": { "home": { "name": "Home", "body": [{ "sparkline": {} }] } },
        });
        assert!(findings(&declared, None).is_empty());
    }

    #[test]
    fn a_custom_items_raises_and_binds_are_validated() {
        let doc = |item: Value| {
            json!({
                "version": 1,
                "custom_items": [{ "name": "month-grid" }],
                "screens": { "home": { "name": "Home", "body": [item] } },
            })
        };

        // A well-formed contract passes.
        assert!(
            findings(
                &doc(json!({ "month-grid": {
                    "raises": ["DateSelected(value)", "MonthChanged"],
                    "binds": ["visible_month", "task.due"]
                } })),
                None,
            )
            .is_empty()
        );

        // A malformed event and a malformed field each report against their entry.
        let bad = findings(
            &doc(
                json!({ "month-grid": { "raises": ["dateSelected"], "binds": ["Visible-Month"] } }),
            ),
            None,
        );
        assert_eq!(bad.len(), 2, "{bad:?}");
        assert!(bad.iter().any(|message| message.contains("dateSelected")), "{bad:?}");
        assert!(bad.iter().any(|message| message.contains("Visible-Month")), "{bad:?}");

        // A scalar where a list belongs is a single finding.
        let scalar = findings(&doc(json!({ "month-grid": { "raises": "DateSelected" } })), None);
        assert_eq!(scalar.len(), 1, "{scalar:?}");
        assert!(scalar[0].contains("must be a list"), "{}", scalar[0]);
    }

    #[test]
    fn style_closes_by_item_type() {
        let tokens = json!({ "typography": { "body": { "size": 16 } } });
        let doc = |item: Value| json!({ "version": 1, "screens": { "home": { "name": "Home", "body": [item] } } });

        assert!(findings(&doc(json!({ "text": { "style": "body" } })), Some(&tokens)).is_empty());
        assert_eq!(
            findings(&doc(json!({ "text": { "style": "outlined" } })), Some(&tokens)).len(),
            1
        );
        // The schema owns the variant enums; the validator stays quiet.
        assert!(
            findings(&doc(json!({ "button": { "style": "outlined" } })), Some(&tokens)).is_empty()
        );
        assert_eq!(
            findings(&doc(json!({ "image": { "style": "rounded" } })), Some(&tokens)).len(),
            1
        );
    }
}
