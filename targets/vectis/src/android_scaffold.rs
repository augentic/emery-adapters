//! Android DX path presence and BoltFFI pattern drift detection.
//!
//! Immutable DX paths match [`crate::scaffold::materialize::ANDROID_DX_RELATIVE_PATHS`].
//! Required substrings are derived from the live `vectis-template` Android Makefile
//! (BoltFFI pack). Byte-compare against an embedded template is retired — refresh
//! is host/agent-owned via [`crate::sync`] from `$TEMPLATE_DIR`. Pin faithfulness
//! for `Android/gradle/libs.versions.toml` is prompt-mandated against `$TEMPLATE_DIR`.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::VectisError;
use crate::scaffold::{default_android_package, validate_app_name};

/// Relative paths under the project root that agents must keep aligned with `$TEMPLATE_DIR`.
pub const IMMUTABLE_RELATIVE_PATHS: [&str; 5] = [
    "Android/Makefile",
    "Android/settings.gradle.kts",
    "Android/build.gradle.kts",
    "Android/app/build.gradle.kts",
    "Android/shared/build.gradle.kts",
];

/// Diagnostic id for scaffold drift findings.
pub const DRIFT_FINDING_ID: &str = "android-scaffold-file-drift";

/// Required Android Makefile substrings from live `vectis-template` BoltFFI DX.
pub const REQUIRED_MAKEFILE_PATTERNS: [&str; 1] = ["boltffi pack android"];

/// Required `:shared` Gradle substrings from live `vectis-template` (BoltFFI output layout).
pub const REQUIRED_SHARED_GRADLE_PATTERNS: [&str; 1] = ["generated/jniLibs"];

/// Compare agent-immutable Android DX files for presence and BoltFFI patterns.
#[must_use]
pub fn android_scaffold_drift_findings(project_root: &Path) -> Vec<Value> {
    let android_root = project_root.join("Android");
    if !android_root.is_dir() {
        return Vec::new();
    }

    if resolve_android_app_name(project_root).is_err() {
        return vec![drift_finding(
            "Android",
            "cannot resolve Android app name from settings.gradle.kts or Application.kt layout",
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
                         (vectis::scaffold::materialize / sync android-scaffold) — do not invent DX"
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

/// Resolve the Android app name for an on-disk shell.
///
/// # Errors
/// Returns [`VectisError::InvalidProject`] when no authoritative name is found.
pub fn resolve_android_app_name(project_root: &Path) -> Result<String, VectisError> {
    let android_root = project_root.join("Android");
    if !android_root.is_dir() {
        return Err(VectisError::InvalidProject {
            message: format!("Android shell directory not found at {}", android_root.display()),
        });
    }

    if let Some(name) = read_settings_gradle_name(&android_root.join("settings.gradle.kts"))?
        && validate_app_name(&name).is_ok()
    {
        return Ok(name);
    }

    discover_app_from_application_kt(&android_root).ok_or_else(|| VectisError::InvalidProject {
        message: "cannot resolve Android app name: set `rootProject.name` in Android/settings.gradle.kts or keep a single `{AppName}Application.kt` under Android/app/src/".into(),
    })
}

/// Resolve the Android application package for an on-disk shell.
///
/// # Errors
/// Returns [`VectisError::InvalidProject`] when no authoritative package is found.
pub fn resolve_android_package(project_root: &Path, app_name: &str) -> Result<String, VectisError> {
    let app_build = project_root.join("Android/app/build.gradle.kts");
    if let Some(package) = read_gradle_assignment(&app_build, "namespace")? {
        return Ok(package);
    }
    if let Some(package) = read_gradle_assignment(&app_build, "applicationId")? {
        return Ok(package);
    }
    Ok(default_android_package(app_name))
}

fn pattern_finding(relative_path: &str, on_disk: &str) -> Option<Value> {
    if relative_path == "Android/Makefile" {
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
    if relative_path == "Android/shared/build.gradle.kts" {
        for pattern in REQUIRED_SHARED_GRADLE_PATTERNS {
            if !on_disk.contains(pattern) {
                return Some(drift_finding(
                    relative_path,
                    &format!(
                        "{relative_path} is missing required BoltFFI output path `{pattern}`; \
                         re-copy from $TEMPLATE_DIR — do not invent Gradle content"
                    ),
                ));
            }
        }
    }
    None
}

fn read_settings_gradle_name(settings_gradle: &Path) -> Result<Option<String>, VectisError> {
    read_gradle_assignment(settings_gradle, "rootProject.name")
}

fn read_gradle_assignment(path: &Path, key: &str) -> Result<Option<String>, VectisError> {
    if !path.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(path).map_err(|err| map_io(&err))?;
    let prefix = format!("{key} =");
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&prefix) {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Ok(Some(value.to_string()));
            }
        }
    }
    Ok(None)
}

fn discover_app_from_application_kt(android_root: &Path) -> Option<String> {
    let app_java = android_root.join("app/src/main/java");
    let mut candidates: Vec<String> = Vec::new();
    collect_application_kt_names(&app_java, &mut candidates);
    candidates.sort();
    candidates.dedup();
    match candidates.as_slice() {
        [single] if validate_app_name(single).is_ok() => Some(single.clone()),
        _ => None,
    }
}

fn collect_application_kt_names(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_application_kt_names(&path, out);
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(app_name) = name.strip_suffix("Application.kt") else {
            continue;
        };
        if !app_name.is_empty() {
            out.push(app_name.to_string());
        }
    }
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
