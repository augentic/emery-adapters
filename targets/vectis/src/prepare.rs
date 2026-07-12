//! Slice-build prepare — scope resolution, materialize step, and prepare orchestration.

mod scope;

use std::path::{Path, PathBuf};

pub use scope::{
    EffectiveAssets, MaterializeScope, materialize_platform_csv, resolve_effective_assets,
    resolve_materialize_scope, scope_needs_materialize, validate_effective_inventory,
};
use serde_json::{Value, json};

use crate::materialize::{
    AssetsArgs, MaterializeCommand, materialize_exit_code, run as run_materialize,
};
use crate::validate::engine::load_shell_platforms;
use crate::verify::{VerifyMode, run as run_verify, verify_exit_code};
use crate::{VectisError, android, ios_scaffold};

/// Run the prepare materialize step for one slice build.
///
/// Scope resolution over the effective `assets.yaml`, then a scoped
/// `materialize assets` run when any in-scope asset lacks exports for a
/// declared shell platform, or a `skipped: true` summary otherwise.
/// Returns the summary envelope `prepare build` embeds under
/// `materialized`.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the effective inventory
/// exists but is unreadable or lacks a parseable `assets:` map, and
/// propagates [`crate::materialize::run`] failures.
pub fn materialize_step(
    slice_dir: &Path, project_root: &Path, shell_platforms: &[String],
) -> Result<Value, VectisError> {
    let Some(effective) = resolve_effective_assets(slice_dir, project_root) else {
        return Ok(skipped_materialize_summary(
            &project_root.join("design-system/assets.yaml"),
            shell_platforms,
        ));
    };
    validate_effective_inventory(&effective)?;
    let scope = resolve_materialize_scope(slice_dir, project_root, shell_platforms, &effective);
    if scope_needs_materialize(&scope, &effective, shell_platforms) {
        let only: Vec<String> = scope.asset_ids.into_iter().collect();
        run_materialize(&MaterializeCommand::Assets(AssetsArgs {
            path: Some(effective.path),
            platform: Some(shell_platforms.to_vec()),
            dry_run: false,
            only: Some(only),
        }))
    } else {
        Ok(skipped_materialize_summary(&effective.path, shell_platforms))
    }
}

/// The `materialize assets` summary emitted when nothing is in scope —
/// shape-identical to a real run's envelope plus `skipped: true`.
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

/// Run the full slice-build prepare for one slice.
///
/// The scoped materialize step, the app-icon bootstrap gate, the
/// Android Gradle-wrapper setup, and the iOS scaffold sync.
/// `slice_dir` may be absolute or relative to `project_root`.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the slice directory
/// cannot be resolved, and propagates materialize / verify / sync
/// failures.
pub fn run_build(project_root: &Path, slice_dir: &Path) -> Result<Value, VectisError> {
    let slice_dir = resolve_slice_dir(project_root, slice_dir)?;
    let platforms = load_shell_platforms(project_root);

    let materialized = materialize_step(&slice_dir, project_root, &platforms)?;

    let bootstrap = run_verify(VerifyMode::BootstrapAppIcon, project_root)?;

    let android_setup = (platforms.iter().any(|p| p == "android")
        && project_root.join("Android").is_dir())
    .then(|| android::run_for_shell_dir(&project_root.join("Android")));

    let scaffold_sync = (platforms.iter().any(|p| p == "ios") && project_root.join("iOS").is_dir())
        .then(|| ios_scaffold::sync_ios_scaffold_files(project_root))
        .transpose()?
        .map(|report| ios_scaffold::scaffold_sync_ios_json(&report));

    Ok(json!({
        "command": "prepare build",
        "slice_dir": slice_dir.strip_prefix(project_root)
            .map_or_else(|_| slice_dir.to_string_lossy().into_owned(), |p| p.to_string_lossy().into_owned()),
        "platforms": platforms,
        "materialized": materialized,
        "bootstrap_app_icon": bootstrap,
        "android_setup": android_setup,
        "scaffold_sync": scaffold_sync,
    }))
}

/// Compute the exit code for a [`run_build`] payload: `1` when the
/// materialize step, the bootstrap gate, or the Android setup surfaced
/// errors, `0` otherwise.
#[must_use]
pub fn exit_code(value: &Value) -> u8 {
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
        && android::setup_exit_code(setup) != 0
    {
        return 1;
    }
    0
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
