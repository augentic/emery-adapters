//! Extracts claims from an operator's written brief.
//!
//! The source is inline text or a workspace holding one regular file, mined
//! whole in one call. An empty brief, or a workspace holding no file or more
//! than one, is refused.

mod survey;

use emery_sdk::{Context, Error, Evidence, Metadata, Model, SourceAdapter, SourceKind};

/// The adapter implementation.
pub struct Adapter;

impl SourceAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata { kind: SourceKind::Intent }
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx.input)?;
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}

/// The prompt embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose!["../prose/extract.md"];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_source!(Adapter);
