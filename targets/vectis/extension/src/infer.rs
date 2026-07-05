//! `vectis infer` subcommand surface.
//!
//! The deterministic clustering engine moved to `specify-vectis-core`
//! (RFC-61 Step 5 Milestone A1); this module keeps the WASI command
//! surface — argument parsing and the JSON envelope — and delegates the
//! run to the core.

use std::path::PathBuf;

use clap::Args as ClapArgs;
use serde_json::Value;

use crate::{VectisError, render_json as render_value};

/// Arguments accepted by `vectis infer`.
#[derive(ClapArgs, Debug, Clone, PartialEq, Eq)]
pub struct InferArgs {
    /// Composition baseline to cluster (`.specify/specs/composition.yaml`).
    #[arg(long)]
    pub composition: PathBuf,

    /// Candidate-cache directory: screenshot stage-6 candidate
    /// skeletons, keyed by provenance, folded into clustering.
    #[arg(long)]
    pub candidate_cache: Option<PathBuf>,

    /// Operator parts file: authoritative parts that seed inference
    /// with naming + promotion authority.
    #[arg(long)]
    pub parts: Option<PathBuf>,

    /// Minimum distinct screens a group must span to cluster.
    #[arg(long, default_value_t = specify_vectis_core::infer::DEFAULT_MIN_OCCURRENCES)]
    pub min_occurrences: u32,
}

impl InferArgs {
    /// Project the parsed CLI arguments onto the core's request shape.
    fn to_core(&self) -> specify_vectis_core::infer::InferArgs {
        specify_vectis_core::infer::InferArgs {
            composition: self.composition.clone(),
            candidate_cache: self.candidate_cache.clone(),
            parts: self.parts.clone(),
            min_occurrences: self.min_occurrences,
        }
    }
}

/// Run the inference engine through the core.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the composition file is
/// unreadable or is not valid YAML.
pub fn run(args: &InferArgs) -> Result<Value, VectisError> {
    specify_vectis_core::infer::run(&args.to_core())
}

/// Render an infer outcome as pretty-printed JSON and exit code.
#[must_use]
pub fn render_json(outcome: Result<Value, VectisError>) -> (String, u8) {
    match outcome {
        Ok(value) => (render_value(&value), 0),
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
