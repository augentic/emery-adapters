//! Deterministic validation engine behind [`crate::validate::run`].
//!
//! Public surface: [`run`] dispatches a [`ValidateMode`] plus optional
//! artifact path to the per-mode handler. Each per-mode envelope
//! carries a uniform shape:
//!
//! ```json
//! {
//!   "mode": "assets",
//!   "path": "design-system/assets.yaml",
//!   "errors":   [{ "path": "/assets/foo/sources/ios/1x", "message": "..." }],
//!   "warnings": [{ "path": "/assets/foo/sources/android", "message": "..." }]
//! }
//! ```
//!
//! Errors / warnings entries carry a JSON Pointer-shaped `path` so the
//! operator can locate the offending sub-document. The dispatcher
//! exits non-zero only when a real sub-report carries errors. Provenance
//! and the rationale behind every rule live in the repository-root
//! `DECISIONS.md` (§"Vectis validation and materialization").

mod all;
mod assets;
pub mod composition;
mod layout;
mod paths;
mod shared;
mod tokens;

use std::path::Path;

pub use assets::exports::{
    app_icon_export_exists, conventional_export_exists, imageset_has_materialized_content,
    platform_pin_active,
};
pub use assets::{collect_asset_references, load_shell_platforms};
pub use paths::{discover_artifact, find_project_root, resolve_default_path_with_root};
use serde_json::Value;
pub use shared::parse_yaml_file;

use crate::validate::ValidateMode;
use crate::validate::error::VectisError;

/// Dispatch one validation mode to the per-mode handler.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the resolved
/// `tokens.yaml` / `assets.yaml` / `layout.yaml` / `composition.yaml`
/// is unreadable in single-mode runs (missing file, permission
/// denied; `validate all` instead surfaces the missing input as a
/// synthetic `skipped: true` sub-report) and [`VectisError::Internal`]
/// if an embedded schema fails to compile. YAML parse failures and
/// schema validation failures are *not* errors at this layer; they are
/// folded into the `errors` array of the per-mode envelope so the
/// operator sees the full report alongside any other findings.
pub fn run(mode: ValidateMode, path: Option<&Path>) -> Result<Value, VectisError> {
    match mode {
        ValidateMode::Tokens => tokens::validate(path),
        ValidateMode::Assets => assets::validate(path),
        ValidateMode::Layout => layout::validate(path),
        ValidateMode::Composition => composition::validate(path),
        ValidateMode::All => all::validate(path),
    }
}

/// Re-enter [`run`] for the auto-invoke path. Runs the named sub-mode
/// against the supplied path and returns its envelope. Used by
/// composition mode to fold sibling tokens / assets envelopes, and by
/// `validate all` to dispatch each sub-mode in turn.
pub(super) fn run_inner(mode: ValidateMode, path: &Path) -> Result<Value, VectisError> {
    run(mode, Some(path))
}
