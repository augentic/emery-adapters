//! `vectis validate` subcommand surface.
//!
//! The deterministic validation engine and embedded schemas moved to
//! `specify-vectis-core` (RFC-61 Step 3); this module keeps the WASI
//! command surface — argument parsing and the JSON envelope — and
//! delegates every check to the core so there is a single source of
//! truth. Provenance for every rule lives in the repository-root
//! `DECISIONS.md` (§"Vectis validation and materialization").

use std::path::PathBuf;

use clap::Args as ClapArgs;
use serde_json::Value;

use crate::render_json as render_value;

/// Arguments accepted by `vectis validate`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct ValidateArgs {
    /// Validation mode to run.
    #[arg(value_parser = parse_mode)]
    pub mode: ValidateMode,

    /// Artifact path for single-artifact modes, or project root for `all`.
    pub path: Option<PathBuf>,
}

/// Parse a CLI mode token onto the core's [`ValidateMode`].
fn parse_mode(token: &str) -> Result<ValidateMode, String> {
    match token {
        "tokens" => Ok(ValidateMode::Tokens),
        "assets" => Ok(ValidateMode::Assets),
        "layout" => Ok(ValidateMode::Layout),
        "composition" => Ok(ValidateMode::Composition),
        "all" => Ok(ValidateMode::All),
        other => Err(format!(
            "unknown validate mode {other:?} (expected tokens, assets, layout, composition, or all)"
        )),
    }
}

/// Re-export the crate-wide error type at its historical path.
///
/// External tests and the extension modules import
/// `specify_vectis::validate::error::VectisError`; the type itself
/// now lives in `specify-vectis-core`.
pub mod error {
    pub use crate::VectisError;
}

pub use specify_vectis_core::validate::{ValidateMode, find_project_root, validate_exit_code};

pub use crate::VectisError;

/// Run one validation mode through the core engine.
///
/// # Errors
///
/// See [`specify_vectis_core::validate::run`].
pub fn run(args: &ValidateArgs) -> Result<Value, VectisError> {
    specify_vectis_core::validate::run(args.mode, args.path.as_deref())
}

/// Render a validation outcome as pretty-printed JSON, without a trailing
/// newline, and return the process exit code that should accompany it.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = validate_exit_code(&value);
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

// `render_json` (success / typed-error envelope + exit code) and the core's
// `validate_exit_code` (recursion through the `all` results→report→errors
// tree) are the CLI's dispatch surface, exercised end-to-end by
// `tests/cli.rs` (`assets_clean_run_exits_zero` — exit 0;
// `missing_input_exits_two` — exit 2 with an `invalid-project` body) and
// `tests/engine/paths.rs` (`all_envelope_propagates_sub_errors` — exit 1).
