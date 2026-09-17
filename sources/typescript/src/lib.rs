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
//! surface is refused rather than mined. The survey and `emery_sdk::mine`
//! alike ask the model the call's context carries, and the guest exports the
//! `source-adapter` world on `wasm32` alone, so the survey is tested natively
//! over a scripted model. [`DOCS`] lists the prose the guest embeds, so the
//! root suite holds the list to the `prose/` tree and the survey suite runs
//! under the same `prompts/survey.md`.

pub mod survey;

use emery_sdk::Doc;

/// The prose the guest embeds: both prompts and the references they link.
pub static DOCS: &[Doc] = emery_sdk::prose!(
    "../prose",
    [
        "prompts/extract.md",
        "prompts/survey.md",
        "references/business-logic.md",
        "references/component-structure.md",
        "references/context-gaps.md",
        "references/dependencies.md",
        "references/design-template.md",
        "references/emery-runtime/README.md",
        "references/emery-runtime/claims.md",
        "references/emery-runtime/reconciliation.md",
        "references/examples/README.md",
        "references/examples/branching-caching.md",
        "references/examples/outbound-http.md",
        "references/examples/parallel-execution.md",
        "references/external-api.md",
        "references/language-mapping.md",
        "references/lessons-learned.md",
        "references/observability.md",
        "references/semantic-search.md",
        "references/verification.md",
    ]
);

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, SourceKind};

    use crate::{DOCS, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Behaviour)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx, DOCS).await?;
        emery_sdk::mine(ctx, DOCS, &seams).await
    }
}
