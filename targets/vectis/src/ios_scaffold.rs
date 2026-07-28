//! iOS DX path presence and `BoltFFI` pattern drift detection.
//!
//! Immutable DX paths match [`crate::scaffold::materialize::IOS_DX_RELATIVE_PATHS`].
//! Required substrings are derived from the live `vectis-exemplar` iOS Makefile /
//! `project.yml` (`BoltFFI` pack + generic simulator destination). Byte-compare
//! against an embedded template is retired — refresh is host/agent-owned via
//! [`crate::sync`] from `$TEMPLATE_DIR`. Pin faithfulness for workspace
//! `Cargo.toml` / `shared/boltffi.toml` is prompt-mandated against `$TEMPLATE_DIR`
//! (the guest cannot see a sibling checkout).

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::VectisError;
use crate::scaffold::validate_app_name;

/// Relative paths under the project root that agents must keep aligned with `$TEMPLATE_DIR`.
pub const IMMUTABLE_RELATIVE_PATHS: [&str; 2] = ["iOS/Makefile", "iOS/project.yml"];

/// Diagnostic id for scaffold drift findings.
pub const DRIFT_FINDING_ID: &str = "ios-scaffold-file-drift";

/// Required iOS Makefile substrings from live `vectis-exemplar` `BoltFFI` DX.
pub const REQUIRED_MAKEFILE_PATTERNS: [&str; 2] =
    ["DESTINATION ?= generic/platform=iOS Simulator", "boltffi pack apple"];

/// Required `project.yml` substring from live `vectis-exemplar` (`BoltFFI` SPM layout).
pub const REQUIRED_PROJECT_YML_PATTERNS: [&str; 1] = ["path: ./generated/Shared"];

/// Compare agent-immutable iOS DX files for presence and `BoltFFI` patterns.
#[must_use]
pub fn ios_scaffold_drift_findings(project_root: &Path) -> Vec<Value> {
    let ios_root = project_root.join("iOS");
    if !ios_root.is_dir() {
        return Vec::new();
    }

    if resolve_ios_app_name(project_root).is_err() {
        return vec![drift_finding(
            "iOS",
            "cannot resolve iOS app name from project.yml or Swift source layout",
        )];
    }

    IMMUTABLE_RELATIVE_PATHS
        .iter()
        .filter_map(|relative_path| {
            let target = project_root.join(relative_path);
            if !target.is_file() {
                return Some(drift_finding(
                    relative_path,
                    &format!(
                        "{relative_path} is missing; re-copy from $TEMPLATE_DIR \
                         (vectis::scaffold::materialize / sync ios-scaffold) — do not invent DX"
                    ),
                ));
            }
            match fs::read_to_string(&target) {
                Ok(on_disk) => pattern_finding(relative_path, &on_disk),
                Err(err) => Some(drift_finding(
                    relative_path,
                    &format!("{relative_path} could not be read ({err})"),
                )),
            }
        })
        .collect()
}

/// Resolve the Xcode app / scheme name for an on-disk iOS shell.
///
/// # Errors
/// Returns [`VectisError::InvalidProject`] when no authoritative name is found.
pub fn resolve_ios_app_name(project_root: &Path) -> Result<String, VectisError> {
    let ios_root = project_root.join("iOS");
    if !ios_root.is_dir() {
        return Err(VectisError::InvalidProject {
            message: format!("iOS shell directory not found at {}", ios_root.display()),
        });
    }

    if let Some(name) = read_project_yml_name(&ios_root.join("project.yml"))?
        && validate_app_name(&name).is_ok()
    {
        return Ok(name);
    }

    discover_app_from_swift_dirs(&ios_root).ok_or_else(|| VectisError::InvalidProject {
        message: "cannot resolve iOS app name: set `name:` in iOS/project.yml or keep a single PascalCase app folder with Swift sources under iOS/".into(),
    })
}

fn pattern_finding(relative_path: &str, on_disk: &str) -> Option<Value> {
    if relative_path.ends_with("Makefile") {
        if on_disk.contains("name=iPhone") || on_disk.contains("platform=iOS Simulator,name=") {
            return Some(drift_finding(
                relative_path,
                &format!(
                    "{relative_path} uses a forbidden named simulator destination; \
                     use `DESTINATION ?= generic/platform=iOS Simulator` from $TEMPLATE_DIR"
                ),
            ));
        }
        for pattern in REQUIRED_MAKEFILE_PATTERNS {
            if !on_disk.contains(pattern) {
                return Some(drift_finding(
                    relative_path,
                    &format!(
                        "{relative_path} is missing required BoltFFI DX pattern `{pattern}`; \
                         re-copy from $TEMPLATE_DIR — do not invent Makefile content"
                    ),
                ));
            }
        }
    }
    if relative_path.ends_with("project.yml") {
        if on_disk.contains("OTHER_LDFLAGS") && on_disk.contains("-w") {
            return Some(drift_finding(
                relative_path,
                &format!(
                    "{relative_path} forbids linker warning suppression via OTHER_LDFLAGS -w; \
                     remove OTHER_LDFLAGS and re-copy from $TEMPLATE_DIR if DX drifted"
                ),
            ));
        }
        for pattern in REQUIRED_PROJECT_YML_PATTERNS {
            if !on_disk.contains(pattern) {
                return Some(drift_finding(
                    relative_path,
                    &format!(
                        "{relative_path} is missing required BoltFFI package path `{pattern}`; \
                         re-copy from $TEMPLATE_DIR — do not invent project.yml content"
                    ),
                ));
            }
        }
    }
    None
}

fn read_project_yml_name(project_yml: &Path) -> Result<Option<String>, VectisError> {
    if !project_yml.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(project_yml).map_err(|err| map_io(&err))?;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name:") {
            let name = rest.trim().trim_matches('"').trim_matches('\'');
            if !name.is_empty() {
                return Ok(Some(name.to_string()));
            }
        }
    }
    Ok(None)
}

fn discover_app_from_swift_dirs(ios_root: &Path) -> Option<String> {
    let mut candidates: Vec<String> = Vec::new();
    let entries = fs::read_dir(ios_root).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "generated" || validate_app_name(&name).is_err() {
            continue;
        }
        if dir_contains_swift(&path) {
            candidates.push(name);
        }
    }
    candidates.sort();
    match candidates.as_slice() {
        [single] => Some(single.clone()),
        _ => None,
    }
}

fn dir_contains_swift(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && dir_contains_swift(&path) {
            return true;
        }
        if path.extension().is_some_and(|ext| ext == "swift") {
            return true;
        }
    }
    false
}

fn drift_finding(path: &str, message: &str) -> Value {
    json!({
        "id": DRIFT_FINDING_ID,
        "severity": "error",
        "source": "deterministic",
        "path": path,
        "message": message,
    })
}

fn map_io(err: &std::io::Error) -> VectisError {
    VectisError::InvalidProject {
        message: err.to_string(),
    }
}
