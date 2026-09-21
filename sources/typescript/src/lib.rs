//! Extracts behavioural claims from TypeScript and JavaScript source.
//!
//! Workspace inputs are divided into exposed surfaces, such as routes,
//! commands, jobs, and exported APIs. Each surface is mined independently
//! from its entry module, with the full source tree available.
//!
//! Surface discovery is model-assisted and runs before any surface is mined.
//!
//! Inline inputs are mined as a whole. Workspaces with no exposed surfaces
//! are rejected.

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

pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose![
    "../prose/extract.md",
    "../prose/survey.md",
    "../prose/references/business-logic.md",
    "../prose/references/component-structure.md",
    "../prose/references/examples/README.md",
    "../prose/references/examples/branching-caching.md",
    "../prose/references/examples/outbound-http.md",
    "../prose/references/examples/parallel-execution.md",
    "../prose/references/external-api.md",
    "../prose/references/observability.md",
    "../prose/references/services.md",
    "../prose/references/types.md",
    "../prose/references/verification.md",
];
