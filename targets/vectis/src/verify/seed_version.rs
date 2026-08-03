//! Domain-neutral `ui-contract/seed.yaml` version gate.
//!
//! Shape validation (field names, types, required keys) belongs in the core:
//! product apps define a `Deserialize` seed type and a `cargo test` that parses
//! `SEED_YAML`. In-guest verify cannot compile the core, so only `version` is
//! checked here.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

/// Finding id when `ui-contract/seed.yaml` declares an unsupported `version`.
pub const SEED_VERSION_FINDING_ID: &str = "canonical-seed-version";

/// Emit findings when `ui-contract/seed.yaml` declares an unsupported `version`.
///
/// Skips silently when the file is absent or whitespace-only.
#[must_use]
pub fn seed_version_findings(project_root: &Path) -> Vec<Value> {
    let Ok(text) = fs::read_to_string(project_root.join("ui-contract/seed.yaml")) else {
        return Vec::new();
    };

    if text.trim().is_empty() {
        return Vec::new();
    }

    if seed_version_ok(&text) {
        return Vec::new();
    }

    vec![error_finding(SEED_VERSION_FINDING_ID, "ui-contract/seed.yaml `version` must be 1")]
}

fn error_finding(id: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "severity": "error",
        "source": "deterministic",
        "path": "ui-contract/seed.yaml",
        "message": message.into(),
    })
}

/// True when a top-level `version:` line declares exactly `1`.
fn seed_version_ok(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_end();
        line.strip_prefix("version:").is_some_and(|rest| rest.trim() == "1")
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    fn write_seed(root: &Path, body: &str) {
        let path = root.join("ui-contract/seed.yaml");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn has_finding(findings: &[Value], id: &str) -> bool {
        findings.iter().filter_map(|f| f["id"].as_str()).any(|found| found == id)
    }

    #[test]
    fn absent_seed_is_skipped() {
        let tmp = tempdir().unwrap();
        assert!(seed_version_findings(tmp.path()).is_empty());
    }

    #[test]
    fn neutral_scaffold_is_ok() {
        let tmp = tempdir().unwrap();
        write_seed(tmp.path(), "version: 1\n");
        assert!(seed_version_findings(tmp.path()).is_empty());
    }

    #[test]
    fn app_domain_payload_with_version_one_is_ok() {
        let tmp = tempdir().unwrap();
        write_seed(
            tmp.path(),
            "version: 1\n\
             items:\n  - id: a\n    title: Example\n",
        );
        assert!(seed_version_findings(tmp.path()).is_empty());
    }

    #[test]
    fn wrong_version_is_flagged() {
        let tmp = tempdir().unwrap();
        write_seed(tmp.path(), "version: 2\n");
        let findings = seed_version_findings(tmp.path());
        assert!(has_finding(&findings, SEED_VERSION_FINDING_ID));
    }

    #[test]
    fn missing_version_is_flagged() {
        let tmp = tempdir().unwrap();
        write_seed(tmp.path(), "items:\n  - id: a\n");
        let findings = seed_version_findings(tmp.path());
        assert!(has_finding(&findings, SEED_VERSION_FINDING_ID));
    }
}
