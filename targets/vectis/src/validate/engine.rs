//! Validation engine behind [`crate::validate::run`]. [`run`] dispatches a [`ValidateMode`] plus optional artifact path to the per-mode handler. Every per-mode envelope shares one shape: ```json { "mode": "assets", "path": "design-system/assets.yaml", "errors":   [{ "path": "/assets/foo/sources/ios/1x", "message": "..." }], "warnings": [{ "path": "/assets/foo/sources/android", "message": "..." }] } ``` Error / warning entries carry a JSON Pointer-shaped `path` locating the offending sub-document.

mod all;
mod assets;
pub(crate) mod composition;
mod layout;
mod paths;
mod shared;
mod tokens;

use std::path::Path;

pub(crate) use assets::exports::{
    app_icon_export_exists, conventional_export_exists, imageset_has_materialized_content,
    platform_pin_active,
};
pub use assets::{collect_asset_references, load_shell_platforms};
pub use paths::{
    discover_artifact, find_project_root, resolve_default_path_with_root,
    resolve_default_path_with_roots,
};
use serde_json::Value;
pub use shared::parse_yaml_file;

use crate::validate::ValidateMode;
use crate::validate::error::VectisError;

/// Dispatch one validation mode to the per-mode handler.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the resolved artifact
/// is unreadable in single-mode runs (`validate all` instead surfaces
/// a missing input as a `skipped: true` sub-report) and
/// [`VectisError::Internal`] if an embedded schema fails to compile.
/// YAML parse and schema violations are folded into the envelope's
/// `errors` array instead of erroring at this layer.
pub fn run(mode: ValidateMode, path: Option<&Path>) -> Result<Value, VectisError> {
    match mode {
        ValidateMode::Tokens => tokens::validate(path),
        ValidateMode::Assets => assets::validate(path),
        ValidateMode::Layout => layout::validate(path),
        ValidateMode::Composition => composition::validate(path),
        ValidateMode::All => all::validate(path),
    }
}

/// Re-enter [`run`] with an explicit path — used by composition
/// mode's sibling auto-invoke and by `validate all`.
pub(super) fn run_inner(mode: ValidateMode, path: &Path) -> Result<Value, VectisError> {
    run(mode, Some(path))
}
