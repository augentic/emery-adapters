//! Android agent-immutable scaffold file sync and drift detection.
//!
//! The [`IMMUTABLE_RELATIVE_PATHS`] files are rendered exclusively from
//! the embedded scaffold templates. Verify emits blocking findings when
//! on-disk bytes diverge; [`sync_android_scaffold_files`] repairs drift
//! without prepare side effects.

use std::fmt::Write;
use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::VectisError;
use crate::scaffold::{Versions, default_android_package, plan_android, validate_app_name};

/// Relative paths under the project root that agents must never edit.
pub const IMMUTABLE_RELATIVE_PATHS: [&str; 5] = [
    "Android/Makefile",
    "Android/settings.gradle.kts",
    "Android/build.gradle.kts",
    "Android/app/build.gradle.kts",
    "Android/shared/build.gradle.kts",
];

/// Diagnostic id for scaffold drift findings.
pub const DRIFT_FINDING_ID: &str = "android-scaffold-file-drift";

/// Required Kotlin compiler setting in CLI-owned Gradle files.
pub const REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS: &str = "allWarningsAsErrors = true";

/// Required Java compiler flag in CLI-owned Gradle files.
pub const REQUIRED_JAVA_COMPILE_WERROR: &str = "-Werror";

/// Required strict Rust flags in the CLI-owned Android Makefile.
pub const REQUIRED_MAKEFILE_RUSTFLAGS: &str = "RUSTFLAGS=\"-D warnings\"";

/// JSON fragment for `scaffold_sync.android` in sync command output.
#[must_use]
pub fn scaffold_sync_android_json(report: &AndroidScaffoldSyncReport) -> Value {
    json!({
        "android": {
            "synced": &report.synced,
            "unchanged": &report.unchanged,
        }
    })
}

/// Outcome of an android-scaffold sync pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AndroidScaffoldSyncReport {
    /// Paths rewritten because on-disk content differed from the template.
    pub synced: Vec<String>,
    /// Paths already matching the template (no write performed).
    pub unchanged: Vec<String>,
}

/// Re-render and overwrite agent-immutable Android scaffold files when `Android/` exists.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the app name or package cannot be
/// resolved or a file write fails.
pub fn sync_android_scaffold_files(
    project_root: &Path,
) -> Result<AndroidScaffoldSyncReport, VectisError> {
    let android_root = project_root.join("Android");
    if !android_root.is_dir() {
        return Ok(AndroidScaffoldSyncReport {
            synced: Vec::new(),
            unchanged: Vec::new(),
        });
    }

    let app_name = resolve_android_app_name(project_root)?;
    let android_package = resolve_android_package(project_root, &app_name)?;
    let expected = expected_immutable_files(&app_name, &android_package)?;
    let mut synced = Vec::new();
    let mut unchanged = Vec::new();

    for file in expected {
        let target = project_root.join(&file.relative_path);
        let expected_bytes = expected_file_bytes(&target, &file.contents);
        let matches_template = target.is_file()
            && on_disk_bytes(&target).is_ok_and(|on_disk| on_disk == expected_bytes.as_bytes());
        if matches_template {
            unchanged.push(file.relative_path);
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| map_io(&err))?;
        }
        fs::write(&target, &expected_bytes).map_err(|err| map_io(&err))?;
        synced.push(file.relative_path);
    }

    Ok(AndroidScaffoldSyncReport { synced, unchanged })
}

/// Compare agent-immutable Android scaffold files against the embedded templates.
#[must_use]
pub fn android_scaffold_drift_findings(project_root: &Path) -> Vec<Value> {
    let android_root = project_root.join("Android");
    if !android_root.is_dir() {
        return Vec::new();
    }

    let Ok(app_name) = resolve_android_app_name(project_root) else {
        return vec![drift_finding(
            "Android",
            "cannot resolve Android app name from settings.gradle.kts or Application.kt layout",
        )];
    };

    let Ok(android_package) = resolve_android_package(project_root, &app_name) else {
        return vec![drift_finding(
            "Android",
            "cannot resolve Android package from app/build.gradle.kts",
        )];
    };

    let Ok(expected) = expected_immutable_files(&app_name, &android_package) else {
        return vec![drift_finding("Android", "failed to render expected Android scaffold files")];
    };

    expected
        .into_iter()
        .filter_map(|file| {
            let target = project_root.join(&file.relative_path);
            let relative_path = file.relative_path;
            let expected_bytes = expected_file_bytes(&target, &file.contents);
            if !target.is_file() {
                return Some(drift_finding(
                    &relative_path,
                    &missing_scaffold_message(&relative_path),
                ));
            }
            match on_disk_bytes(&target) {
                Ok(on_disk) if on_disk == expected_bytes.as_bytes() => None,
                Ok(on_disk) => {
                    let on_disk_text = String::from_utf8_lossy(&on_disk);
                    Some(drift_finding(
                        &relative_path,
                        &drift_message(&relative_path, &on_disk_text),
                    ))
                }
                Err(err) => Some(drift_finding(
                    &relative_path,
                    &unreadable_scaffold_message(&relative_path, &err),
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

fn expected_immutable_files(
    app_name: &str, android_package: &str,
) -> Result<Vec<crate::scaffold::PlannedFile>, VectisError> {
    let versions = Versions::embedded()?;
    let plan = plan_android(app_name, android_package, &[], &versions)?;
    Ok(plan
        .files
        .into_iter()
        .filter(|file| IMMUTABLE_RELATIVE_PATHS.contains(&file.relative_path.as_str()))
        .collect())
}

fn expected_file_bytes(target: &Path, template_contents: &str) -> String {
    if target.file_name().is_some_and(|name| name == "build.gradle.kts")
        && target
            .parent()
            .is_some_and(|parent| parent.file_name().is_some_and(|name| name == "shared"))
        && let Some(ndk) = read_substituted_ndk_version(target)
    {
        return template_contents.replace("__ANDROID_NDK_VERSION__", &ndk);
    }
    template_contents.to_string()
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

fn read_substituted_ndk_version(shared_build: &Path) -> Option<String> {
    let source = fs::read_to_string(shared_build).ok()?;
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("ndkVersion = ") {
            let version = rest.trim().trim_matches('"');
            if version != "__ANDROID_NDK_VERSION__" && !version.is_empty() {
                return Some(version.to_string());
            }
        }
    }
    None
}

fn drift_message(relative_path: &str, on_disk: &str) -> String {
    let mut message = format!(
        "{relative_path} diverges from the embedded Android scaffold template; agents must not edit this file — the adapter re-renders it from the embedded template during build"
    );
    if relative_path.ends_with("Makefile") && !on_disk.contains(REQUIRED_MAKEFILE_RUSTFLAGS) {
        let _ = write!(
            message,
            " (Makefile cargo invocations must prefix {REQUIRED_MAKEFILE_RUSTFLAGS})"
        );
    } else if relative_path.ends_with("build.gradle.kts") {
        if relative_path.contains("/app/") {
            if !on_disk.contains(REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS) {
                let _ = write!(
                    message,
                    " (Gradle kotlin.compilerOptions must set {REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS} on the app module)"
                );
            } else if !on_disk.contains(REQUIRED_JAVA_COMPILE_WERROR) {
                let _ = write!(
                    message,
                    " (Gradle must add JavaCompile {REQUIRED_JAVA_COMPILE_WERROR} on the app module)"
                );
            }
        } else if relative_path.contains("/shared/")
            && on_disk.contains(REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS)
        {
            let _ = write!(
                message,
                " (the shared module compiles generated UniFFI Kotlin only — it must not set {REQUIRED_GRADLE_ALL_WARNINGS_AS_ERRORS})"
            );
        }
    }
    message
}

fn missing_scaffold_message(relative_path: &str) -> String {
    format!(
        "{relative_path} is missing; CLI-owned scaffold files must be present — the adapter re-renders it from the embedded template during build"
    )
}

fn unreadable_scaffold_message(relative_path: &str, err: &std::io::Error) -> String {
    format!(
        "{relative_path} could not be read ({err}); CLI-owned scaffold files must match the embedded template — the adapter re-renders it from the embedded template during build"
    )
}

fn on_disk_bytes(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    fs::read(path)
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
