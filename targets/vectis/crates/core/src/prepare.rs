//! Slice-build prepare — RFC §2.1 scope resolution and the conditional
//! materialize step.
//!
//! Absorbed from the legacy extension's `prepare build` subcommand
//! (RFC-61 Step 3). This module owns the deterministic half the guest
//! runs as its build prelude (replacing the `adapter.yaml`
//! `prepare.argv` hook): resolve the effective `assets.yaml`
//! (slice-local → project cascade), derive the in-scope asset ids from
//! the slice's `composition.yaml` or artifact prose, and run
//! [`crate::materialize`] over exactly that scope when a declared shell
//! platform lacks on-disk exports. The extension's `prepare build` CLI
//! keeps its host-bootstrap legs (app-icon verify gate, Android Gradle
//! setup, iOS scaffold sync) around a call into [`materialize_step`];
//! those legs depend on the scaffold / verify machinery that stays
//! extension-side until Step 5.

mod scope;

use std::path::Path;

pub use scope::{
    EffectiveAssets, MaterializeScope, materialize_platform_csv, resolve_effective_assets,
    resolve_materialize_scope, scope_needs_materialize, validate_effective_inventory,
};
use serde_json::{Value, json};

use crate::VectisError;
use crate::materialize::{AssetsArgs, MaterializeCommand, run as run_materialize};

/// Run the deterministic prepare materialize step for one slice build.
///
/// Scope resolution over the effective `assets.yaml`, then a scoped
/// `materialize assets` run when any in-scope asset lacks exports for a
/// declared shell platform, or a `skipped: true` summary otherwise.
///
/// The returned value is the same `materialize assets` summary envelope
/// the extension's `prepare build` embeds under its `materialized` key.
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
