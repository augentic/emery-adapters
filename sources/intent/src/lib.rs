//! Extracts claims from an operator's written brief.
//!
//! An intent source is the operator's free-form brief, given inline or as a
//! one-file tree. It is never split: [`survey`] carries the brief verbatim
//! as one seam, and the guest's `extract` hands it to `emery_sdk::mine`,
//! whose document carries one `intent` claim with the brief and one
//! `requirement` claim per directive it states. The guest exports the
//! `source-adapter` world on `wasm32` alone, so the survey is tested
//! natively, and [`DOCS`] lists the prose it embeds, so the root suite holds
//! the list to the `prose/` tree.

pub mod survey;

use emery_sdk::Doc;

/// The prose the guest embeds: the extraction prompt and the references it links.
pub static DOCS: &[Doc] = emery_sdk::prose!(
    "../prose",
    [
        "prompts/extract.md",
        "references/emery-runtime/README.md",
        "references/emery-runtime/claims.md",
        "references/emery-runtime/reconciliation.md",
    ]
);

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, SourceKind};

    use crate::{DOCS, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Intent)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx.input)?;
        emery_sdk::mine(ctx, DOCS, &seams).await
    }
}
