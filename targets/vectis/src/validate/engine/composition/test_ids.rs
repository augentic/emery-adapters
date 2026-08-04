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
    for_each_test_id(node, json_path, &mut |object_path, test_id| {
        let path = format!("{object_path}/test_id");
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
    });
}

/// Collect every `test_id` value in a composition sub-tree (`json_path` → kebab id).
pub fn collect_test_id_values(node: &Value, json_path: &str, out: &mut BTreeMap<String, String>) {
    for_each_test_id(node, json_path, &mut |object_path, test_id| {
        out.insert(object_path.to_string(), test_id.to_string());
    });
}

/// Derive a portable `MAESTRO_*` constant name from a kebab-case test id.
#[must_use]
pub fn kebab_to_maestro_key(value: &str) -> String {
    format!("MAESTRO_{}", value.replace('-', "_").to_uppercase())
}

fn child_pointer(json_path: &str, token: &str) -> String {
    format!("{json_path}/{}", escape_pointer_token(token))
}

fn for_each_test_id(node: &Value, json_path: &str, visit: &mut impl FnMut(&str, &str)) {
    match node {
        Value::Object(map) => {
            if let Some(test_id) = map.get("test_id").and_then(Value::as_str) {
                visit(json_path, test_id);
            }
            for (key, val) in map {
                for_each_test_id(val, &child_pointer(json_path, key), visit);
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                for_each_test_id(val, &format!("{json_path}/{i}"), visit);
            }
        }
        _ => {}
    }
}

/// Whether `value` matches `^[a-z][a-z0-9]*(-[a-z0-9]+)*$`.
#[must_use]
pub fn is_kebab_test_id(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    let mut head = first.bytes();
    let Some(start) = head.next() else {
        return false;
    };
    if !start.is_ascii_lowercase() {
        return false;
    }
    if !head.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()) {
        return false;
    }
    segments.all(|segment| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // Walk smoke tests; duplicate/format integration coverage lives in validate suites.

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
    fn kebab_grammar_matches_schema() {
        for ok in ["splash-cta", "a", "a1", "row-2", "list-row-updated"] {
            assert!(is_kebab_test_id(ok), "expected accept: {ok}");
        }
        for bad in ["", "1-foo", "9", "-a", "a-", "Splash-CTA", "splash_cta", "a--b"] {
            assert!(!is_kebab_test_id(bad), "expected reject: {bad}");
        }
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
