//! Extracts claims from a TypeScript or JavaScript code tree.
//!
//! A code tree is mined one externally visible surface at a time — a route, a
//! command, a job, an exported API — with the whole tree in view. Which
//! modules serve one surface is no directory layout's to state, so
//! [`survey`] asks the model once under `prompts/survey.md`; and a handler's
//! behaviour runs through its imports and `tsconfig.json`, so each seam is
//! lent the root and told which files are its own. The guest hands the
//! seams to `emery_sdk::mine` and exports the `source-adapter` world on
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
            export::metadata(SourceKind::Behaviour)
        }

        async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
            let input = SourceInput::from(input);
            let ctx = Context {
                adapter_id: &id,
                input: &input,
            };
            let seams = survey::survey(&WasiModel, &ctx).await?;
            Ok(emery_sdk::mine(&WasiModel, &ctx, prose::docs(), &seams).await?.into())
        }
    }
}

/// The prose corpus for the adapter.
pub mod prose {
    emery_sdk::include_prose!();
}
pub mod survey;
