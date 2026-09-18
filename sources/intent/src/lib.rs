//! Extracts claims from an operator's written brief.
//!
//! The adapter accepts inline text or a workspace containing one regular
//! file. The brief must not be empty and is always mined as a single unit.
//! Emery's generated files are ignored when counting workspace files.

#[cfg(target_arch = "wasm32")]
mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, SourceKind};

    use crate::{PROSE, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Intent)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx.input)?;
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}

/// The prompt embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose!["prompts/extract.md"];
