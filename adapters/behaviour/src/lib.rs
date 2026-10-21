//! Extracts screen behaviour from ui-ir `behaviour.json` documents.
//!
//! A workspace holds one or more `behaviour.json` files, written beside the
//! source tree by ui-ir's `code-to-behaviour` and `capture-to-behaviour`
//! (`augentic/ui-ir-experiment`; a browser cannot run in a guest). The
//! documents are read as plain JSON, so this crate has no dependency on
//! ui-ir. Their `file#L` evidence is relative to the workspace root. Each
//! document is one seam: the model states the meaning
//! (what a trigger, a flow and the screen are for) as `requirement` and
//! `criterion` claims anchored where the document's evidence points, each
//! marked `inferred` with its producer. The document's facts are then joined
//! as `call` and `example` claims without a model turn. A workspace with no
//! `behaviour.json` is refused.

mod evidence;
mod survey;

use emery_sdk::{Context, Error, Evidence, Metadata, Model, SourceAdapter, SourceKind};

/// The adapter implementation.
pub struct Adapter;

impl SourceAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            kind: SourceKind::Behaviour,
        }
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let survey = survey::survey(ctx)?;
        let meaning = emery_sdk::extract(ctx, PROSE, &survey.seams).await?;
        Ok(survey.join(meaning))
    }
}

/// The prompt embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose!["../prose/extract.md"];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_source!(Adapter);
