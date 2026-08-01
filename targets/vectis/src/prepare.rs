//! Slice-build prepare — scope resolution and the materialize step.

mod scope;

use std::path::Path;

pub use scope::{
    EffectiveAssets, MaterializeScope, materialize_platform_csv, resolve_effective_assets,
    resolve_materialize_scope, scope_needs_materialize, validate_effective_inventory,
};
use serde_json::{Value, json};

use crate::VectisError;
use crate::materialize::{AssetsArgs, MaterializeCommand, run as run_materialize};

/// Run the prepare materialize step for one slice build.
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
