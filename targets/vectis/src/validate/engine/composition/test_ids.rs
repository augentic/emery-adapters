//! `test_id` collection and uniqueness enforcement.
//!
//! Shared by `validate layout`, `validate composition`, and test-id projection.

use std::collections::BTreeMap;

use serde_json::Value;

use super::finding::Finding;
use crate::validate::engine::shared::escape_pointer_token;

/// Validate every `test_id` in a composition sub-tree: kebab format and
/// document-wide uniqueness.
pub fn check_test_ids(node: &Value, json_path: &str, errors: &mut Vec<Finding>) {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    walk_test_ids(node, json_path, &mut seen, errors);
}

/// Collect every `test_id` value in a composition sub-tree (`json_path` → kebab id).
pub fn collect_test_id_values(node: &Value, json_path: &str, out: &mut BTreeMap<String, String>) {
    match node {
        Value::Object(map) => {
            if let Some(test_id) = map.get("test_id").and_then(Value::as_str) {
                out.insert(json_path.to_string(), test_id.to_string());
            }
            for (key, val) in map {
                let child_path = if json_path.is_empty() {
                    format!("/{key}")
                } else {
                    format!("{json_path}/{}", escape_pointer_token(key))
                };
                collect_test_id_values(val, &child_path, out);
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                collect_test_id_values(val, &format!("{json_path}/{i}"), out);
            }
        }
        _ => {}
    }
}

/// Derive a portable `MAESTRO_*` constant name from a kebab-case test id.
#[must_use]
pub fn kebab_to_maestro_key(value: &str) -> String {
    format!("MAESTRO_{}", value.replace('-', "_").to_uppercase())
}

fn walk_test_ids(
    node: &Value, json_path: &str, seen: &mut BTreeMap<String, String>, errors: &mut Vec<Finding>,
) {
    match node {
        Value::Object(map) => {
            if let Some(test_id) = map.get("test_id").and_then(Value::as_str) {
                let path = format!("{json_path}/test_id");
                if !is_kebab_test_id(test_id) {
                    errors.push(Finding::new(
                        path,
                        format!(
                            "`test_id` value `{test_id}` must match `[a-z][a-z0-9]*(-[a-z0-9]+)*`"
                        ),
                    ));
                } else if let Some(first_path) = seen.get(test_id) {
                    errors.push(Finding::new(
                        path,
                        format!(
                            "duplicate `test_id` `{test_id}` (also at {first_path}); test ids must be unique within the document"
                        ),
                    ));
                } else {
                    seen.insert(test_id.to_string(), path);
                }
            }

            for (key, val) in map {
                let child_path = format!("{json_path}/{}", escape_pointer_token(key));
                walk_test_ids(val, &child_path, seen, errors);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                walk_test_ids(v, &format!("{json_path}/{i}"), seen, errors);
            }
        }
        _ => {}
    }
}

/// Whether `value` matches the portable kebab-case test-id grammar.
#[must_use]
pub fn is_kebab_test_id(value: &str) -> bool {
    !value.is_empty()
        && value.split('-').all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_unique_kebab_test_ids() {
        let doc = json!({
            "screens": {
                "splash": {
                    "name": "Splash",
                    "body": [
                        {
                            "button": {
                                "label": "Go",
                                "test_id": "splash-cta"
                            }
                        }
                    ]
                }
            }
        });

        let mut errors = Vec::new();
        check_test_ids(&doc, "", &mut errors);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn rejects_duplicate_test_ids() {
        let doc = json!({
            "screens": {
                "a": {
                    "name": "A",
                    "body": [{ "button": { "test_id": "same-id" } }]
                },
                "b": {
                    "name": "B",
                    "body": [{ "button": { "test_id": "same-id" } }]
                }
            }
        });

        let mut errors = Vec::new();
        check_test_ids(&doc, "", &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("duplicate"));
    }

    #[test]
    fn rejects_non_kebab_test_ids() {
        let doc = json!({
            "screens": {
                "splash": {
                    "name": "Splash",
                    "body": [{ "button": { "test_id": "Splash_CTA" } }]
                }
            }
        });

        let mut errors = Vec::new();
        check_test_ids(&doc, "", &mut errors);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].message.contains("must match"));
    }
}
