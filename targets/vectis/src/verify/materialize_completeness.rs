//! Detect partial template materialize (scaffold allowlist incomplete).
//!
//! Build agents sometimes copy only shell/core trees needed for `make verify`
//! and skip Maestro/canonical infra (`Makefile.toml`, `contract/`, `.maestro/`,
//! `tools/`). Once a materialized workspace has started (`Cargo.toml` +
//! `shared/`), workspace bootstrap paths must be present per `scaffold::materialize`.
//! Canonical-UI / Maestro directories (`contract/`, `.maestro/`, `tools/`) are
//! required only when a composition declares `test_id` (UI intent).

use std::path::Path;

use serde_json::{Value, json};

use crate::composition_manifests::composition_manifest_paths;

/// Root files required for any materialized workspace (UI-independent).
const ALWAYS_REQUIRED_ROOT_FILES: &[&str] = &["Makefile.toml"];

/// Root directories required for any materialized workspace (UI-independent).
const ALWAYS_REQUIRED_ROOT_DIRS: &[&str] = &["supply-chain"];

/// Canonical-UI / Maestro infra required only when the composition declares UI intent.
const UI_REQUIRED_ROOT_DIRS: &[&str] = &["contract", ".maestro", "tools"];

/// Emit findings when a materialized workspace is missing allowlisted bootstrap paths.
#[must_use]
pub fn materialize_completeness_findings(project_root: &Path) -> Vec<Value> {
    if !materialize_tree_started(project_root) {
        return Vec::new();
    }

    let mut findings = Vec::new();

    for rel in ALWAYS_REQUIRED_ROOT_FILES {
        if !project_root.join(rel).is_file() {
            findings.push(error_finding(
                "materialize-root-file-missing",
                format!(
                    "`{rel}` missing; run the allowlisted `scaffold::materialize` copy \
                     from `$TEMPLATE_DIR` (full workspace bootstrap — not per-shell partial \
                     scaffold)"
                ),
            ));
        }
    }

    for rel in ALWAYS_REQUIRED_ROOT_DIRS {
        if !project_root.join(rel).is_dir() {
            findings.push(error_finding(
                "materialize-root-dir-missing",
                format!(
                    "`{rel}/` missing; run the allowlisted `scaffold::materialize` copy \
                     from `$TEMPLATE_DIR`"
                ),
            ));
        }
    }

    if has_ui_intent(project_root) {
        for rel in UI_REQUIRED_ROOT_DIRS {
            if !project_root.join(rel).is_dir() {
                findings.push(error_finding(
                    "materialize-ui-dir-missing",
                    format!(
                        "`{rel}/` missing but the composition declares `test_id` \
                         (canonical-UI / Maestro intent); run the allowlisted \
                         `scaffold::materialize` copy so UI infra survives strip"
                    ),
                ));
            }
        }
    }

    findings
}

/// Partial materialize signal: root workspace manifest plus shared crate tree.
fn materialize_tree_started(project_root: &Path) -> bool {
    project_root.join("Cargo.toml").is_file() && project_root.join("shared").is_dir()
}

/// UI intent signal: any composition file declares at least one `test_id`.
fn has_ui_intent(project_root: &Path) -> bool {
    composition_files(project_root).iter().any(|path| file_declares_test_id(path))
}

/// Baseline + per-slice composition manifests, when present.
fn composition_files(project_root: &Path) -> Vec<std::path::PathBuf> {
    composition_manifest_paths(project_root)
}

fn file_declares_test_id(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .is_ok_and(|source| source.lines().any(|line| line.trim_start().starts_with("test_id:")))
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
    fn skipped_before_materialize_starts() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("shared/src")).unwrap();
        assert!(materialize_completeness_findings(tmp.path()).is_empty());
    }

    #[test]
    fn reports_infra_but_not_ui_dirs_without_composition() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[workspace]\nmembers = [\"shared\"]\n").unwrap();
        fs::create_dir_all(tmp.path().join("shared/src")).unwrap();

        let findings = materialize_completeness_findings(tmp.path());
        let ids: Vec<&str> = findings.iter().filter_map(|f| f["id"].as_str()).collect();

        assert!(ids.contains(&"materialize-root-file-missing"));
        assert!(ids.contains(&"materialize-root-dir-missing"));
        assert!(!ids.contains(&"materialize-ui-dir-missing"));
    }

    #[test]
    fn flags_ui_dirs_when_composition_declares_test_id() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[workspace]\nmembers = [\"shared\"]\n").unwrap();
        fs::create_dir_all(tmp.path().join("shared/src")).unwrap();
        fs::create_dir_all(tmp.path().join(".emery/specs")).unwrap();
        fs::write(
            tmp.path().join(".emery/specs/composition.yaml"),
            "screens:\n  - name: Splash\n    test_id: splash-screen\n",
        )
        .unwrap();

        let findings = materialize_completeness_findings(tmp.path());

        assert!(
            findings
                .iter()
                .filter_map(|f| f["id"].as_str())
                .any(|id| id == "materialize-ui-dir-missing")
        );
    }
}
