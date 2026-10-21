//! A collection may sit in content, but not in another collection's row, nor inside a component.
//!
//! A `list` or `grid` nested in a group is one more repeated field on the screen, and its rows are a
//! row scope of their own. Inside a row template there is no scope to put that field in: the row
//! renders one element, and [`crate::contract`] derives that element's fields from what the row
//! binds. A collection there would have to become a field *of the element*, a second level of data
//! the contract does not derive, so it is refused here, beside the file.
//!
//! A component is refused for the same reason from the other side: it is emitted once and rendered
//! with its parameters alone in scope, so it has nothing to iterate either. [`crate::model`]
//! refuses the same shape, and this says so beside the file first.

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// Where a collection would have nothing to iterate from.
#[derive(Clone, Copy)]
enum Scope {
    /// The screen's own content, where a collection is a field on the screen.
    Screen,

    /// A collection's row template, which renders one element.
    Row,

    /// A component placement, rendered with its parameters alone.
    Component,
}

/// Refuses every `list` or `grid` inside another collection's row template or a component.
pub fn check_nested_collections(composition: &Value, errors: &mut Vec<Finding>) {
    for (root, path) in super::items::region_roots(composition) {
        // A body that is a collection arrives already unwrapped to its row template.
        let scope = if path.ends_with("/list/item") || path.ends_with("/grid/item") {
            Scope::Row
        } else {
            Scope::Screen
        };
        walk(root, &path, scope, errors);
    }
}

fn walk(node: &Value, path: &str, scope: Scope, errors: &mut Vec<Finding>) {
    match node {
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{path}/{index}"), scope, errors);
            }
        }
        Value::Object(map) if map.len() == 1 => {
            let Some((key, props)) = map.iter().next() else {
                return;
            };
            let child = format!("{path}/{}", escape_pointer_token(key));
            match (key.as_str(), scope) {
                ("list" | "grid", Scope::Row) => errors.push(Finding::new(
                    child,
                    format!(
                        "a `{key}` inside another collection's row template has no scope to \
                         iterate: a row renders one element, and holds no collection of its own. \
                         Move it beside the outer collection, or make what it shows fields of the \
                         outer row"
                    ),
                )),
                ("list" | "grid", Scope::Component) => errors.push(Finding::new(
                    child,
                    format!(
                        "a `{key}` inside a `component:` group has nothing to iterate: a \
                         component is rendered with its parameters alone. Place the collection \
                         beside the component instead of inside it"
                    ),
                )),
                ("list" | "grid", Scope::Screen) => {
                    if let Some(items) = props.get("item") {
                        walk(items, &format!("{child}/item"), Scope::Row, errors);
                    }
                }
                ("group", _) => {
                    let scope = match scope {
                        Scope::Screen if props.get("component").is_some() => Scope::Component,
                        _ => scope,
                    };
                    if let Some(items) = props.get("items") {
                        walk(items, &format!("{child}/items"), scope, errors);
                    }
                }
                ("form", _) => walk(props, &child, scope, errors),
                _ => {}
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn found(composition: &Value) -> Vec<Finding> {
        let mut errors = Vec::new();
        check_nested_collections(composition, &mut errors);
        errors
    }

    #[test]
    fn a_collection_beside_content_is_admitted() {
        let composition = json!({"screens": {"today": {"body": [
            {"text": {"content": "Hourly"}},
            {"group": {"items": [{"list": {"each": "hours", "item": [{"text": {"bind": "time"}}]}}]}},
        ]}}});
        assert_eq!(found(&composition), Vec::new());
    }

    #[test]
    fn a_collection_in_a_row_template_is_refused_wherever_the_outer_one_sits() {
        let inner = json!({"list": {"each": "hours", "item": [{"text": {"bind": "time"}}]}});
        let body_list = json!({"screens": {"days": {"body": {"list": {
            "each": "days",
            "item": [{"group": {"items": [inner]}}],
        }}}}});
        let nested_list = json!({"screens": {"today": {"body": [
            {"list": {"each": "days", "item": [inner]}},
        ]}}});

        for composition in [body_list, nested_list] {
            let errors = found(&composition);
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert!(errors[0].path().ends_with("/list"), "{errors:?}");
        }
    }

    #[test]
    fn a_collection_inside_a_component_is_refused() {
        let composition = json!({"screens": {"today": {"body": [
            {"group": {"component": "forecast", "items": [
                {"list": {"each": "hours", "item": [{"text": {"bind": "time"}}]}},
            ]}},
        ]}}});
        let errors = found(&composition);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].message().contains("`component:`"), "{errors:?}");
    }
}
