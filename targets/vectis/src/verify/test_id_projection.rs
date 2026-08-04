//! Verify `ui-contract/test-ids.yaml` matches the effective composition projection.

use std::path::Path;

use serde_json::{Value, json};

use crate::projections::test_id_registry::{self, REGISTRY_REL};

pub const PROJECTION_STALE_ID: &str = "canonical-test-id-projection-stale";

/// Emit findings when `ui-contract/test-ids.yaml` is stale relative to composition.
#[must_use]
pub fn test_id_projection_findings(project_root: &Path, active_slice: Option<&str>) -> Vec<Value> {
    let mut findings = Vec::new();

    let expected = match test_id_registry::harvest_entries(project_root, active_slice) {
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
    let on_disk = match test_id_registry::parse_flat_file(&registry_path) {
        Ok(entries) => entries,
        Err(err) => {
            findings.push(error_finding(
                PROJECTION_STALE_ID,
                format!("{REGISTRY_REL} is invalid: {err}"),
            ));
            return findings;
        }
    };

    // No composition test ids and no registry file — nothing to check.
    let nothing_to_check = expected.is_empty() && !registry_path.is_file();
    if expected != on_disk && !nothing_to_check {
        findings.push(error_finding(
            PROJECTION_STALE_ID,
            format!(
                "`{REGISTRY_REL}` is stale or missing; re-run `emery build` after editing \
                 composition `test_id` values"
            ),
        ));
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
