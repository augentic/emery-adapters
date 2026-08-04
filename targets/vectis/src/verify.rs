//! Declared-vs-present platform shell verification.

mod android_toolchain;
mod app_icon;
mod catalog;
mod compile_stamp;
mod core_stamp;
mod suppression_scan;

use std::path::Path;

pub use catalog::catalog_findings;
pub use core_stamp::{CORE_VERIFY_STAMP, core_src_digest};
use serde_json::Value;
pub use suppression_scan::{FINDING_ID, suppression_scan_findings};

use crate::VectisError;
use crate::android_scaffold::android_scaffold_drift_findings;
use crate::ios_scaffold::ios_scaffold_drift_findings;
use crate::shell::{SUPPORTED_SHELL_PLATFORMS, shell_present};

/// Verification mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VerifyMode {
    /// Build/lint-time: emit diagnostic findings for declared platforms.
    Verify,
    /// Build-time: gate the launcher `app-icon` for declared UI
    /// platforms (`ios` / `android`).
    BootstrapAppIcon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformStatus {
    platform: String,
    declared: bool,
    present: bool,
}

/// Run one verification mode against explicit roots.
///
/// RFC-87 split: `change_root` carries the Emery change tree
/// (`.emery/*` reads) and `code_root` carries the product code (shell
/// trees, design-system, stamps). A single-checkout caller passes the
/// same path twice.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when `project.yaml` is
/// missing or unparseable, or lacks a `platforms` field.
pub fn run(mode: VerifyMode, change_root: &Path, code_root: &Path) -> Result<Value, VectisError> {
    let platforms = load_platforms(change_root)?;

    match mode {
        VerifyMode::Verify => {
            let statuses: Vec<PlatformStatus> =
                platforms.iter().map(|p| check_platform(p, code_root)).collect();
            Ok(render_verify(&statuses, change_root, code_root, &platforms))
        }
        VerifyMode::BootstrapAppIcon => Ok(render_bootstrap_app_icon(code_root, &platforms)),
    }
}

/// Compute the exit code for a verify payload.
#[must_use]
pub fn verify_exit_code(value: &Value) -> u8 {
    let has_findings = value.get("findings").and_then(Value::as_array).is_some_and(|arr| {
        arr.iter().any(|f| f.get("severity").and_then(Value::as_str) == Some("error"))
    });
    u8::from(has_findings)
}

/// Load the declared `platforms:` list from `.emery/project.yaml`.
///
/// # Errors
/// Returns [`VectisError::InvalidProject`] when the file is missing,
pub fn load_platforms(project_root: &Path) -> Result<Vec<String>, VectisError> {
    // The host CLI owns the project config at `.emery/project.yaml`;
    // there is no root-level `project.yaml` in a Emery project.
    let config_path = project_root.join(".emery").join("project.yaml");
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

fn check_platform(platform: &str, project_root: &Path) -> PlatformStatus {
    PlatformStatus {
        platform: platform.to_string(),
        declared: true,
        present: shell_present(project_root, platform),
    }
}

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

fn render_verify(
    statuses: &[PlatformStatus], change_root: &Path, code_root: &Path, platforms: &[String],
) -> Value {
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
                    code_root.display(),
                ),
            }));
        }
    }

    findings.extend(catalog_findings(change_root, code_root, platforms));

    let android_declared = platforms.iter().any(|p| p == "android");
    let android_present =
        statuses.iter().find(|s| s.platform == "android").is_some_and(|s| s.present);
    findings.extend(android_toolchain::android_toolchain_findings(
        code_root,
        android_declared,
        android_present,
    ));

    if platforms.iter().any(|p| p == "ios") && shell_present(code_root, "ios") {
        findings.extend(ios_scaffold_drift_findings(code_root));
    }

    if platforms.iter().any(|p| p == "android") && shell_present(code_root, "android") {
        findings.extend(android_scaffold_drift_findings(code_root));
    }

    let ios_present = statuses.iter().find(|s| s.platform == "ios").is_some_and(|s| s.present);
    findings.extend(compile_stamp::compile_stamp_findings(
        code_root,
        platforms,
        ios_present,
        android_present,
    ));
    findings.extend(core_stamp::core_stamp_findings(code_root));

    findings.extend(suppression_scan_findings(code_root, platforms));

    serde_json::json!({
        "mode": "verify",
        "project-root": code_root.display().to_string(),
        "findings": findings,
    })
}
