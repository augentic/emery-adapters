//! Extracts claims from a TypeScript or JavaScript code tree.
//!
//! A code tree is mined one externally visible surface at a time — a route, a
//! command, a job, an exported API — with the whole tree in view. Which
//! modules serve one surface is no directory layout's to state, so
//! [`survey`] asks the model once under the `prompts/survey.md` it is handed; and a handler's
//! behaviour runs through its imports and `tsconfig.json`, so each seam is
//! lent the root and told which files are its own. The guest binds the
//! host's model once, hands the seams to `emery_sdk::mine`, and exports the
//! `source-adapter` world on `wasm32` alone, so the survey is tested natively.

pub mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
    use emery_sdk::{Doc, Model, SourceKind};

    use crate::survey;

    static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");

    // The adapter's capabilities on the WASI defaults: the model alone.
    struct Provider;
    impl Model for Provider {}

    struct Adapter;
    export::export!(Adapter with_types_in export);

    impl Guest for Adapter {
        fn metadata(_id: AdapterId) -> AdapterMetadata {
            emery_sdk::metadata(SourceKind::Behaviour)
        }

        async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
            emery_sdk::extract(&Provider, id, input, DOCS, async |model, ctx| {
                survey::survey(model, ctx, DOCS).await
            })
            .await
        }
    }
}
