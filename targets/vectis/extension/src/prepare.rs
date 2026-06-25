//! `vectis prepare` subcommand — slice-build prepare orchestration (RFC §2.1 + §6).

mod scope;

use std::path::{Path, PathBuf};

use clap::{Args as ClapArgs, Subcommand};
pub use scope::{
    EffectiveAssets, MaterializeScope, materialize_platform_csv, resolve_effective_assets,
    resolve_materialize_scope, scope_needs_materialize, validate_effective_inventory,
};
use serde_json::{Value, json};

use crate::android::{run_for_shell_dir, setup_exit_code};
use crate::materialize::{
    AssetsArgs, MaterializeCommand, materialize_exit_code, run as run_materialize,
};
use crate::validate::engine::load_shell_platforms;
use crate::validate::find_project_root;
use crate::verify::{VerifyArgs, VerifyMode, run as run_verify, verify_exit_code};
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

/// Dispatch a parsed [`PrepareCommand`].
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the project root or slice
/// directory cannot be resolved.
pub fn run(command: &PrepareCommand) -> Result<Value, VectisError> {
    match command {
        PrepareCommand::Build(args) => run_build(args),
    }
}

/// Render a prepare outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = prepare_exit_code(&value);
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

fn prepare_exit_code(value: &Value) -> u8 {
    if let Some(materialized) = value.get("materialized")
        && materialize_exit_code(materialized) != 0
    {
        return 1;
    }
    if let Some(bootstrap) = value.get("bootstrap_app_icon")
        && verify_exit_code(bootstrap) != 0
    {
        return 1;
    }
    if let Some(setup) = value.get("android_setup")
        && setup_exit_code(setup) != 0
    {
        return 1;
    }
    0
}

fn run_build(args: &BuildArgs) -> Result<Value, VectisError> {
    let project_root = resolve_project_root()?;
    let slice_dir = resolve_slice_dir(&project_root, &args.slice_dir)?;
    let platforms = load_shell_platforms(&project_root);

    let materialized = if let Some(effective) = resolve_effective_assets(&slice_dir, &project_root)
    {
        validate_effective_inventory(&effective)?;
        let scope = resolve_materialize_scope(&slice_dir, &project_root, &platforms, &effective);
        if scope_needs_materialize(&scope, &effective, &platforms) {
            let only: Vec<String> = scope.asset_ids.into_iter().collect();
            let mat_args = AssetsArgs {
                path: Some(effective.path),
                platform: Some(platforms.clone()),
                dry_run: false,
                only: Some(only),
            };
            run_materialize(&MaterializeCommand::Assets(mat_args))?
        } else {
            skipped_materialize_summary(&effective.path, &platforms)
        }
    } else {
        skipped_materialize_summary(&project_root.join("design-system/assets.yaml"), &platforms)
    };

    let bootstrap = run_verify(&VerifyArgs {
        mode: VerifyMode::BootstrapAppIcon,
        path: Some(project_root.clone()),
    })?;

    let android_setup = (platforms.iter().any(|p| p == "android")
        && project_root.join("Android").is_dir())
    .then(|| run_for_shell_dir(&project_root.join("Android")));

    Ok(json!({
        "command": "prepare build",
        "slice_dir": slice_dir.strip_prefix(&project_root)
            .map_or_else(|_| slice_dir.to_string_lossy().into_owned(), |p| p.to_string_lossy().into_owned()),
        "platforms": platforms,
        "materialized": materialized,
        "bootstrap_app_icon": bootstrap,
        "android_setup": android_setup,
    }))
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

fn resolve_slice_dir(project_root: &Path, slice_dir: &Path) -> Result<PathBuf, VectisError> {
    let resolved = if slice_dir.is_absolute() {
        slice_dir.to_path_buf()
    } else {
        project_root.join(slice_dir)
    };
    if !resolved.is_dir() {
        return Err(VectisError::InvalidProject {
            message: format!("slice directory not found at {}", resolved.display()),
        });
    }
    Ok(resolved)
}

fn skipped_materialize_summary(path: &Path, platforms: &[String]) -> Value {
    json!({
        "command": "materialize assets",
        "path": path.display().to_string(),
        "dry_run": false,
        "platforms": platforms,
        "materialized": [],
        "skipped_pins": [],
        "errors": [],
        "skipped": true,
    })
}
