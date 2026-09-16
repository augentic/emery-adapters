//! Extracts claims from an operator's written brief.
//!
//! An intent source is the operator's free-form brief, given inline or as a
//! one-file tree. It is never split: [`survey`] carries the brief verbatim
//! as one seam, and the guest hands it to `emery_sdk::mine`, whose document
//! carries one `intent` claim with the brief and one `requirement` claim per
//! directive it states. The guest exports the `source-adapter` world on
//! `wasm32` alone, so the survey is tested natively.

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
    use emery_sdk::model::WasiModel;
    use emery_sdk::{Context, SourceInput, SourceKind};

    use crate::{prose, survey};

    struct Adapter;
    export::export!(Adapter with_types_in export);

    impl Guest for Adapter {
        fn metadata(_id: AdapterId) -> AdapterMetadata {
            export::metadata(SourceKind::Intent)
        }

        async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
            let input = SourceInput::from(input);
            let ctx = Context {
                adapter_id: &id,
                input: &input,
            };
            let seams = survey::survey(&input.content)?;
            Ok(emery_sdk::mine(&WasiModel, &ctx, prose::docs(), &seams).await?.into())
        }
    }
}

/// The prose corpus for the adapter.
pub mod prose {
    emery_sdk::include_prose!();
}
pub mod survey;
