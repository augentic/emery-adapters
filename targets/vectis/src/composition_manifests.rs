//! Discover and merge composition manifests under a Emery project root.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::VectisError;

/// Baseline (`.emery/specs/composition.yaml`) plus slice-local manifests, sorted.
///
/// Used for coarse UI-intent scans only — not for canonical test-id projection.
#[must_use]
pub fn composition_manifest_paths(project_root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let baseline = project_root.join(".emery/specs/composition.yaml");
    if baseline.is_file() {
        paths.push(baseline);
    }

    let slices_root = project_root.join(".emery/slices");
    if slices_root.is_dir()
        && let Ok(read_dir) = std::fs::read_dir(&slices_root)
    {
        let mut slice_paths: Vec<PathBuf> = read_dir
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("composition.yaml"))
            .filter(|path| path.is_file())
            .collect();
        slice_paths.sort();
        paths.extend(slice_paths);
    }

    paths
}

/// Effective composition document for test-id projection.
///
/// When `active_slice` is set, merges that slice's `composition.yaml` onto the
/// baseline using the same screen-level semantics as the Emery engine merge.
/// When `active_slice` is `None`, only the merged baseline is used (post-merge /
/// desk verify).
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when manifests are unreadable, malformed,
/// or when a slice delta conflicts with the baseline.
pub fn effective_composition(
    project_root: &Path, active_slice: Option<&str>,
) -> Result<Value, VectisError> {
    let baseline_path = project_root.join(".emery/specs/composition.yaml");
    let baseline_text = read_optional_text(&baseline_path)?;

    let slice_text = match active_slice {
        Some(slice) => {
            let path = project_root.join(format!(".emery/slices/{slice}/composition.yaml"));
            read_optional_text(&path)?
        }
        None => None,
    };

    slice_text.map_or_else(
        || parse_baseline_document(baseline_text.as_deref()),
        |text| merge_composition_delta(baseline_text.as_deref(), &text),
    )
}

fn read_optional_text(path: &Path) -> Result<Option<String>, VectisError> {
    if !path.is_file() {
        return Ok(None);
    }
    std::fs::read_to_string(path)
        .map_err(|err| VectisError::InvalidProject {
            message: format!("{} not readable: {err}", path.display()),
        })
        .map(Some)
}

fn parse_baseline_document(baseline_text: Option<&str>) -> Result<Value, VectisError> {
    let text = baseline_text.filter(|s| !s.trim().is_empty()).unwrap_or("version: 1\nscreens: {}");
    serde_saphyr::from_str(text).map_err(|err| VectisError::InvalidProject {
        message: format!("invalid baseline composition: {err}"),
    })
}

/// Mirror `emery::slice::merge::composition::merge` screen-level semantics.
fn merge_composition_delta(
    baseline_text: Option<&str>, delta_text: &str,
) -> Result<Value, VectisError> {
    let delta_doc: Value =
        serde_saphyr::from_str(delta_text).map_err(|err| VectisError::InvalidProject {
            message: format!("invalid slice composition: {err}"),
        })?;

    let has_screens = delta_doc.get("screens").is_some();
    let has_delta = delta_doc.get("delta").is_some();

    if has_screens && !has_delta {
        return Ok(delta_doc);
    }

    if !has_delta {
        return Err(VectisError::InvalidProject {
            message: "slice composition has neither `screens` nor `delta`".into(),
        });
    }

    let mut baseline_doc = parse_baseline_document(baseline_text)?;

    let screens = baseline_doc
        .as_object_mut()
        .and_then(|doc| doc.get_mut("screens"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| VectisError::InvalidProject {
            message: "baseline has no `screens` mapping".into(),
        })?;

    let delta = delta_doc.get("delta").and_then(Value::as_object).ok_or_else(|| {
        VectisError::InvalidProject {
            message: "`delta` is not a mapping".into(),
        }
    })?;

    let mut errors = Vec::new();

    if let Some(removed) = delta.get("removed").and_then(Value::as_object) {
        for slug in removed.keys() {
            screens.remove(slug.as_str());
        }
    }

    if let Some(added) = delta.get("added").and_then(Value::as_object) {
        for (slug, screen_entry) in added {
            if screens.contains_key(slug.as_str()) {
                errors.push(format!(
                    "screen `{slug}` already exists in baseline; use `modified` to update it"
                ));
                continue;
            }
            screens.insert(slug.clone(), screen_entry.clone());
        }
    }

    if let Some(modified) = delta.get("modified").and_then(Value::as_object) {
        for (slug, screen_entry) in modified {
            if !screens.contains_key(slug.as_str()) {
                errors.push(format!(
                    "screen `{slug}` not found in baseline; use `added` for new screens"
                ));
                continue;
            }
            screens.insert(slug.clone(), screen_entry.clone());
        }
    }

    if errors.is_empty() {
        Ok(baseline_doc)
    } else {
        Err(VectisError::InvalidProject {
            message: format!("composition delta merge failed:\n- {}", errors.join("\n- ")),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn baseline_only_when_no_active_slice() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".emery/specs")).unwrap();
        fs::create_dir_all(root.join(".emery/slices/follow-up")).unwrap();
        fs::write(
            root.join(".emery/specs/composition.yaml"),
            "version: 1\nscreens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
        )
        .unwrap();
        fs::write(
            root.join(".emery/slices/follow-up/composition.yaml"),
            "version: 1\ndelta:\n  added:\n    stub:\n      name: Stub\n      body:\n        - text:\n            test_id: stub-message\n  modified: {}\n  removed: {}\n",
        )
        .unwrap();

        let doc = effective_composition(root, None).expect("baseline only");
        assert!(doc["screens"]["splash"].is_object());
        assert!(doc["screens"].get("stub").is_none());
    }

    #[test]
    fn merges_active_slice_delta_onto_baseline() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".emery/specs")).unwrap();
        fs::create_dir_all(root.join(".emery/slices/follow-up")).unwrap();
        fs::write(
            root.join(".emery/specs/composition.yaml"),
            "version: 1\nscreens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
        )
        .unwrap();
        fs::write(
            root.join(".emery/slices/follow-up/composition.yaml"),
            "version: 1\ndelta:\n  added:\n    stub:\n      name: Stub\n      body:\n        - text:\n            test_id: stub-message\n  modified: {}\n  removed: {}\n",
        )
        .unwrap();

        let doc = effective_composition(root, Some("follow-up")).expect("merged");
        assert!(doc["screens"]["splash"].is_object());
        assert!(doc["screens"]["stub"].is_object());
    }

    #[test]
    fn modified_replaces_screen_and_drops_old_test_ids() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".emery/specs")).unwrap();
        fs::create_dir_all(root.join(".emery/slices/rename")).unwrap();
        fs::write(
            root.join(".emery/specs/composition.yaml"),
            "version: 1\nscreens:\n  list:\n    name: List\n    body:\n      - button:\n          test_id: list-row\n",
        )
        .unwrap();
        fs::write(
            root.join(".emery/slices/rename/composition.yaml"),
            "version: 1\ndelta:\n  added: {}\n  modified:\n    list:\n      name: List\n      body:\n        - button:\n            test_id: list-row-updated\n  removed: {}\n",
        )
        .unwrap();

        let doc = effective_composition(root, Some("rename")).expect("modified");
        let body = &doc["screens"]["list"]["body"];
        assert_eq!(body[0]["button"]["test_id"], json!("list-row-updated"));
    }

    #[test]
    fn removed_screen_drops_test_ids() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".emery/specs")).unwrap();
        fs::create_dir_all(root.join(".emery/slices/prune")).unwrap();
        fs::write(
            root.join(".emery/specs/composition.yaml"),
            "version: 1\nscreens:\n  keep:\n    name: Keep\n    body:\n      - button:\n          test_id: keep-cta\n  drop:\n    name: Drop\n    body:\n      - button:\n          test_id: drop-cta\n",
        )
        .unwrap();
        fs::write(
            root.join(".emery/slices/prune/composition.yaml"),
            "version: 1\ndelta:\n  added: {}\n  modified: {}\n  removed:\n    drop:\n      reason: obsolete\n",
        )
        .unwrap();

        let doc = effective_composition(root, Some("prune")).expect("removed");
        assert!(doc["screens"].get("keep").is_some());
        assert!(doc["screens"].get("drop").is_none());
    }
}
