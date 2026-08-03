//! Verify `ui-contract/test-ids.yaml` matches the effective composition projection.

use std::path::Path;

use serde_json::{Value, json};

use crate::projections::test_ids::{self, REGISTRY_REL};

pub const PROJECTION_STALE_ID: &str = "canonical-test-id-projection-stale";

/// Emit findings when `ui-contract/test-ids.yaml` is stale relative to composition.
#[must_use]
pub fn test_id_projection_findings(project_root: &Path, active_slice: Option<&str>) -> Vec<Value> {
    let mut findings = Vec::new();

    let expected = match test_ids::harvest_entries(project_root, active_slice) {
        Ok(entries) => entries,
        Err(err) => {
            findings.push(error_finding(
                PROJECTION_STALE_ID,
                format!("could not harvest composition test ids: {err}"),
            ));
            return findings;
        }
    };

    let registry_path = project_root.join(REGISTRY_REL);
    let on_disk = match test_ids::parse_flat_file(&registry_path) {
        Ok(entries) => entries,
        Err(err) => {
            findings.push(error_finding(
                PROJECTION_STALE_ID,
                format!("{REGISTRY_REL} is invalid: {err}"),
            ));
            return findings;
        }
    };

    if expected != on_disk {
        if expected.is_empty() && !registry_path.is_file() {
            // No composition test ids and no registry file — nothing to check.
        } else {
            findings.push(error_finding(
                PROJECTION_STALE_ID,
                format!(
                    "`{REGISTRY_REL}` is stale or missing; re-run `emery build` after editing \
                     composition `test_id` values"
                ),
            ));
        }
    }

    findings
}

fn error_finding(id: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "severity": "error",
        "source": "deterministic",
        "message": message.into(),
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn flags_stale_registry() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".emery/specs")).unwrap();
        fs::write(
            root.join(".emery/specs/composition.yaml"),
            "screens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
        )
        .unwrap();

        let findings = test_id_projection_findings(root, None);
        assert!(
            findings.iter().filter_map(|f| f["id"].as_str()).any(|id| id == PROJECTION_STALE_ID)
        );
    }

    #[test]
    fn clean_when_registry_matches_composition() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".emery/specs")).unwrap();
        fs::write(
            root.join(".emery/specs/composition.yaml"),
            "screens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
        )
        .unwrap();
        test_ids::write_generated(root, None).expect("write generated");

        let findings = test_id_projection_findings(root, None);
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn clean_when_composition_and_registry_are_empty() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("ui-contract")).unwrap();
        fs::write(root.join(REGISTRY_REL), "test_ids: {}\n").unwrap();

        let findings = test_id_projection_findings(root, None);
        assert!(findings.is_empty(), "{findings:?}");
    }
}
