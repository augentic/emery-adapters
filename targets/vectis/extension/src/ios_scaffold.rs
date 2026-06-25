//! iOS agent-immutable scaffold file sync and drift detection.
//!
//! `iOS/Makefile` and `iOS/project.yml` are rendered exclusively from the
//! embedded scaffold templates. Prepare overwrites drift before agent work;
//! verify emits blocking findings when on-disk bytes diverge.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::VectisError;
use crate::scaffold::{Versions, default_android_package, plan_ios, validate_app_name};

/// Relative paths under the project root that agents must never edit.
pub const IMMUTABLE_RELATIVE_PATHS: [&str; 2] = ["iOS/Makefile", "iOS/project.yml"];

/// Diagnostic id for scaffold drift findings.
pub const DRIFT_FINDING_ID: &str = "ios-scaffold-file-drift";

/// Required `sim-build` destination literal in the Makefile template.
pub const REQUIRED_SIM_DESTINATION: &str = "generic/platform=iOS Simulator";

/// JSON fragment for `scaffold_sync.ios` in prepare and sync command output.
#[must_use]
pub fn scaffold_sync_ios_json(report: &IosScaffoldSyncReport) -> Value {
    json!({
        "ios": {
            "synced": &report.synced,
            "unchanged": &report.unchanged,
        }
    })
}

/// Outcome of a prepare-time scaffold sync pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IosScaffoldSyncReport {
    /// Paths rewritten because on-disk content differed from the template.
    pub synced: Vec<String>,
    /// Paths already matching the template (no write performed).
    pub unchanged: Vec<String>,
}

/// Re-render and overwrite agent-immutable iOS scaffold files when `iOS/` exists.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the app name cannot be
/// resolved or a file write fails.
pub fn sync_ios_scaffold_files(project_root: &Path) -> Result<IosScaffoldSyncReport, VectisError> {
    let ios_root = project_root.join("iOS");
    if !ios_root.is_dir() {
        return Ok(IosScaffoldSyncReport {
            synced: Vec::new(),
            unchanged: Vec::new(),
        });
    }

    let app_name = resolve_ios_app_name(project_root)?;
    let expected = expected_immutable_files(&app_name)?;
    let mut synced = Vec::new();
    let mut unchanged = Vec::new();

    for file in expected {
        let target = project_root.join(&file.relative_path);
        if target.is_file()
            && fs::read_to_string(&target).map_err(|err| map_io(&err))? == file.contents
        {
            unchanged.push(file.relative_path);
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| map_io(&err))?;
        }
        fs::write(&target, &file.contents).map_err(|err| map_io(&err))?;
        synced.push(file.relative_path);
    }

    Ok(IosScaffoldSyncReport { synced, unchanged })
}

/// Compare agent-immutable iOS scaffold files against the embedded templates.
///
/// Returns diagnostic-shaped findings (`severity: error`) for each drifted file.
#[must_use]
pub fn ios_scaffold_drift_findings(project_root: &Path) -> Vec<Value> {
    let ios_root = project_root.join("iOS");
    if !ios_root.is_dir() {
        return Vec::new();
    }

    let Ok(app_name) = resolve_ios_app_name(project_root) else {
        return vec![drift_finding(
            "iOS",
            "cannot resolve iOS app name from project.yml or Swift source layout",
        )];
    };

    let Ok(expected) = expected_immutable_files(&app_name) else {
        return vec![drift_finding("iOS", "failed to render expected iOS scaffold files")];
    };

    expected
        .into_iter()
        .filter_map(|file| {
            let target = project_root.join(&file.relative_path);
            let relative_path = file.relative_path;
            if !target.is_file() {
                return Some(drift_finding(
                    &relative_path,
                    &missing_scaffold_message(&relative_path),
                ));
            }
            let Ok(on_disk) = fs::read_to_string(&target) else {
                return Some(drift_finding(
                    &relative_path,
                    &format!(
                        "{relative_path} is not readable UTF-8 text; CLI-owned scaffold files must match the embedded template — run `vectis sync ios-scaffold` or `specify slice build --phase prepare`"
                    ),
                ));
            };
            if on_disk == file.contents {
                return None;
            }
            Some(drift_finding(&relative_path, &drift_message(&relative_path, &on_disk)))
        })
        .collect()
}

/// Resolve the Xcode app / scheme name for an on-disk iOS shell.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when no authoritative name is found.
pub fn resolve_ios_app_name(project_root: &Path) -> Result<String, VectisError> {
    let ios_root = project_root.join("iOS");
    if !ios_root.is_dir() {
        return Err(VectisError::InvalidProject {
            message: format!("iOS shell directory not found at {}", ios_root.display()),
        });
    }

    if let Some(name) = read_project_yml_name(&ios_root.join("project.yml"))? {
        validate_app_name(&name)?;
        return Ok(name);
    }

    discover_app_from_swift_dirs(&ios_root).ok_or_else(|| VectisError::InvalidProject {
        message: "cannot resolve iOS app name: set `name:` in iOS/project.yml or keep a single PascalCase app folder with Swift sources under iOS/".into(),
    })
}

fn expected_immutable_files(
    app_name: &str,
) -> Result<Vec<crate::scaffold::PlannedFile>, VectisError> {
    let versions = Versions::embedded()?;
    let android_package = default_android_package(app_name);
    let plan = plan_ios(app_name, &android_package, &[], &versions)?;
    Ok(plan
        .files
        .into_iter()
        .filter(|file| IMMUTABLE_RELATIVE_PATHS.contains(&file.relative_path.as_str()))
        .collect())
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

fn drift_message(relative_path: &str, on_disk: &str) -> String {
    let mut message = format!(
        "{relative_path} diverges from the embedded iOS scaffold template; agents must not edit this file — run `vectis sync ios-scaffold` or `specify slice build --phase prepare`"
    );
    if relative_path.ends_with("Makefile") {
        if on_disk.contains("name=iPhone") || on_disk.contains("platform=iOS Simulator,name=") {
            let _ = write!(
                message,
                " (forbidden named simulator destination; required: '{REQUIRED_SIM_DESTINATION}')"
            );
        } else if !on_disk.contains(REQUIRED_SIM_DESTINATION) {
            let _ =
                write!(message, " (sim-build must use -destination '{REQUIRED_SIM_DESTINATION}')");
        }
    }
    message
}

fn missing_scaffold_message(relative_path: &str) -> String {
    format!(
        "{relative_path} is missing; CLI-owned scaffold files must be present — run `vectis sync ios-scaffold` or `specify slice build --phase prepare`"
    )
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
