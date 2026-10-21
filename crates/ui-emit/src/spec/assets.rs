//! Where a composition names an asset.
//!
//! The references are collected here and checked against `assets.json` by
//! [`super::composition::refs`].

use serde_json::Value;

use crate::spec::shared::escape_pointer_token;

/// One asset reference found in a composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRef {
    /// The referenced asset id, which is the `assets.json` key.
    pub id: String,

    /// JSON Pointer of the referencing key inside the composition.
    pub path: String,
}

/// Collects every static asset reference in a composition.
#[must_use]
pub fn collect_asset_references(value: &Value) -> Vec<AssetRef> {
    let mut refs = Vec::new();
    walk(value, "", &mut refs);
    refs
}

fn walk(node: &Value, json_path: &str, refs: &mut Vec<AssetRef>) {
    match node {
        Value::Object(map) => {
            for (key, val) in map {
                let child = format!("{json_path}/{}", escape_pointer_token(key));

                match key.as_str() {
                    "image" | "icon" => {
                        if let Some(name) = val.get("name").and_then(Value::as_str) {
                            refs.push(AssetRef {
                                id: name.to_owned(),
                                path: format!("{child}/name"),
                            });
                        }
                    }
                    // A component call site names the artwork each chosen nested instance shows.
                    "args" => {
                        for (param, arg) in val.as_object().into_iter().flatten() {
                            if let Some(asset) = arg.get("asset").and_then(Value::as_str) {
                                refs.push(AssetRef {
                                    id: asset.to_owned(),
                                    path: format!("{child}/{}/asset", escape_pointer_token(param)),
                                });
                            }
                        }
                    }
                    // A footer menu's tabs name their icon directly, like a control does.
                    "footer_menu" => {
                        let tabs = val.get("tabs").and_then(Value::as_array);
                        for (index, tab) in tabs.into_iter().flatten().enumerate() {
                            if let Some(icon) = tab.get("icon").and_then(Value::as_str) {
                                refs.push(AssetRef {
                                    id: icon.to_owned(),
                                    path: format!("{child}/tabs/{index}/icon"),
                                });
                            }
                        }
                    }
                    "icon-button" | "fab" => {
                        if let Some(icon) = val.get("icon").and_then(Value::as_str) {
                            refs.push(AssetRef {
                                id: icon.to_owned(),
                                path: format!("{child}/icon"),
                            });
                        }
                    }
                    _ => {}
                }

                walk(val, &child, refs);
            }
        }
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                walk(entry, &format!("{json_path}/{index}"), refs);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_icon_item_is_a_reference() {
        let composition = serde_json::json!({ "items": [{ "icon": { "name": "trash" } }] });
        let refs = collect_asset_references(&composition);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "trash");
        assert_eq!(refs[0].path, "/items/0/icon/name");
    }

    /// `icon-button` and `fab` name their glyph on `icon`, not on a nested `name`.
    #[test]
    fn a_control_names_its_glyph_directly() {
        let composition = serde_json::json!({ "items": [{ "fab": { "icon": "plus" } }] });
        let refs = collect_asset_references(&composition);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "plus");
        assert_eq!(refs[0].path, "/items/0/fab/icon");
    }

    /// A call site's `{ asset: key }` argument is a reference like an icon's `name`.
    #[test]
    fn an_asset_argument_is_a_reference() {
        let composition = serde_json::json!({ "items": [{ "group": {
            "component": "metric-tile",
            "args": { "label": "WIND", "icon": { "asset": "icon-wind" } }
        } }] });
        let refs = collect_asset_references(&composition);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "icon-wind");
        assert_eq!(refs[0].path, "/items/0/group/args/icon/asset");
    }

    /// A footer menu's tab names its icon on `icon`, as a control does, and is reached once.
    #[test]
    fn a_footer_menu_tab_icon_is_a_reference() {
        let composition = serde_json::json!({ "footer_menu": {
            "event": "Navigate",
            "screens": ["today"],
            "tabs": [{ "route": "today", "label": "Today", "icon": "icon-today" }]
        } });
        let refs = collect_asset_references(&composition);

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].id, "icon-today");
        assert_eq!(refs[0].path, "/footer_menu/tabs/0/icon");
    }

    #[test]
    fn a_composition_with_no_assets_yields_none() {
        let composition = serde_json::json!({ "items": [{ "text": { "content": "hi" } }] });
        assert_eq!(collect_asset_references(&composition), Vec::new());
    }
}
