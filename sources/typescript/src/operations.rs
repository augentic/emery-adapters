use emery_adapter::types::{Context, Evidence, SourceInput};
use emery_adapter::{Error, Model, SourceAdapter, content_note, evidence};
use emery_prose::registry::Doc;

use crate::registry;

/// TypeScript / JavaScript source trees → one code Evidence document.
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
            "Extract the claim set of the TypeScript / JavaScript source bound to \
             adapter `{id}` (source key `{key}`).\n\n\
             {content}\n\n\
             The prompt's references are available through this call's `read_doc` tool \
             (`list_docs` enumerates them) — load the reference bodies on demand when a \
             surface needs deeper analysis.\n\n\
             Answer with one JSON object matching the gated schema: the Evidence body \
             (`authority: \"behaviour\"`, `claims`) the prompt describes — every \
             spec-worthy behaviour lifted into a `requirement` claim with a `statement`, \
             backed by `excerpt` / `type` / `call` detail claims. The caller persists \
             the document; do not write it yourself.",
            id = ctx.adapter_id,
            key = input.key,
            content = content_note(input, "the TypeScript / JavaScript source tree"),
        );
        evidence(model, ctx, system, user).await
    }
}
