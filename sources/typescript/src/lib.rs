//! Extracts behavioural claims from TypeScript and JavaScript source.
//!
//! A workspace whose production modules fit within the SDK's inline budget is
//! mined whole in one call over those modules, laid into the turn, as a
//! single-module workspace is; inline input is mined whole too. A larger
//! workspace is divided into exposed surfaces, such as routes, commands, jobs,
//! and exported APIs. Each surface is mined independently from its entry
//! module, with the full source tree available, and its `requirement` and
//! `criterion` ids are held to the stem the survey gave it.
//!
//! Surface discovery is model-assisted and runs before any surface is mined.
//!
//! A workspace with no production module or no exposed surface is rejected.

#[cfg(target_arch = "wasm32")]
mod survey;

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, SourceKind};

    use crate::{PROSE, survey};

    emery_sdk::source_adapter!(metadata, extract);

    fn metadata() -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Behaviour)
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx, PROSE).await?;
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}

/// The prompts and reference documents embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose![
    "../prose/extract.md",
    "../prose/survey.md",
    "../prose/references/examples/README.md",
    "../prose/references/examples/branching-caching.md",
    "../prose/references/examples/outbound-http.md",
    "../prose/references/examples/parallel-execution.md",
];
