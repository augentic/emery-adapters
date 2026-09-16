//! Extracts claims from a tree of written documentation.
//!
//! A documentation source is a written tree — specifications, guides,
//! decision records. [`survey`] cuts it one top-level directory at a time,
//! and the guest hands the seams to `emery_sdk::mine`, which lends each its
//! own directory and joins the claims into one document. The guest exports
//! the `source-adapter` world on `wasm32` alone, so the survey is tested
//! natively.

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
            export::metadata(SourceKind::Documentation)
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
