//! `vectis android` subcommand surface.
//!
//! The vendored Gradle-wrapper installer moved to `specify-vectis-core`
//! (RFC-61 Step 5 Milestone A1); this module keeps the WASI command
//! surface — argument parsing, project-root resolution, and the JSON
//! envelope — and delegates the install to the core.

use std::path::{Path, PathBuf};

use clap::{Args as ClapArgs, Subcommand};
use serde_json::Value;
pub use specify_vectis_core::android::{run_for_shell_dir, setup_exit_code};

use crate::validate::find_project_root;
use crate::{VectisError, render_json as render_value};

/// Nested targets under `vectis android`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum AndroidCommand {
    /// Install the vendored Gradle wrapper when absent.
    Setup(AndroidSetupArgs),
}

/// Arguments for `vectis android setup`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct AndroidSetupArgs {
    /// Project directory. Falls back to `PROJECT_DIR` env, then CWD walk-up.
    pub path: Option<PathBuf>,
}

/// Dispatch a parsed [`AndroidCommand`].
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the project root or
/// `Android/` shell directory cannot be resolved.
pub fn run(command: &AndroidCommand) -> Result<Value, VectisError> {
    match command {
        AndroidCommand::Setup(args) => {
            let project_root = resolve_project_root(args.path.as_deref())?;
            specify_vectis_core::android::setup(&project_root)
        }
    }
}

/// Render an android outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = setup_exit_code(&value);
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
