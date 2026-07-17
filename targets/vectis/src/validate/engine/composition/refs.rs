//! Cross-artifact reference resolution: token references against
//! `tokens.yaml` categories and static asset references against
//! `assets.yaml` ids.

use serde_json::Value;

use super::finding::Finding;
use crate::validate::engine::assets::collect_asset_references;
use crate::validate::engine::shared::escape_pointer_token;

pub(super) fn resolve_token_references(
    composition: &Value, tokens: &Value, errors: &mut Vec<Finding>,
) {
    walk_token_refs(composition, "", tokens, errors);
}

fn walk_token_refs(node: &Value, json_path: &str, tokens: &Value, errors: &mut Vec<Finding>) {
    match node {
        Value::Object(map) => {
            for (key, val) in map {
                let child_path = format!("{json_path}/{}", escape_pointer_token(key));

                if let Some(category) = token_category_for_key(key)
                    && let Some(name) = val.as_str()
                {
                    check_token_ref(category, name, &child_path, tokens, errors);
                }

                // `padding` may also be a paddingSpec object: walk
                // each side as a spacing ref.
                if key == "padding"
                    && let Some(side_map) = val.as_object()
                {
                    for (side, side_val) in side_map {
                        if let Some(name) = side_val.as_str() {
                            let side_path = format!("{child_path}/{}", escape_pointer_token(side));
                            check_token_ref("spacing", name, &side_path, tokens, errors);
                        }
                    }
                }

                walk_token_refs(val, &child_path, tokens, errors);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                walk_token_refs(v, &format!("{json_path}/{i}"), tokens, errors);
            }
        }
        _ => {}
    }
}

const fn token_category_for_key(key: &str) -> Option<&'static str> {
    match key.as_bytes() {
        b"color" | b"background" => Some("colors"),
        b"elevation" => Some("elevation"),
        b"gap" | b"padding" => Some("spacing"),
        b"corner_radius" => Some("cornerRadius"),
        _ => None,
    }
}

fn check_token_ref(
    category: &str, name: &str, json_path: &str, tokens: &Value, errors: &mut Vec<Finding>,
) {
    let exists =
        tokens.get(category).and_then(Value::as_object).is_some_and(|m| m.contains_key(name));
    if !exists {
        errors.push(Finding::new(
            json_path,
            format!(
                "composition references unknown {category} token `{name}` -- not present in tokens.yaml under `{category}.{name}`",
            ),
        ));
    }
}

pub(super) fn resolve_asset_references(
    composition: &Value, assets: &Value, errors: &mut Vec<Finding>,
) {
    let asset_ids = assets.get("assets").and_then(Value::as_object);
    let refs = collect_asset_references(composition);
    for asset_ref in &refs {
        let exists = asset_ids.is_some_and(|m| m.contains_key(&asset_ref.id));
        if !exists {
            errors.push(Finding::new(
                asset_ref.path.clone(),
                format!(
                    "composition references unknown asset id `{}` -- not present in assets.yaml",
                    asset_ref.id,
                ),
            ));
        }
    }
}
