//! A bound variant names an `enum` param of its component, and sits in a row.
//!
//! `params: {state: {bind: hour_state}}` says each row of a collection reads its own look from a
//! field, because the drawn rows differ in it (weather-test's hour strip: one `HourCell` at
//! `state=now`, the rest at `state=default`). `vx_emit` types that field by the variant's declared
//! values and switches the row's look on it, so two facts have to hold, and neither is visible to a
//! schema that sees one file:
//!
//! - The axis is a variant of the component, an `enum` param in `components.yaml`. Anything else
//!   gives the field no values and the switch no cases.
//! - The placement is inside a `list` or `grid` row. A row has an element to read a field from; a
//!   placement on the screen is one look, chosen in the design.
//!
//! - The row carries `looks:`, one per value the field can take, each with the template's shape and
//!   none of its reads: no `bind`, `event` or `interaction`, which a shell takes from the template
//!   at the same position. A look the shape does not line up with has nothing to take them from.
//!
//! A `TODO(binding)` field is a hole like any other; its looks are checked all the same.

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// Checks every bound `params` entry in the document against the catalogue.
///
/// `catalog` is `components.yaml`. Without one there is nothing to check an axis against, so only
/// the placement is judged.
pub(crate) fn check_bound_params(
    composition: &Value, catalog: Option<&Value>, errors: &mut Vec<Finding>,
) {
    for (root, path) in super::items::region_roots(composition) {
        let in_row = path.ends_with("/list/item") || path.ends_with("/grid/item");
        walk(root, &path, in_row, catalog, errors);
    }
}

fn walk(
    node: &Value, path: &str, in_row: bool, catalog: Option<&Value>, errors: &mut Vec<Finding>,
) {
    match node {
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{path}/{index}"), in_row, catalog, errors);
            }
        }
        Value::Object(map) if map.len() == 1 => {
            let Some((key, props)) = map.iter().next() else { return };
            let child = format!("{path}/{}", escape_pointer_token(key));
            match key.as_str() {
                "list" | "grid" => {
                    if let Some(items) = props.get("item") {
                        walk(items, &format!("{child}/item"), true, catalog, errors);
                    }
                }
                "group" => {
                    params(props, &child, in_row, catalog, errors);
                    if let Some(items) = props.get("items") {
                        walk(items, &format!("{child}/items"), in_row, catalog, errors);
                    }
                }
                "form" => walk(props, &child, in_row, catalog, errors),
                _ => {}
            }
        }
        _ => {}
    }
}

fn params(
    group: &Value, path: &str, in_row: bool, catalog: Option<&Value>, errors: &mut Vec<Finding>,
) {
    looks(group, path, catalog, errors);
    let Some(params) = group.get("params").and_then(Value::as_object) else { return };
    let slug = group.get("component").and_then(Value::as_str).unwrap_or_default();

    for (axis, value) in params {
        let Some(field) = value.get("bind").and_then(Value::as_str) else { continue };
        let at = format!("{path}/params/{}", escape_pointer_token(axis));

        if !in_row {
            errors.push(Finding::new(
                at.clone(),
                format!(
                    "`{axis}: {{bind: {field}}}` reads a variant from a row's field, and this \
                     placement is not in a `list` or `grid` row, so there is no row to read it from. \
                     Set `{axis}` to one of its values here"
                ),
            ));
        }

        let Some(catalog) = catalog else { continue };
        let declared = catalog.pointer(&format!(
            "/components/{}/params/{}",
            escape_pointer_token(slug),
            escape_pointer_token(axis)
        ));
        if declared.and_then(|param| param.get("type")).and_then(Value::as_str) != Some("enum") {
            errors.push(Finding::new(
                at,
                format!(
                    "`{axis}` is bound to `{field}`, and components.yaml declares no `{axis}` \
                     variant on `{slug}`, so the field has no values to take and the row no looks \
                     to switch between. Bind one of the component's `enum` params, or set a value"
                ),
            ));
        }
    }
}

/// A row's `looks`: present exactly when one param is bound, keyed by values the catalogue
/// declares, each the template's shape and nothing it reads.
fn looks(group: &Value, path: &str, catalog: Option<&Value>, errors: &mut Vec<Finding>) {
    let bound: Vec<&str> = group
        .get("params")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(_, value)| value.get("bind").is_some())
        .map(|(axis, _)| axis.as_str())
        .collect();
    let looks = group.get("looks").and_then(Value::as_object);
    let at = format!("{path}/looks");

    let (axis, looks) = match (bound.as_slice(), looks) {
        ([], None) => return,
        ([axis], Some(looks)) => (*axis, looks),
        ([axis], None) => {
            errors.push(Finding::new(
                format!("{path}/params/{}", escape_pointer_token(axis)),
                format!(
                    "`{axis}` is bound, and the row has no `looks:` to switch between, so no shell \
                     can draw a row at any value of it. Re-extract: the compiler writes one look per \
                     value the drawn rows show"
                ),
            ));
            return;
        }
        ([], Some(_)) => {
            errors.push(Finding::new(
                at,
                "`looks:` says how a row draws at each value of a bound variant, and no param here \
                 is bound. Bind the variant the looks differ in, or remove them",
            ));
            return;
        }
        (_, _) => {
            errors.push(Finding::new(
                at,
                format!(
                    "{} params are bound ({}), and a row switches its look on one field. Bind one \
                     variant and set the rest",
                    bound.len(),
                    bound.iter().map(|axis| format!("`{axis}`")).collect::<Vec<_>>().join(", ")
                ),
            ));
            return;
        }
    };

    let slug = group.get("component").and_then(Value::as_str).unwrap_or_default();
    let declared: Option<Vec<&str>> = catalog
        .and_then(|catalog| {
            catalog.pointer(&format!(
                "/components/{}/params/{}/values",
                escape_pointer_token(slug),
                escape_pointer_token(axis)
            ))
        })
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_str).collect());
    let template = shape(group);

    for (value, look) in looks {
        let here = format!("{at}/{}", escape_pointer_token(value));
        if let Some(declared) = &declared
            && !declared.contains(&value.as_str())
        {
            errors.push(Finding::new(
                here.clone(),
                format!(
                    "a look for `{axis}={value}`, and components.yaml declares `{axis}` on `{slug}` \
                     as {}. A look is keyed by a value the variant has",
                    declared.iter().map(|value| format!("`{value}`")).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        if let Some(key) = reads(look) {
            errors.push(Finding::new(
                here.clone(),
                format!(
                    "the `{value}` look has a `{key}:`, and a look is only how a row is drawn: what \
                     it reads and raises is the row's own, at the same position. Remove it from the \
                     look"
                ),
            ));
        }
        if shape(look) != template {
            errors.push(Finding::new(
                here,
                format!(
                    "the `{value}` look is not the row's shape — its items differ in number or kind \
                     — so there is no position to take the row's fields and events from. A look \
                     differs from the row in how it is drawn, never in what it holds"
                ),
            ));
        }
    }
}

/// The first `bind`, `format`, `event` or `interaction` key anywhere under a look. `format` is a read:
/// it says how the `bind` beside it is shown, so it is the template's exactly as the `bind` is.
fn reads(value: &Value) -> Option<&'static str> {
    match value {
        Value::Object(map) => ["bind", "format", "event", "interaction"]
            .into_iter()
            .find(|key| map.contains_key(*key))
            .or_else(|| map.values().find_map(reads)),
        Value::Array(entries) => entries.iter().find_map(reads),
        _ => None,
    }
}

/// A group's items as a tree of node kinds — `text,group(text,icon)` — which is what lines a look up
/// with its row position by position.
fn shape(group: &Value) -> String {
    group
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|node| node.as_object()?.iter().next())
        .map(|(kind, props)| match shape(props) {
            inner if inner.is_empty() => kind.clone(),
            inner => format!("{kind}({inner})"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn catalog() -> Value {
        json!({"components": {"hour-cell": {"params": {
            "time": {"type": "string"},
            "state": {"type": "enum", "values": ["default", "now"]},
        }}}})
    }

    fn found(composition: &Value) -> Vec<Finding> {
        let mut errors = Vec::new();
        check_bound_params(composition, Some(&catalog()), &mut errors);
        errors
    }

    fn strip(params: &Value) -> Value {
        json!({"screens": {"today": {"body": [
            {"list": {"each": "hours", "item": [
                {"group": {"component": "hour-cell", "params": params, "items": []}},
            ]}},
        ]}}})
    }

    #[test]
    fn a_row_binding_a_declared_variant_is_admitted() {
        let looks = json!({"now": {"items": []}});
        assert!(looked(&json!({"state": {"bind": "hour_state"}}), &looks).len() == 1, "the shape");
        let looks = json!({"now": {"items": [{"text": {"content": "14:00"}}]}});
        assert!(looked(&json!({"state": {"bind": "hour_state"}}), &looks).is_empty());
        assert!(looked(&json!({"state": {"bind": "TODO(binding)"}}), &looks).is_empty());
        assert!(found(&strip(&json!({"state": "now"}))).is_empty());
    }

    #[test]
    fn a_bound_axis_the_component_does_not_declare_as_a_variant_is_refused() {
        for axis in ["size", "time"] {
            let looks = json!({"now": {"items": [{"text": {"content": "14:00"}}]}});
            let errors = looked(&json!({axis: {"bind": "hour_state"}}), &looks);
            let errors: Vec<_> =
                errors.into_iter().filter(|e| e.message().contains("declares no")).collect();
            assert_eq!(errors.len(), 1, "{axis}: {errors:?}");
            assert!(errors[0].message().contains("declares no"), "{errors:?}");
            assert!(errors[0].path().ends_with(&format!("/params/{axis}")), "{errors:?}");
        }
    }

    fn looked(params: &Value, looks: &Value) -> Vec<Finding> {
        let composition = json!({"screens": {"today": {"body": [
            {"list": {"each": "hours", "item": [
                {"group": {"component": "hour-cell", "params": params, "looks": looks,
                           "items": [{"text": {"bind": "time"}}]}},
            ]}},
        ]}}});
        found(&composition)
    }

    #[test]
    fn looks_drawn_in_the_row_s_shape_are_admitted() {
        let looks = json!({
            "now": {"background": "surface-accent", "items": [{"text": {"content": "14:00"}}]},
            "default": {"items": [{"text": {"content": "15:00"}}]},
        });
        let errors = looked(&json!({"state": {"bind": "hour_state"}}), &looks);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn a_bound_variant_without_looks_is_refused() {
        let errors = found(&strip(&json!({"state": {"bind": "hour_state"}})))
            .into_iter()
            .filter(|finding| finding.message().contains("no `looks:`"))
            .count();
        assert_eq!(errors, 1);
    }

    #[test]
    fn a_look_that_reads_or_differs_in_shape_or_names_no_value_is_refused() {
        let bound = json!({"state": {"bind": "hour_state"}});
        let cases = [
            (json!({"now": {"items": [{"text": {"bind": "time"}}]}}), "has a `bind:`"),
            (json!({"now": {"items": [{"text": {"format": "time"}}]}}), "has a `format:`"),
            (json!({"now": {"items": [{"icon": {"name": "sun"}}]}}), "not the row's shape"),
            (json!({"later": {"items": [{"text": {"content": "x"}}]}}), "`default`, `now`"),
        ];
        for (looks, said) in cases {
            let errors = looked(&bound, &looks);
            assert_eq!(errors.len(), 1, "{said}: {errors:?}");
            assert!(errors[0].message().contains(said), "{errors:?}");
        }
        let errors = looked(&json!({"state": "now"}), &json!({"now": {"items": []}}));
        assert!(errors[0].message().contains("no param here is bound"), "{errors:?}");
    }

    #[test]
    fn a_bound_variant_outside_a_row_is_refused() {
        let composition = json!({"screens": {"today": {"body": [
            {"group": {"component": "hour-cell", "params": {"state": {"bind": "hour_state"}},
                       "items": []}},
        ]}}});
        let errors = found(&composition);
        assert!(
            errors.iter().any(|e| e.message().contains("not in a `list` or `grid` row")),
            "{errors:?}"
        );
    }
}
