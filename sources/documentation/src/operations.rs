use emery_adapter::types::{Context, Evidence, SourceInput};
use emery_adapter::{Error, Model, SourceAdapter, content_note, evidence};
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
        let user = format!(
            "Extract the claim set of the documentation source bound to adapter `{id}` \
             (source key `{key}`).\n\n\
             {content}\n\n\
             Answer with one JSON object matching the gated schema: the Evidence body \
             (`authority`, `claims`) the prompt describes. The caller persists the \
             document; do not write it yourself.",
            id = ctx.adapter_id,
            key = input.key,
            content = content_note(input, "the documentation tree"),
        );
        evidence(model, ctx, system, user).await
    }
}
