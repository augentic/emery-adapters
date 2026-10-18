//! Extracts claims from a tree of written documentation.
//!
//! An inline value is mined whole in one call. A workspace is surveyed for
//! the subjects its documents introduce and mined in one call within the
//! inline budget, or one call per stem past it, falling back to directory
//! groups when no subject is named.

mod survey;

use emery_sdk::{Context, Error, Evidence, Metadata, Model, SourceAdapter, SourceKind};

/// The adapter implementation.
pub struct Adapter;

impl SourceAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata { kind: SourceKind::Documentation }
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = survey::survey(ctx).await?;
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}

/// The prompts embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] =
    emery_sdk::prose!["../prose/extract.md", "../prose/survey.md"];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_source!(Adapter);
