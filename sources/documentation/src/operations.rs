use emery_adapter::types::{Context, Evidence, SourceInput};
use emery_adapter::{Error, EvidenceTurn, Model, SourceAdapter, evidence};
use emery_prose::registry::Doc;

use crate::registry;

/// Written specifications / documentation trees → one Evidence document.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput,
    ) -> Result<Evidence, Error> {
        let system = registry::body("prompts/extract.md");
        let turn = EvidenceTurn::bound("documentation", "the documentation tree");
        evidence(model, ctx, input, system, turn).await
    }
}
