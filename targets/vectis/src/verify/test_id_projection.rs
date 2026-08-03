//! Verify canonical test-id projection and contract overlay collisions.

use std::path::Path;

use serde_json::{Value, json};

use crate::projections::test_ids::{self, GENERATED_REL};

pub const PROJECTION_STALE_ID: &str = "canonical-test-id-projection-stale";
pub const DUPLICATED_ID: &str = "canonical-test-id-duplicated";
pub const CONTRACT_FORBIDDEN_ID: &str = "canonical-test-id-contract-forbidden";

const CONTRACT_REL: &str = "contract/test-ids.yaml";

/// Emit findings when the generated registry is stale or overlaps `contract/test-ids.yaml`.
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

    let generated_path = project_root.join(GENERATED_REL);
    let on_disk = match test_ids::parse_flat_file(&generated_path) {
        Ok(entries) => entries,
        Err(err) => {
            findings.push(error_finding(
                PROJECTION_STALE_ID,
                format!("{GENERATED_REL} is invalid: {err}"),
            ));
            return findings;
        }
    };

    if expected != on_disk {
        if expected.is_empty() && !generated_path.is_file() {
            // No composition test ids and no derived file — nothing to check.
        } else {
            findings.push(error_finding(
                PROJECTION_STALE_ID,
                format!(
                    "`{GENERATED_REL}` is stale or missing; re-run `emery build` after editing \
                     composition `test_id` values"
                ),
            ));
        }
    }

    let contract_path = project_root.join(CONTRACT_REL);
    let contract = match test_ids::parse_flat_file(&contract_path) {
        Ok(entries) => entries,
        Err(err) => {
            findings
                .push(error_finding(DUPLICATED_ID, format!("{CONTRACT_REL} is invalid: {err}")));
            return findings;
        }
    };

    if is_emery_project(project_root) && !contract.is_empty() {
        findings.push(error_finding(
            CONTRACT_FORBIDDEN_ID,
            format!(
                "`{CONTRACT_REL}` must be `test_ids: {{}}` in Emery-managed product apps; \
                 author test ids as inline `test_id` in composition only"
            ),
        ));
        return findings;
    }

    for key in contract.keys() {
        if expected.contains_key(key) {
            findings.push(error_finding(
                DUPLICATED_ID,
                format!(
                    "`{key}` is declared in both composition (via `{GENERATED_REL}`) and \
                     `{CONTRACT_REL}`; product apps author test ids in composition only — leave \
                     `contract/test-ids.yaml` as `test_ids: {{}}` or remove overlapping demo keys"
                ),
            ));
        }
    }

    findings
}

fn is_emery_project(project_root: &Path) -> bool {
    project_root.join(".emery/project.yaml").is_file()
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

    fn write_emery_project(root: &Path) {
        fs::create_dir_all(root.join(".emery")).unwrap();
        fs::write(root.join(".emery/project.yaml"), "name: test\nplatforms:\n  - core\n").unwrap();
    }

    #[test]
    fn flags_stale_generated_registry() {
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
    fn clean_when_generated_matches_composition() {
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
    fn flags_contract_overlay_key_collision() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join(".emery/specs")).unwrap();
        fs::create_dir_all(root.join("contract")).unwrap();
        fs::write(
            root.join(".emery/specs/composition.yaml"),
            "screens:\n  splash:\n    name: Splash\n    body:\n      - button:\n          test_id: splash-cta\n",
        )
        .unwrap();
        test_ids::write_generated(root, None).expect("write generated");
        fs::write(root.join(CONTRACT_REL), "test_ids:\n  MAESTRO_SPLASH_CTA: splash-cta\n")
            .unwrap();

        let findings = test_id_projection_findings(root, None);
        assert!(findings.iter().filter_map(|f| f["id"].as_str()).any(|id| id == DUPLICATED_ID));
    }

    #[test]
    fn forbids_non_empty_contract_in_emery_project() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        write_emery_project(root);
        fs::create_dir_all(root.join("contract")).unwrap();
        fs::write(root.join(CONTRACT_REL), "test_ids:\n  MAESTRO_DEMO_ONLY: demo-only\n").unwrap();

        let findings = test_id_projection_findings(root, None);
        assert!(
            findings.iter().filter_map(|f| f["id"].as_str()).any(|id| id == CONTRACT_FORBIDDEN_ID)
        );
    }
}
