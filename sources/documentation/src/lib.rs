//! Extracts claims from a tree of written documentation.
//!
//! Workspace inputs are grouped by top-level directory. Directories containing
//! fewer than two documents are combined. If fewer than two groups remain,
//! the entire input is mined as a whole.
//!
//! Inline inputs are always mined as a whole.

#[cfg(target_arch = "wasm32")]
mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, SourceKind};

    use crate::{DOCS, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Documentation)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx.input)?;
        emery_sdk::extract(ctx, DOCS, &seams).await
    }
}

/// The prompt and reference documents embedded in the adapter.
pub static DOCS: &[emery_sdk::Doc] = emery_sdk::prose!(
    "../prose",
    [
        "prompts/extract.md",
        "references/emery-runtime/claims.md",
        "references/emery-runtime/reconciliation.md",
    ]
);
