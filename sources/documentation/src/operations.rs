use emery_adapter::answers::{EVIDENCE_ANSWER_SCHEMA, evidence_tail};
use emery_adapter::registry::Doc;
use emery_adapter::seam::{Context, Error, Evidence, SourceContent, SourceInput, SourceMetadata};
use emery_adapter::{Model, SourceAdapter, repaired};

use crate::registry;

/// Written specifications / documentation trees → one Evidence document.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
    const IDENTITY: &str = concat!("documentation@", env!("CARGO_PKG_VERSION"));

    fn metadata() -> SourceMetadata {
        SourceMetadata {
            emery_floor: Some("0.38.0".to_string()),
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput,
    ) -> Result<Evidence, Error> {
        let system = registry::body("prompts/extract.md").to_string();
        let user = format!(
            "Extract the claim set of the documentation source bound to adapter `{id}` \
             (source key `{key}`).\n\n\
             {content}\n\n\
             Answer with one JSON object matching the gated schema: the Evidence body \
             (`authority`, `claims`) the prompt describes. The caller persists the \
             document; do not write it yourself.",
            id = ctx.adapter_id,
            key = input.key,
            content = content_note(input),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}

fn content_note(input: &SourceInput) -> String {
    match &input.content {
        SourceContent::Workspace(view) => format!(
            "`$SOURCE_DIR` is the read-only view at `{}` — the documentation tree the \
             prompt walks. Nothing outside it is reachable; extract mines only this \
             source.",
            view.root
        ),
        SourceContent::Value(value) => format!(
            "The bound material is this inline value; no `$SOURCE_DIR` is \
             lent:\n\n{value}\n\n\
             Nothing else is reachable; extract mines only this source."
        ),
    }
}
