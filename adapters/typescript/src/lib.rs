//! Extracts behavioural claims from TypeScript and JavaScript source.
//!
//! An inline value is mined whole in one call. A workspace's production
//! modules are parsed and surveyed into seams by surface stem, with exported
//! declarations joined as `type` claims. A workspace with no production
//! module is refused.

mod survey;

use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, SourceKind};

/// The adapter implementation.
pub struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Behaviour;

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let survey = survey::survey(ctx).await?;
        let evidence = emery_sdk::extract(ctx, PROSE, &survey.seams).await?;
        Ok(survey.join(evidence))
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

#[cfg(target_arch = "wasm32")]
emery_sdk::export_source!(Adapter);
