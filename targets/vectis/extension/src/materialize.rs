//! `vectis materialize` subcommand surface — canonical-to-export asset
//! conversion.
//!
//! The conversion funnels (SVG parse / rasterisation, export layouts,
//! auto-pins) moved to `specify-vectis-core` (RFC-61 Step 3); this
//! module keeps the WASI command surface — argument parsing and the
//! JSON envelope — and delegates each run to the core.

use std::path::PathBuf;

use clap::{Args as ClapArgs, Subcommand};
use serde_json::Value;
pub use specify_vectis_core::materialize::{
    collect_paths, materialize_exit_code, parse_vector_svg, paths,
};

use crate::{VectisError, render_json as render_value};

/// Nested targets under `vectis materialize`.
#[derive(Subcommand, Debug, Clone, PartialEq, Eq)]
pub enum MaterializeCommand {
    /// Convert canonical asset masters into per-platform exports.
    Assets(AssetsArgs),
}

/// Arguments for `vectis materialize assets`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct AssetsArgs {
    /// Path to `assets.yaml`. Defaults to the design-system cascade.
    pub path: Option<PathBuf>,

    /// Comma-separated platform filter (`ios`, `android`). Defaults to both.
    #[arg(long, value_delimiter = ',')]
    pub platform: Option<Vec<String>>,

    /// Report planned writes without creating files or auto-writing pins.
    #[arg(long)]
    pub dry_run: bool,

    /// Limit materialization to these asset ids (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub only: Option<Vec<String>>,
}

impl AssetsArgs {
    /// Project the parsed CLI arguments onto the core's request shape.
    fn to_core(&self) -> specify_vectis_core::materialize::AssetsArgs {
        specify_vectis_core::materialize::AssetsArgs {
            path: self.path.clone(),
            platform: self.platform.clone(),
            dry_run: self.dry_run,
            only: self.only.clone(),
        }
    }
}

/// Dispatch a parsed [`MaterializeCommand`] through the core.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the resolved `assets.yaml`
/// is missing or unreadable, or when `--platform` carries an unknown token.
pub fn run(command: &MaterializeCommand) -> Result<Value, VectisError> {
    match command {
        MaterializeCommand::Assets(args) => specify_vectis_core::materialize::run(
            &specify_vectis_core::materialize::MaterializeCommand::Assets(args.to_core()),
        ),
    }
}

/// Render a materialize outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => {
            let code = materialize_exit_code(&value);
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
