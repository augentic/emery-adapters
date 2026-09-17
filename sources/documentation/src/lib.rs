//! Extracts claims from a tree of written documentation.
//!
//! A documentation source is a written tree — specifications, guides,
//! decision records. [`survey`] cuts it one top-level directory at a time,
//! and the guest's `extract` hands the seams to `emery_sdk::mine`, which
//! lends each call the root, tells it the directory's files to mine, and
//! joins the claims into one document. The guest exports the
//! `source-adapter` world on `wasm32` alone, so the survey is tested
//! natively, and [`DOCS`] lists the prose it embeds, so the root suite holds
//! the list to the `prose/` tree.

pub mod survey;

use emery_sdk::Doc;

/// The prose the guest embeds: the extraction prompt, the rule it applies, and the references it links.
pub static DOCS: &[Doc] = emery_sdk::prose!(
    "../prose",
    [
        "prompts/extract.md",
        "references/emery-runtime/README.md",
        "references/emery-runtime/claims.md",
        "references/emery-runtime/reconciliation.md",
        "rules/documentation-verbatim-preservation.md",
    ]
);

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
        emery_sdk::mine(ctx, DOCS, &seams).await
    }
}
