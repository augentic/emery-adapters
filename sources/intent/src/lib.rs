//! Extracts claims from an operator's written brief.
//!
//! The source is inline text or a workspace holding one regular file. The
//! brief is mined whole, in one call.
//!
//! # Refusals
//!
//! - an empty brief
//! - a workspace holding no file, or more than one, hidden entries and
//!   Emery's own files left out of the count

mod survey;

use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, SourceKind};

/// The adapter implementation.
pub struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Intent;

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx.input)?;
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}

/// The prompt embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose!["../prose/extract.md"];

// Export the adapter as a WASM module.
#[cfg(target_arch = "wasm32")]
emery_sdk::source_adapter!(Adapter);
