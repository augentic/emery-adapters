//! `vectis prepare` subcommand surface.
//!
//! The full prepare orchestration — scope resolution, conditional
//! materialize, the app-icon bootstrap gate, the Android Gradle setup,
//! and the iOS scaffold sync — moved to `specify-vectis-core` (RFC-61
//! Steps 3 and 5); this module keeps the WASI command surface —
//! argument parsing, project-root resolution, and the JSON envelope —
//! and delegates the run to the core.

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};
use serde_json::Value;
pub use specify_vectis_core::prepare::{
    EffectiveAssets, MaterializeScope, materialize_platform_csv, resolve_effective_assets,
    resolve_materialize_scope, scope_needs_materialize, validate_effective_inventory,
};

use crate::validate::find_project_root;
use crate::{VectisError, render_json as render_value};

/// Nested targets under `vectis prepare`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum PrepareCommand {
    /// Run slice-build prepare: scope resolution, conditional materialize, bootstrap gate.
    Build(BuildArgs),
}

/// Arguments for `vectis prepare build`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct BuildArgs {
    /// Slice directory relative to `$PROJECT_DIR` (preferred) or absolute.
    pub slice_dir: PathBuf,
}

/// Dispatch a parsed [`PrepareCommand`] through the core.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the project root or slice
/// directory cannot be resolved.
pub fn run(command: &PrepareCommand) -> Result<Value, VectisError> {
    match command {
        PrepareCommand::Build(args) => {
            let project_root = resolve_project_root()?;
            specify_vectis_core::prepare::run_build(&project_root, &args.slice_dir)
        }
    }
}

/// Render a prepare outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = specify_vectis_core::prepare::exit_code(&value);
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

fn resolve_project_root() -> Result<PathBuf, VectisError> {
    if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(project_dir));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_project_root(&cwd).ok_or_else(|| VectisError::InvalidProject {
        message: "cannot locate project root (no .specify/ directory found)".into(),
    })
}
