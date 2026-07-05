//! `vectis verify` subcommand surface.
//!
//! The deterministic `verify` and `bootstrap-app-icon` modes moved to
//! `specify-vectis-core` (RFC-61 Step 5 Milestone A1); this module
//! keeps the WASI command surface — argument parsing, project-root
//! resolution, and the JSON envelope — plus the advisory `host-prereq`
//! mode, which probes host environment variables and native toolchain
//! paths and therefore stays out of the wasm-clean core.

use std::path::{Path, PathBuf};

use clap::{Args as ClapArgs, ValueEnum};
use serde_json::Value;
pub use specify_vectis_core::verify::{
    FINDING_ID, load_platforms, suppression_scan_findings, verify_exit_code,
};

use crate::validate::find_project_root;
use crate::{VectisError, host_prereq, render_json as render_value};

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
    /// Prepare-time: probe host toolchain prerequisites for declared platforms.
    HostPrereq,
}

/// Run the verify engine.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when `project.yaml` is
/// missing or unparseable, or lacks a `platforms` field.
pub fn run(args: &VerifyArgs) -> Result<Value, VectisError> {
    let project_root = resolve_project_root(args.path.as_deref())?;

    match args.mode {
        VerifyMode::Verify => specify_vectis_core::verify::run(
            specify_vectis_core::verify::VerifyMode::Verify,
            &project_root,
        ),
        VerifyMode::BootstrapAppIcon => specify_vectis_core::verify::run(
            specify_vectis_core::verify::VerifyMode::BootstrapAppIcon,
            &project_root,
        ),
        VerifyMode::HostPrereq => {
            let platforms = load_platforms(&project_root)?;
            Ok(render_host_prereq(&project_root, &platforms))
        }
    }
}

/// Render a `(success | error)` result as pretty-printed JSON with exit code.
///
/// Every mode exits 1 when any `error`-severity finding is present, and
/// 0 otherwise.
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

fn render_host_prereq(project_root: &Path, platforms: &[String]) -> Value {
    let findings = host_prereq::host_prereq_findings(platforms);
    serde_json::json!({
        "mode": "host-prereq",
        "project-root": project_root.display().to_string(),
        "platforms": platforms,
        "findings": findings,
    })
}
