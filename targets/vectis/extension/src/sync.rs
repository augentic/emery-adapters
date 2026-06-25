//! `vectis sync` subcommand — lightweight scaffold repair without prepare side effects.

use std::path::{Path, PathBuf};

use clap::{Args as ClapArgs, Subcommand};
use serde_json::{Value, json};

use crate::ios_scaffold::{scaffold_sync_ios_json, sync_ios_scaffold_files};
use crate::validate::find_project_root;
use crate::{VectisError, render_json as render_value};

/// Nested targets under `vectis sync`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum SyncCommand {
    /// Re-render agent-immutable `iOS/Makefile` and `iOS/project.yml` from templates.
    IosScaffold(IosScaffoldArgs),
}

/// Arguments for `vectis sync ios-scaffold`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct IosScaffoldArgs {
    /// Project directory. Falls back to `PROJECT_DIR` env, then CWD walk-up.
    pub path: Option<PathBuf>,
}

/// Dispatch a parsed [`SyncCommand`].
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the project root cannot be
/// resolved or scaffold sync fails.
pub fn run(command: &SyncCommand) -> Result<Value, VectisError> {
    match command {
        SyncCommand::IosScaffold(args) => run_ios_scaffold(args),
    }
}

/// Render a sync outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => (render_value(&value), 0),
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

fn run_ios_scaffold(args: &IosScaffoldArgs) -> Result<Value, VectisError> {
    let project_root = resolve_project_root(args.path.as_deref())?;
    let report = sync_ios_scaffold_files(&project_root)?;
    Ok(json!({
        "command": "sync ios-scaffold",
        "project-root": project_root.display().to_string(),
        "scaffold_sync": scaffold_sync_ios_json(&report),
    }))
}

fn resolve_project_root(path: Option<&Path>) -> Result<PathBuf, VectisError> {
    if let Some(p) = path {
        return Ok(p.to_path_buf());
    }
    if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(project_dir));
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_project_root(&cwd).ok_or_else(|| VectisError::InvalidProject {
        message: "cannot locate project root (no .specify/ directory found)".into(),
    })
}
