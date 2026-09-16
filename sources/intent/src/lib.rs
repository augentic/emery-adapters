//! Extracts claims from an operator's written brief.
//!
//! An intent source is the operator's free-form brief, given inline or as a
//! one-file tree. It is never split: [`survey`] carries the brief verbatim
//! as one seam, and the guest's `extract` hands it to `emery_sdk::mine`,
//! whose document carries one `intent` claim with the brief and one
//! `requirement` claim per directive it states. The guest exports the
//! `source-adapter` world on `wasm32` alone, so the survey is tested
//! natively.

pub mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Doc, Error, Evidence, Model, SourceKind};

    use crate::survey;

    // The extraction prompt and its references, from the tree beside `src/`.
    static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");

    // The adapter's capabilities on the WASI defaults: the model alone.
    struct Provider;
    impl Model for Provider {}

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Intent)
    }

    async fn extract(ctx: &Context<'_>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx)?;
        emery_sdk::mine(&Provider, ctx, DOCS, &seams).await
    }
}
