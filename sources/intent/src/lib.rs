//! Extracts claims from an operator's written brief.
//!
//! An intent source is the operator's free-form brief, given inline or as a
//! one-file tree. It is never split: [`survey`] carries the brief verbatim
//! as one seam, and the guest hands it to `emery_sdk::mine`, whose document
//! carries one `intent` claim with the brief and one `requirement` claim per
//! directive it states. The guest exports the `source-adapter` world on
//! `wasm32` alone, so the survey is tested natively.

pub mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
    use emery_sdk::{Doc, SourceKind};

    use crate::survey;

    // The extraction prompt and its references, from the tree beside `src/`.
    static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");

    struct Adapter;
    export::export!(Adapter with_types_in export);

    impl Guest for Adapter {
        fn metadata(_id: AdapterId) -> AdapterMetadata {
            emery_sdk::metadata(SourceKind::Intent)
        }

        async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
            emery_sdk::extract(id, input, DOCS, async |ctx| survey::survey(&ctx.input.content))
                .await
        }
    }
}
