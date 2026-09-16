//! Extracts claims from a TypeScript or JavaScript code tree.
//!
//! A code tree is mined one exposed surface at a time — a route, a command, a
//! job, an exported API — with the whole tree in view. Where the source's
//! boundary lies is no directory layout's to state, so [`survey`] asks the
//! model once, under the `prompts/survey.md` it is handed, for the surfaces
//! and the module a caller enters each at, and nothing behind them; a
//! surface's behaviour runs from its entry through imports and
//! `tsconfig.json`, so each seam is lent the root and told its surface and
//! entry, and the extract call follows the rest. A tree that exposes no
//! surface is refused rather than mined. The guest binds the host's model
//! once, lends it to the survey and to `emery_sdk::mine` alike from its
//! `extract`, and exports the `source-adapter` world on `wasm32` alone, so
//! the survey is tested natively.

pub mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Doc, Error, Evidence, Model, SourceKind};

    use crate::survey;

    static DOCS: &[Doc] = emery_sdk::include_prose!("../prose");

    // The adapter's capabilities on the WASI defaults: the model alone.
    struct Provider;
    impl Model for Provider {}

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Behaviour)
    }

    async fn extract(ctx: &Context<'_>) -> Result<Evidence, Error> {
        let seams = survey::survey(&Provider, ctx, DOCS).await?;
        emery_sdk::mine(&Provider, ctx, DOCS, &seams).await
    }
}
