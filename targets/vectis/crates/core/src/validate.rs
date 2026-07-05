//! Deterministic validation library for Vectis UI artifacts — schema +
//! cross-artifact validation for tokens, assets, layout, and
//! composition, plus an `all` fan-out.
//!
//! Absorbed from the legacy extension's `validate` subcommand (RFC-61
//! Step 3): the engine and its rule set are unchanged; only the
//! CLI argument surface stayed behind in `specify-vectis-extension`,
//! which converts its clap types onto [`ValidateMode`] and calls
//! [`run`]. The guest's `build` / `merge` operations call [`run`]
//! directly as their deterministic postlude gate. Provenance for every
//! rule lives in the extension's sidecar `DECISIONS.md` until Step 5
//! retires it.

use std::path::Path;

use serde_json::Value;

pub mod engine;

pub use engine::find_project_root;

/// Re-export the crate-wide error type at the path the engine modules
/// historically import (`crate::validate::error::VectisError`).
pub mod error {
    pub use crate::VectisError;
}

/// Vectis validation modes, mirroring the extension's CLI spelling.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ValidateMode {
    /// Validate a `tokens.yaml` file.
    Tokens,
    /// Validate an `assets.yaml` file.
    Assets,
    /// Validate a `layout.yaml` file.
    Layout,
    /// Validate a `composition.yaml` file.
    Composition,
    /// Validate all Vectis UI artifacts reachable from the given root.
    All,
}

impl ValidateMode {
    /// Return the stable mode spelling used in report envelopes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::Assets => "assets",
            Self::Layout => "layout",
            Self::Composition => "composition",
            Self::All => "all",
        }
    }
}

/// Run one validation mode against an explicit artifact path (or the
/// project cascade when `path` is `None`).
///
/// # Errors
///
/// Returns [`crate::VectisError::InvalidProject`] when the resolved
/// artifact is unreadable in single-mode runs and
/// [`crate::VectisError::Internal`] if an embedded schema fails to
/// compile; YAML parse and schema violations fold into the envelope's
/// `errors` array instead. See [`engine::run`].
pub fn run(mode: ValidateMode, path: Option<&Path>) -> Result<Value, crate::VectisError> {
    engine::run(mode, path)
}

/// Compute the recursive validation exit code for a success payload:
/// `1` when any real sub-report carries errors, `0` otherwise.
#[must_use]
pub fn validate_exit_code(value: &Value) -> u8 {
    u8::from(envelope_has_errors(value))
}

/// Whether a validation envelope (or any folded sub-report) carries
/// errors — the recursion `validate_exit_code` and the guest's
/// deterministic postlude share.
#[must_use]
pub fn envelope_has_errors(node: &Value) -> bool {
    if node.get("errors").and_then(Value::as_array).is_some_and(|arr| !arr.is_empty()) {
        return true;
    }
    if let Some(results) = node.get("results").and_then(Value::as_array) {
        return results.iter().any(|entry| {
            entry.get("report").is_some_and(envelope_has_errors) || envelope_has_errors(entry)
        });
    }
    false
}
