//! A wrapping row is a grid, and a grid needs a cell width.
//!
//! `wrap: true` says the children flow onto further lines, but not how many fit on one: that is
//! the children's width against the row's, and a target laying the row out as a grid takes its
//! column width from the children. So every child must state one, and all must state the same
//! one. A child that hugs, fills, or disagrees leaves the renderer to guess at the columns, and
//! guessing is what put the weather app's four metric tiles on one line.

use serde_json::Value;

use super::finding::Finding;
use crate::spec::shared::escape_pointer_token;

/// Refuses every `wrap: true` group that is not a row of children sharing one fixed width.
pub fn check_wrapping_rows(node: &Value, json_path: &str, errors: &mut Vec<Finding>) {
    match node {
        Value::Object(map) => {
            if let Some(group) = map.get("group") {
                check_group(group, &format!("{json_path}/group"), errors);
            }
            for (key, value) in map {
                check_wrapping_rows(
                    value,
                    &format!("{json_path}/{}", escape_pointer_token(key)),
                    errors,
                );
            }
        }
        Value::Array(array) => {
            for (index, value) in array.iter().enumerate() {
                check_wrapping_rows(value, &format!("{json_path}/{index}"), errors);
            }
        }
        _ => {}
    }
}

fn check_group(group: &Value, path: &str, errors: &mut Vec<Finding>) {
    if group.get("wrap").and_then(Value::as_bool) != Some(true) {
        return;
    }

    if group.get("direction").and_then(Value::as_str) != Some("row") {
        errors.push(Finding::new(
            format!("{path}/wrap"),
            "only a `direction: row` group wraps; drop `wrap` or make the group a row",
        ));
        return;
    }

    let widths: Vec<Option<f64>> = group
        .get("items")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(cell_width).collect())
        .unwrap_or_default();

    let shared = widths.first().copied().flatten();
    let uniform = shared.is_some_and(|first| {
        widths.iter().all(|width| width.is_some_and(|width| (width - first).abs() < f64::EPSILON))
    });
    if !uniform {
        errors.push(Finding::new(
            format!("{path}/wrap"),
            "a wrapping row needs fixed-width children to form cells: every item must state the \
             same numeric `size.width`",
        ));
    }
}

/// The numeric width a content node states, whether it is a group or an item.
fn cell_width(node: &Value) -> Option<f64> {
    let (_, props) = node.as_object()?.iter().next()?;
    props.get("size")?.get("width")?.as_f64()
}
