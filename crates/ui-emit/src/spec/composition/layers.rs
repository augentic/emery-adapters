//! Layers, bounds and per-child alignment: the three things a web layout states that a flow of rows
//! and columns cannot.
//!
//! The schema closes each field's shape. What it cannot see is where a field sits and what it
//! names: a `position` means "a layer of the enclosing stack", so it is only meaningful directly
//! under a `direction: stack` group, and its `offset` names `tokens.offset` entries, which live in a
//! second document. A `min_*` above its `max_*` is an empty range no renderer can honour.

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// Runs the three checks over every content node in the document.
pub fn check_layers(composition: &Value, tokens: Option<&Value>, errors: &mut Vec<Finding>) {
    walk(composition, "", tokens, errors);
}

/// Every node array is checked against its container: a group's `items` against the group's
/// `direction`, and any other node list (a screen's `body`, a header row, a footer) as the column
/// it is drawn in, so a `position` there is refused too.
fn walk(node: &Value, path: &str, tokens: Option<&Value>, errors: &mut Vec<Finding>) {
    match node {
        Value::Object(map) => {
            for (key, value) in map {
                if ui_ir::is_extension_slot(key) {
                    continue;
                }
                let here = format!("{path}/{}", escape_pointer_token(key));
                if let Value::Array(children) = value {
                    let direction = if key == "items" {
                        map.get("direction").and_then(Value::as_str).unwrap_or("column")
                    } else {
                        "column"
                    };
                    for (index, child) in children.iter().enumerate() {
                        let at = format!("{here}/{index}");
                        if let Some((kind, props)) = node_props(child) {
                            check_props(
                                props,
                                &format!("{at}/{}", escape_pointer_token(kind)),
                                Some(direction),
                                tokens,
                                errors,
                            );
                        }
                        walk(child, &at, tokens, errors);
                    }
                    continue;
                }
                walk(value, &here, tokens, errors);
            }
        }
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{path}/{index}"), tokens, errors);
            }
        }
        _ => {}
    }
}

/// A content node's type key and its props, when it is a single-key object holding an object.
fn node_props(node: &Value) -> Option<(&str, &Value)> {
    let map = node.as_object()?;
    if map.len() != 1 {
        return None;
    }
    let (kind, props) = map.iter().next()?;
    props.is_object().then_some((kind.as_str(), props))
}

fn check_props(
    props: &Value, path: &str, parent: Option<&str>, tokens: Option<&Value>,
    errors: &mut Vec<Finding>,
) {
    if let Some(position) = props.get("position") {
        if parent != Some("stack") {
            errors.push(Finding::new(
                format!("{path}/position"),
                "`position` makes a child a layer of its stack, and this child's group is not \
                 `direction: stack` -- make the parent a stack (its flow content as the first \
                 child), or drop `position`",
            ));
        }
        if let Some(offset) = position.get("offset").and_then(Value::as_object) {
            for (axis, name) in offset {
                let Some(name) = name.as_str() else { continue };
                let known = tokens
                    .and_then(|tokens| tokens.get("offset"))
                    .and_then(Value::as_object)
                    .is_some_and(|names| names.contains_key(name));
                // Without a tokens.json nothing resolves, and that absence is a finding of its own.
                if tokens.is_some() && !known {
                    errors.push(Finding::new(
                        format!("{path}/position/offset/{}", escape_pointer_token(axis)),
                        format!(
                            "composition references unknown offset token `{name}` -- not present \
                             in tokens.json under `offset.{name}`"
                        ),
                    ));
                }
            }
        }
    }

    if let Some(size) = props.get("size") {
        for (low, high) in [("min_width", "max_width"), ("min_height", "max_height")] {
            if let (Some(min), Some(max)) =
                (size.get(low).and_then(Value::as_f64), size.get(high).and_then(Value::as_f64))
                && min > max
            {
                errors.push(Finding::new(
                    format!("{path}/size/{low}"),
                    format!("`{low}` ({min}) is above `{high}` ({max}), an empty range"),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn findings(composition: &Value, tokens: &Value) -> Vec<String> {
        let mut errors = Vec::new();
        check_layers(composition, Some(tokens), &mut errors);
        errors
            .into_iter()
            .map(|finding| format!("{}: {}", finding.path(), finding.message()))
            .collect()
    }

    fn screen(body: &Value) -> Value {
        json!({ "version": 1, "screens": { "index": { "name": "Index", "body": [body] } } })
    }

    #[test]
    fn a_layer_of_a_stack_with_a_resolving_offset_is_clean() {
        let composition = screen(&json!({ "group": { "direction": "stack", "items": [
            { "group": { "direction": "column", "items": [{ "text": { "content": "a" } }] } },
            { "button": { "content": "", "position": {
                "align": "top-end", "offset": { "x": "offset-m20", "y": "offset-16" } } } },
        ] } }));
        let tokens = json!({ "offset": { "offset-m20": -20, "offset-16": 16 } });
        assert_eq!(findings(&composition, &tokens), Vec::<String>::new());
    }

    #[test]
    fn a_position_outside_a_stack_is_refused() {
        let composition = screen(&json!({ "group": { "direction": "row", "items": [
            { "text": { "content": "a", "position": { "align": "center" } } },
        ] } }));
        let found = findings(&composition, &json!({}));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("/screens/index/body/0/group/items/0/text/position"));
    }

    #[test]
    fn an_offset_naming_no_token_is_refused() {
        let composition = screen(&json!({ "group": { "direction": "stack", "items": [
            { "text": { "content": "a", "position": { "align": "center", "offset": { "y": "offset-9" } } } },
        ] } }));
        let found = findings(&composition, &json!({ "offset": { "offset-8": 8 } }));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("unknown offset token `offset-9`"), "{found:?}");
    }

    #[test]
    fn an_empty_bound_range_is_refused() {
        let composition = screen(&json!({ "group": { "direction": "column", "items": [
            { "text": { "content": "a", "size": { "min_width": 30, "max_width": 20 } } },
        ] } }));
        let found = findings(&composition, &json!({}));
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("`min_width` (30) is above `max_width` (20)"), "{found:?}");
    }

    #[test]
    fn web_facts_are_not_read() {
        let composition = screen(&json!({ "group": { "direction": "column", "items": [
            { "text": { "content": "a", "x-web": { "items": [{ "text": { "position": { "align": "center" } } }] } } },
        ] } }));
        assert_eq!(findings(&composition, &json!({})), Vec::<String>::new());
    }
}
