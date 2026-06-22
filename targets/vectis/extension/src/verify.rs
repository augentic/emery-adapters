//! `vectis verify` subcommand — declared-vs-present platform shell verification.
//!
//! Authority is `project.yaml.platforms` (the typed platform set, not
//! per-slice proposals). The engine inspects on-disk shell trees and
//! reports which declared platforms are present.
//!
//! Two modes:
//!
//! - **verify** (build/lint): emits `diagnostic.schema.json`-shaped
//!   findings and exits non-zero on any miss for a supported platform.
//! - **bootstrap-app-icon** (build-time): gates the launcher `app-icon`
//!   for every declared UI platform (`ios` / `android`), exiting
//!   non-zero when one is neither shell-resident (RFC-46 §6.3) nor
//!   satisfiable from `design-system/assets.yaml` (§4.1).

mod app_icon;
mod catalog;

use std::path::{Path, PathBuf};

use clap::{Args as ClapArgs, ValueEnum};
use serde_json::Value;

use crate::shell::{SUPPORTED_SHELL_PLATFORMS, shell_present};
use crate::validate::find_project_root;
use crate::{VectisError, render_json as render_value};

/// Arguments accepted by `vectis verify`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct VerifyArgs {
    /// Verification mode to run.
    #[arg(long, value_enum)]
    pub mode: VerifyMode,

    /// Project directory. Falls back to `PROJECT_DIR` env, then CWD walk-up.
    pub path: Option<PathBuf>,
}

/// Verification mode.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum VerifyMode {
    /// Build/lint-time: emit diagnostic findings, exit non-zero on miss.
    Verify,
    /// Build-time: gate the launcher `app-icon` for declared UI
    /// platforms (`ios` / `android`); RFC-46 §6.
    BootstrapAppIcon,
}

/// Per-platform status entry in the verify report.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlatformStatus {
    platform: String,
    declared: bool,
    present: bool,
}

/// Run the verify engine.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when `project.yaml` is
/// missing or unparseable, or lacks a `platforms` field.
pub fn run(args: &VerifyArgs) -> Result<Value, VectisError> {
    let project_root = resolve_project_root(args.path.as_deref())?;
    let platforms = load_platforms(&project_root)?;

    match args.mode {
        VerifyMode::Verify => {
            let statuses: Vec<PlatformStatus> =
                platforms.iter().map(|p| check_platform(p, &project_root)).collect();
            Ok(render_verify(&statuses, &project_root, &platforms))
        }
        VerifyMode::BootstrapAppIcon => Ok(render_bootstrap_app_icon(&project_root, &platforms)),
    }
}

/// Render a `(success | error)` result as pretty-printed JSON with exit code.
///
/// Both modes exit 1 when any `error`-severity finding is present (a missing
/// supported shell, or an unsatisfiable launcher `app-icon`), and 0 otherwise.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = verify_exit_code(&value);
            (render_value(&value), code)
        }
        Err(err) => {
            let exit_code = err.exit_code();
            let Value::Object(mut payload) = err.to_json() else {
                unreachable!("VectisError::to_json always returns an object")
            };
            payload.entry("exit-code".to_string()).or_insert(Value::from(exit_code));
            (render_value(&Value::Object(payload)), exit_code)
        }
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

fn resolve_project_root(path: Option<&Path>) -> Result<PathBuf, VectisError> {
    if let Some(p) = path {
        return Ok(p.to_path_buf());
    }
    if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(project_dir));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_project_root(&cwd).ok_or_else(|| VectisError::InvalidProject {
        message: "cannot locate project root (no .specify/ directory found)".into(),
    })
}

fn load_platforms(project_root: &Path) -> Result<Vec<String>, VectisError> {
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

    findings.extend(catalog::catalog_findings(project_root, platforms));

    serde_json::json!({
        "mode": "verify",
        "project-root": project_root.display().to_string(),
        "findings": findings,
    })
}
