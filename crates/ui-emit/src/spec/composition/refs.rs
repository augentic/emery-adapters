//! Cross-artifact reference resolution: token references against
//! `tokens.json` categories and static asset references against
//! `assets.json` ids.

use serde_json::Value;

use super::finding::Finding;
use crate::spec::assets::collect_asset_references;
use crate::spec::shared::escape_pointer_token;

pub fn resolve_token_references(composition: &Value, tokens: &Value, errors: &mut Vec<Finding>) {
    walk_token_refs(composition, "", tokens, errors);
}

fn walk_token_refs(node: &Value, json_path: &str, tokens: &Value, errors: &mut Vec<Finding>) {
    match node {
        Value::Object(map) => {
            for (key, val) in map {
                // An extension slot (`x-*`): opaque facts for one target, not references.
                if ui_ir::is_extension_slot(key) {
                    continue;
                }
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

                // `border` may also be a borderSpec object, whose `width` names the `border`
                // category. Its `color` needs no help: the recursion below reaches it under a key
                // this table already routes to `colors`.
                if key == "border" {
                    if let Some(name) = val.get("width").and_then(Value::as_str) {
                        check_token_ref(
                            "border",
                            name,
                            &format!("{child_path}/width"),
                            tokens,
                            errors,
                        );
                    }
                    check_border_color(val, &child_path, tokens, errors);
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

/// Refuses a border nothing states a colour for, at either end.
///
/// A `border` token is a width. Figma keeps the colour on the frame that draws the stroke, so
/// `tokens.json` usually cannot say one and `composition.json` says it per layer instead. Neither
/// half is required on its own, but between them one of them has to answer, because the emitter
/// has no third place to look: a border with no colour reserves its width and draws nothing, which
/// looks like a spacing bug several files away from the artifact that caused it.
///
/// Reported here rather than in the emitter so it is one finding against the line that decided it,
/// before anything is rendered.
fn check_border_color(border: &Value, json_path: &str, tokens: &Value, errors: &mut Vec<Finding>) {
    // The object form states its own colour, or names a width that has to have one. The bare string
    // form is the width alone, so it always falls through to the token.
    let width = match border {
        Value::String(width) => width,
        Value::Object(spec) => {
            if spec.contains_key("color") {
                return;
            }
            let Some(Value::String(width)) = spec.get("width") else { return };
            width
        }
        _ => return,
    };

    // An unresolvable width is already reported by [`check_token_ref`], and a second finding about
    // the colour of a token that does not exist would only bury the first.
    let Some(entry) = tokens.get("border").and_then(Value::as_object).and_then(|m| m.get(width))
    else {
        return;
    };
    if entry.get("color").and_then(Value::as_str).is_some() {
        return;
    }

    errors.push(Finding::new(
        json_path,
        format!(
            "border `{width}` is drawn in no colour -- this layer names none and `border.{width}` \
             in tokens.json has none either, so it would render transparent. Give the layer a \
             `border: {{width: {width}, color: <colors token>}}`, or set `border.{width}.color` in \
             tokens.json as the default for every layer that names this width bare.",
        ),
    ));
}

const fn token_category_for_key(key: &str) -> Option<&'static str> {
    match key.as_bytes() {
        b"color" | b"background" => Some("colors"),
        b"elevation" => Some("elevation"),
        b"gap" | b"cross_gap" | b"padding" => Some("spacing"),
        b"shadow" => Some("shadow"),
        b"text_style" => Some("typography"),
        b"corner_radius" => Some("cornerRadius"),
        b"border" => Some("border"),
        b"opacity" => Some("opacity"),
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
                "composition references unknown {category} token `{name}` -- not present in tokens.json under `{category}.{name}`",
            ),
        ));
    }
}

pub fn resolve_asset_references(composition: &Value, assets: &Value, errors: &mut Vec<Finding>) {
    let asset_ids = assets.get("assets").and_then(Value::as_object);
    let refs = collect_asset_references(composition);
    for asset_ref in &refs {
        let exists = asset_ids.is_some_and(|m| m.contains_key(&asset_ref.id));
        if !exists {
            errors.push(Finding::new(
                asset_ref.path.clone(),
                format!(
                    "composition references unknown asset id `{}` -- not present in assets.json",
                    asset_ref.id,
                ),
            ));
        }
    }
}
