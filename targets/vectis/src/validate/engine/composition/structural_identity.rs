//! Structural-identity engine for `component:` directives.
//!
//! Skeleton normalisation, the base-instance identity rule (shared with
//! `validate layout`), and the content-addressed fingerprint the
//! `infer` verb keys clusters on.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::finding::Finding;
use crate::validate::engine::shared::escape_pointer_token;

struct ComponentInstance {
    slug: String,
    skeleton: Skeleton,
    path: String,
    in_platform_override: bool,
}

/// Normalised structural skeleton for a group's children.
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Skeleton {
    /// A leaf item identified by its single property key (e.g.
    /// `text`, `checkbox`); item properties are deliberately ignored.
    Item(String),
    /// A group: ordered children plus the `*-when` keys present on the
    /// group props.
    Group {
        /// Sorted, deduplicated `*-when` key names present on the group.
        when_keys: Vec<String>,
        /// Ordered child skeletons.
        items: Vec<Self>,
    },
}

/// Validate the structural-identity rule for every `component: <slug>`
/// directive in a sub-tree. Shared by layout and composition modes.
pub fn check_structural_identity(node: &Value, json_path: &str, errors: &mut Vec<Finding>) {
    let mut instances: Vec<ComponentInstance> = Vec::new();
    walk_for_components(node, json_path, false, &mut instances);

    let mut by_slug: BTreeMap<String, Vec<&ComponentInstance>> = BTreeMap::new();
    for inst in &instances {
        by_slug.entry(inst.slug.clone()).or_default().push(inst);
    }

    for (slug, group) in by_slug {
        // Identity is enforced across base instances only;
        // `platforms.*` overrides MAY diverge.
        let base: Vec<&ComponentInstance> =
            group.iter().filter(|i| !i.in_platform_override).copied().collect();
        if base.len() < 2 {
            continue;
        }
        let canonical = base[0];
        for other in base.iter().skip(1) {
            if other.skeleton != canonical.skeleton {
                errors.push(Finding::new(
                    other.path.clone(),
                    format!(
                        "component slug `{slug}` has a different skeleton at {} than the canonical instance at {} (structural-identity rule); slug instances may differ in `bind`, `event`, `error`, asset / token references, `*-when` condition values, and free text content but their group skeleton MUST match across all base instances",
                        other.path,
                        canonical.path,
                    ),
                ));
            }
        }
    }
}

fn walk_for_components(
    node: &Value, json_path: &str, in_platform: bool, out: &mut Vec<ComponentInstance>,
) {
    match node {
        Value::Object(map) => {
            for (key, val) in map {
                let child_path = format!("{json_path}/{}", escape_pointer_token(key));
                let descend_in_platform = in_platform || key == "platforms";
                if key == "group"
                    && let Some(component) = val.get("component").and_then(Value::as_str)
                {
                    out.push(ComponentInstance {
                        slug: component.to_string(),
                        skeleton: build_group_skeleton(val),
                        path: child_path.clone(),
                        in_platform_override: in_platform,
                    });
                }
                walk_for_components(val, &child_path, descend_in_platform, out);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                walk_for_components(v, &format!("{json_path}/{i}"), in_platform, out);
            }
        }
        _ => {}
    }
}

/// Build a [`Skeleton::Group`] from a `groupProps` JSON value.
#[must_use]
pub fn build_group_skeleton(group_props: &Value) -> Skeleton {
    let mut when_keys: Vec<String> = group_props
        .as_object()
        .map(|m| m.keys().filter(|k| k.ends_with("-when") && k.len() > 5).cloned().collect())
        .unwrap_or_default();
    when_keys.sort();
    when_keys.dedup();

    let items: Vec<Skeleton> = group_props
        .get("items")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().map(build_node_skeleton).collect())
        .unwrap_or_default();

    Skeleton::Group { when_keys, items }
}

/// Build a skeleton fragment for a single `contentNode`: a nested
/// `{ group: ... }` recurses via [`build_group_skeleton`]; an item
/// keeps only its kind key (itemProps are wiring, ignored).
#[must_use]
pub fn build_node_skeleton(node: &Value) -> Skeleton {
    let Some(map) = node.as_object() else {
        return Skeleton::Item(String::from("<unknown>"));
    };
    let (Some((key, val)), 1) = (map.iter().next(), map.len()) else {
        return Skeleton::Item(String::from("<unknown>"));
    };
    if key == "group" { build_group_skeleton(val) } else { Skeleton::Item(key.clone()) }
}

/// Content-addressed fingerprint over a normalised [`Skeleton`]:
#[must_use]
pub fn fingerprint(skeleton: &Skeleton) -> String {
    let mut canonical = String::new();
    encode_skeleton(skeleton, &mut canonical);
    let digest = Sha256::digest(canonical.as_bytes());
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn encode_skeleton(skeleton: &Skeleton, buf: &mut String) {
    match skeleton {
        Skeleton::Item(kind) => {
            buf.push_str("I:");
            buf.push_str(kind);
            buf.push(';');
        }
        Skeleton::Group { when_keys, items } => {
            buf.push_str("G[");
            buf.push_str(&when_keys.join(","));
            buf.push_str("](");
            for item in items {
                encode_skeleton(item, buf);
            }
            buf.push_str(");");
        }
    }
}

/// Project a [`Skeleton`] into the name-free JSON fragment the
/// `infer` report carries as the cluster's representative skeleton.
#[must_use]
pub fn skeleton_to_json(skeleton: &Skeleton) -> Value {
    match skeleton {
        Skeleton::Item(kind) => json!({ "item": kind }),
        Skeleton::Group { when_keys, items } => json!({
            "group": {
                "when_keys": when_keys,
                "items": items.iter().map(skeleton_to_json).collect::<Vec<_>>(),
            }
        }),
    }
}

// Collapse (dense private matrix): the fingerprint equivalence table
// (wiring ignored, cardinality and `*-when` keys significant) is pure
// and dense; the operator-visible identity rule is owned by
// `tests/validate.rs` via the public `validate::run`.
#[cfg(test)]
mod tests {
    use serde_json::Map;

    use super::*;

    fn group(items: Value) -> Value {
        let mut map = Map::new();
        map.insert("items".to_string(), items);
        Value::Object(map)
    }

    // Bind/event wiring is ignored by fingerprinting.
    #[test]
    fn fingerprint_and_skeleton() {
        let skeleton = build_group_skeleton(&group(json!([
            { "icon-button": { "bind": "home", "event": "Navigate(Home)" } },
            { "icon-button": { "bind": "search", "event": "Navigate(Search)" } },
        ])));
        assert_eq!(fingerprint(&skeleton), fingerprint(&skeleton));

        let rewired = build_group_skeleton(&group(json!([
            { "icon-button": { "bind": "profile", "event": "Navigate(Profile)" } },
            { "icon-button": { "bind": "inbox", "event": "Navigate(Inbox)" } },
        ])));
        assert_eq!(fingerprint(&skeleton), fingerprint(&rewired));

        let two =
            build_group_skeleton(&group(json!([ { "icon-button": {} }, { "icon-button": {} } ])));
        let three = build_group_skeleton(&group(json!([
            { "icon-button": {} },
            { "icon-button": {} },
            { "icon-button": {} },
        ])));
        assert_ne!(fingerprint(&two), fingerprint(&three));

        let bare = build_group_skeleton(&json!({ "items": [ { "text": {} } ] }));
        let conditional =
            build_group_skeleton(&json!({ "active-when": "$x", "items": [ { "text": {} } ] }));
        assert_ne!(fingerprint(&bare), fingerprint(&conditional));

        let nested = build_group_skeleton(&json!({
            "active-when": "$route",
            "items": [
                { "icon-button": {} },
                { "group": { "items": [ { "text": {} } ] } },
            ],
        }));
        let projected = skeleton_to_json(&nested);
        assert_eq!(projected["group"]["when_keys"], json!(["active-when"]));
        assert_eq!(projected["group"]["items"][0], json!({ "item": "icon-button" }));
        assert_eq!(
            projected["group"]["items"][1],
            json!({ "group": { "when_keys": [], "items": [ { "item": "text" } ] } })
        );
    }
}
