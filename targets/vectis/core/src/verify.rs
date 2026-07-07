//! Declared-vs-present platform shell verification.

mod android_toolchain;
mod app_icon;
mod catalog;
mod compile_stamp;
mod suppression_scan;

use std::path::Path;

pub use catalog::catalog_findings;
use serde_json::Value;
pub use suppression_scan::{FINDING_ID, suppression_scan_findings};

use crate::VectisError;
use crate::android_scaffold::android_scaffold_drift_findings;
use crate::ios_scaffold::ios_scaffold_drift_findings;
use crate::shell::{SUPPORTED_SHELL_PLATFORMS, shell_present};

/// Deterministic verification mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VerifyMode {
    /// Build/lint-time: emit diagnostic findings for declared platforms.
    Verify,
    /// Build-time: gate the launcher `app-icon` for declared UI
    /// platforms (`ios` / `android`).
    BootstrapAppIcon,
}

/// Per-platform status entry in the verify report.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformStatus {
    platform: String,
    declared: bool,
    present: bool,
}

/// Run one deterministic verification mode against an explicit project root.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when `project.yaml` is
/// missing or unparseable, or lacks a `platforms` field.
pub fn run(mode: VerifyMode, project_root: &Path) -> Result<Value, VectisError> {
    let platforms = load_platforms(project_root)?;

    match mode {
        VerifyMode::Verify => {
            let statuses: Vec<PlatformStatus> =
                platforms.iter().map(|p| check_platform(p, project_root)).collect();
            Ok(render_verify(&statuses, project_root, &platforms))
        }
        VerifyMode::BootstrapAppIcon => Ok(render_bootstrap_app_icon(project_root, &platforms)),
    }
}

/// Compute the exit code for a verify payload.
///
/// Returns 1 when any `error`-severity finding is present, 0 otherwise.
/// Both `verify` and `bootstrap-app-icon` modes carry their result in
/// the same `findings` array.
#[must_use]
pub fn verify_exit_code(value: &Value) -> u8 {
    let has_findings = value.get("findings").and_then(Value::as_array).is_some_and(|arr| {
        arr.iter().any(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
    });
    u8::from(has_findings)
}

// ── project.yaml loading ───────────────────────────────────────────

/// Load the declared `platforms:` list from `.specify/project.yaml`.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the file is missing,
/// unparseable, or does not declare a string `platforms` array.
pub fn load_platforms(project_root: &Path) -> Result<Vec<String>, VectisError> {
    // The host CLI owns the project config at `.specify/project.yaml`;
    // there is no root-level `project.yaml` in a Specify project.
    let config_path = project_root.join(".specify").join("project.yaml");
    let source =
        std::fs::read_to_string(&config_path).map_err(|err| VectisError::InvalidProject {
            message: format!("project.yaml not readable at {}: {err}", config_path.display()),
        })?;
    let doc: Value =
        serde_saphyr::from_str(&source).map_err(|err| VectisError::InvalidProject {
            message: format!("project.yaml is not valid YAML: {err}"),
        })?;
    let platforms = doc.get("platforms").and_then(Value::as_array).ok_or_else(|| {
        VectisError::InvalidProject {
            message: "project.yaml does not declare a `platforms` array".into(),
        }
    })?;
    platforms
        .iter()
        .map(|v| {
            v.as_str().map(String::from).ok_or_else(|| VectisError::InvalidProject {
                message: "project.yaml `platforms` array contains a non-string entry".into(),
            })
        })
        .collect()
}

// ── per-platform shell detection ───────────────────────────────────

fn check_platform(platform: &str, project_root: &Path) -> PlatformStatus {
    PlatformStatus {
        platform: platform.to_string(),
        declared: true,
        present: shell_present(project_root, platform),
    }
}

// ── output rendering ───────────────────────────────────────────────

fn is_supported(platform: &str) -> bool {
    SUPPORTED_SHELL_PLATFORMS.contains(&platform)
}

fn render_bootstrap_app_icon(project_root: &Path, platforms: &[String]) -> Value {
    let findings = app_icon::bootstrap_app_icon_findings(project_root, platforms);
    serde_json::json!({
        "mode": "bootstrap-app-icon",
        "project-root": project_root.display().to_string(),
        "findings": findings,
    })
}

fn render_verify(statuses: &[PlatformStatus], project_root: &Path, platforms: &[String]) -> Value {
    let mut findings: Vec<Value> = Vec::new();

    for status in statuses {
        if !is_supported(&status.platform) {
            findings.push(serde_json::json!({
                "id": "platform-not-yet-supported",
                "severity": "info",
                "source": "deterministic",
                "message": format!(
                    "platform `{}` is accepted but has no on-disk interpretation yet",
                    status.platform,
                ),
            }));
            continue;
        }
        if !status.present {
            findings.push(serde_json::json!({
                "id": "platform-shell-missing",
                "severity": "error",
                "source": "deterministic",
                "message": format!(
                    "declared platform `{}` has no shell tree under `{}`",
                    status.platform,
                    project_root.display(),
                ),
            }));
        }
    }

    findings.extend(catalog_findings(project_root, platforms));

    let android_declared = platforms.iter().any(|p| p == "android");
    let android_present =
        statuses.iter().find(|s| s.platform == "android").is_some_and(|s| s.present);
    findings.extend(android_toolchain::android_toolchain_findings(
        project_root,
        android_declared,
        android_present,
    ));

    if platforms.iter().any(|p| p == "ios") && shell_present(project_root, "ios") {
        findings.extend(ios_scaffold_drift_findings(project_root));
    }

    if platforms.iter().any(|p| p == "android") && shell_present(project_root, "android") {
        findings.extend(android_scaffold_drift_findings(project_root));
    }

    let ios_present = statuses.iter().find(|s| s.platform == "ios").is_some_and(|s| s.present);
    findings.extend(compile_stamp::compile_stamp_findings(
        project_root,
        platforms,
        ios_present,
        android_present,
    ));

    findings.extend(suppression_scan_findings(project_root, platforms));

    serde_json::json!({
        "mode": "verify",
        "project-root": project_root.display().to_string(),
        "findings": findings,
    })
}
