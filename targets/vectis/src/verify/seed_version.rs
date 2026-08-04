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
    use super::*;

    #[test]
    fn seed_version_ok_accepts_version_one() {
        assert!(seed_version_ok("version: 1\n"));
        assert!(seed_version_ok("version: 1\nitems:\n  - id: a\n    title: Example\n"));
    }

    #[test]
    fn seed_version_ok_rejects_wrong_or_missing_version() {
        assert!(!seed_version_ok("version: 2\n"));
        assert!(!seed_version_ok("items:\n  - id: a\n"));
    }
}
