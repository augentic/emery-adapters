//! What kind of value a bound text shows: `format` on the items and arguments that carry one.
//!
//! `format` says that a field is a typed value — a moment, a measurement, a number, a unit — which
//! the view formats with the browser's locale, so nobody writes weekday names or unit symbols as
//! copy. The schema owns the closed style list
//! (`$defs/formatStyle`) and that a `format` sits beside a `bind`. Two facts it cannot reach are
//! checked here:
//!
//! - **Only a `text` shows a formatted value.** On a `switch`, an `icon`, a `group` or a custom item
//!   it would be a promise no emitter keeps. The schema refuses it too (`textItem` alone declares
//!   `format`; `itemProps` and `groupProps` forbid it), and this states it in words a person can act
//!   on, for every item kind whatever arm of the walk it takes.
//! - **A field is one kind of value wherever it is shown.** The contract derives one typed field
//!   from each bound name, so `day` shown as a `weekday` in one place and as a `temperature` in
//!   another is a field with two types, and `day` shown with a `format` in one place and without
//!   one in another is a field that is both a typed value and a string. Styles of one kind mix
//!   freely — a moment can be shown as its `weekday` here and its `time` there, and a speed unit as
//!   its symbol and its long name.
//!
//! A field's scope is its screen, or the row template of the collection it sits in: a row's
//! `temperature` is a different field from the screen's. Only texts and component arguments are
//! compared — the displays a `format` can sit on. A control's `bind` (a `switch`'s `Bool`, a
//! `segmented-control`'s option) is typed by the control, not by a format, and an `icon`'s names an
//! asset. A hole (`TODO(binding)`) names no field yet, and a dot-qualified bind reaches an outer
//! scope this walk does not resolve; neither is compared.

use std::collections::BTreeMap;

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// The closed `format` vocabulary.
///
/// Mirrors `$defs/formatStyle` in ui-ir's `composition.schema.json`.
pub const FORMAT_STYLES: [&str; 18] = [
    "weekday",
    "time",
    "date",
    "date-time",
    "temperature",
    "speed",
    "pressure",
    "distance",
    "number",
    "percent",
    "temperature-unit",
    "speed-unit",
    "pressure-unit",
    "distance-unit",
    "temperature-unit-name",
    "speed-unit-name",
    "pressure-unit-name",
    "distance-unit-name",
];

/// The kind of value a style shows, which is what the contract types a field by.
///
/// `None` for a style outside [`FORMAT_STYLES`], which the schema reports in its own words.
#[must_use]
pub fn kind(style: &str) -> Option<String> {
    if !FORMAT_STYLES.contains(&style) {
        return None;
    }
    Some(match style {
        "weekday" | "time" | "date" | "date-time" => "a moment".to_owned(),
        "number" | "percent" => "a number".to_owned(),
        _ => match style.split_once("-unit") {
            Some((quantity, _)) => format!("a {quantity} unit"),
            None => format!("a {style}"),
        },
    })
}

/// One place a field is shown.
struct Use {
    /// The `format` it is shown with, `None` for a plain string.
    style: Option<String>,
    /// The pointer to the `bind` it is shown through.
    path: String,
}

/// Check every `format` in the document, and every field shown in more than one place.
pub fn check_formats(composition: &Value, errors: &mut Vec<Finding>) {
    let mut uses: BTreeMap<(String, String), Vec<Use>> = BTreeMap::new();

    for (root, path) in super::items::region_roots(composition) {
        let screen = screen_scope(&path);
        // A body that is a collection arrives already unwrapped to its row template.
        let scope =
            ["/list/item", "/grid/item"].iter().find(|suffix| path.ends_with(*suffix)).map_or_else(
                || screen.clone(),
                |_| {
                    let pattern = composition.pointer(&path[..path.len() - "/item".len()]);
                    row_scope(&screen, pattern, &path)
                },
            );
        walk(root, &path, &screen, &scope, &mut uses, errors);
    }

    for ((_, field), shown) in &uses {
        let Some(first) = shown.first() else { continue };
        for other in &shown[1..] {
            if let Some(message) = disagreement(field, first, other) {
                errors.push(Finding::new(other.path.clone(), message));
            }
        }
    }
}

/// The screen a region root belongs to, as the scope key of its screen-level fields: the pointer up
/// to and including the screen's slug.
fn screen_scope(path: &str) -> String {
    let depth = if path.starts_with("/delta/") { 4 } else { 3 };
    path.split('/').take(depth).collect::<Vec<_>>().join("/")
}

/// The scope of a collection's row template: the field it iterates, once that is known, so the same
/// collection drawn in a state and in the body is one row; otherwise the collection's own pointer.
fn row_scope(screen: &str, pattern: Option<&Value>, path: &str) -> String {
    match pattern.and_then(|pattern| pattern.get("each")).and_then(Value::as_str) {
        Some(each) if crate::spec::holes::Kind::of(each).is_none() => {
            format!("{screen} row of {each}")
        }
        _ => format!("{screen} row at {path}"),
    }
}

fn walk(
    node: &Value, path: &str, screen: &str, scope: &str,
    uses: &mut BTreeMap<(String, String), Vec<Use>>, errors: &mut Vec<Finding>,
) {
    match node {
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{path}/{index}"), screen, scope, uses, errors);
            }
        }
        Value::Object(map) if map.len() == 1 => {
            let Some((key, props)) = map.iter().next() else {
                return;
            };
            let child = format!("{path}/{}", escape_pointer_token(key));
            // Before the dispatch, so no arm can skip it: a `group`, a `list` or a custom item that
            // carries a `format` is as wrong as a `switch` that does.
            if key != "text" && props.get("format").is_some() {
                errors.push(Finding::new(
                    format!("{child}/format"),
                    format!(
                        "`format` says what kind of value a bound text shows, and this is a \
                         `{key}`. Only a `text` (or a component argument) formats a value: move \
                         the `format` to the text that shows it, or remove it"
                    ),
                ));
            }
            match key.as_str() {
                "list" | "grid" => {
                    if let Some(items) = props.get("item") {
                        let items_path = format!("{child}/item");
                        let row = row_scope(screen, Some(props), &child);
                        walk(items, &items_path, screen, &row, uses, errors);
                    }
                }
                "form" => walk(props, &child, screen, scope, uses, errors),
                "group" => {
                    if let Some(args) = props.get("args").and_then(Value::as_object) {
                        for (name, value) in args {
                            let at = format!("{child}/args/{}", escape_pointer_token(name));
                            record(value, &at, scope, uses);
                        }
                    }
                    if let Some(items) = props.get("items") {
                        walk(items, &format!("{child}/items"), screen, scope, uses, errors);
                    }
                }
                "text" => record(props, &child, scope, uses),
                _ => {}
            }
        }
        _ => {}
    }
}

/// Records the field a text or an argument shows, when it binds a real one in this scope.
fn record(props: &Value, path: &str, scope: &str, uses: &mut BTreeMap<(String, String), Vec<Use>>) {
    let Some(field) = props.get("bind").and_then(Value::as_str) else {
        return;
    };
    if crate::spec::holes::Kind::of(field).is_some() || field.contains('.') {
        return;
    }
    let style = props.get("format").and_then(Value::as_str).map(str::to_owned);
    uses.entry((scope.to_owned(), field.to_owned())).or_default().push(Use {
        style,
        path: format!("{path}/bind"),
    });
}

/// What is wrong with showing `field` as both `first` and `other`, if anything.
fn disagreement(field: &str, first: &Use, other: &Use) -> Option<String> {
    match (&first.style, &other.style) {
        (None, None) => None,
        (Some(style), None) | (None, Some(style)) => {
            let (formatted, plain) =
                if first.style.is_some() { (first, other) } else { (other, first) };
            Some(format!(
                "`{field}` is shown with `format: {style}` at `{}` and as plain text at `{}`. One \
                 field is a typed value or a string, not both: give every text showing `{field}` \
                 the same kind of `format`, or bind the plain one to a different field",
                formatted.path, plain.path
            ))
        }
        (Some(a), Some(b)) => {
            let (ka, kb) = (kind(a)?, kind(b)?);
            (ka != kb).then(|| {
                format!(
                    "`{field}` is shown as {ka} (`format: {a}`) at `{}` and as {kb} (`format: \
                     {b}`) here. One field has one type: format every text showing `{field}` as \
                     the same kind of value, or bind them to different fields",
                    first.path
                )
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FORMAT_STYLES, kind};

    #[test]
    fn every_style_has_a_kind_and_the_kinds_follow_the_quantity() {
        for style in FORMAT_STYLES {
            assert!(kind(style).is_some(), "{style}");
        }
        assert_eq!(kind("weekday"), kind("date-time"));
        assert_eq!(kind("number"), kind("percent"));
        assert_eq!(kind("speed-unit"), kind("speed-unit-name"));
        assert_ne!(kind("speed"), kind("speed-unit"));
        assert_ne!(kind("speed"), kind("pressure"));
        assert_ne!(kind("temperature-unit"), kind("pressure-unit-name"));
        assert_eq!(kind("measurement"), None);
    }
}
