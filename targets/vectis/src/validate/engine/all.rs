//! `validate all` — fan out across every per-mode validator and fold
//! the per-mode envelopes into a combined `{ "mode": "all", "results":
//! [...] }` shape.

use std::path::Path;

use serde_json::{Value, json};

use super::paths::{default_project_root, resolve_default_path_with_root};
use super::run_inner;
use crate::validate::ValidateMode;
use crate::validate::error::VectisError;

/// Run every per-mode validator against the supplied project root (or
/// CWD) and fold the envelopes into one combined envelope.
///
/// Sub-mode order `layout`, `composition`, `tokens`, `assets` matches
/// the "structural input → wired composition → cross-artifact
/// references" pipeline. A missing default-resolved input becomes a
/// synthetic `skipped: true` sub-report so the combined run continues;
/// only a real sub-report with errors flips the exit code.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when a sub-mode's present
/// input fails to read, and [`VectisError::Internal`] if an embedded
/// schema fails to compile.
pub(super) fn validate(path: Option<&Path>) -> Result<Value, VectisError> {
    let project_root = path.map_or_else(default_project_root, Path::to_path_buf);

    let mut results: Vec<Value> = Vec::new();
    for mode in [
        ValidateMode::Layout,
        ValidateMode::Composition,
        ValidateMode::Tokens,
        ValidateMode::Assets,
    ] {
        let target = resolve_default_path_with_root(mode, &project_root);
        let report = if target.is_file() {
            run_inner(mode, &target)?
        } else {
            json!({
                "mode": mode.as_str(),
                "path": target.display().to_string(),
                "errors": Vec::<Value>::new(),
                "warnings": Vec::<Value>::new(),
                "skipped": true,
                "message": format!(
                    "no input found at {}; default-resolved via the artifacts: block (or its embedded fallback)",
                    target.display(),
                ),
            })
        };
        results.push(json!({
            "mode": mode.as_str(),
            "report": report,
        }));
    }

    Ok(json!({
        "mode": ValidateMode::All.as_str(),
        "path": project_root.display().to_string(),
        "results": results,
    }))
}
