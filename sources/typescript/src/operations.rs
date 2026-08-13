use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceInput, SourceMetadata};
use adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

/// Frame the engine-prepared binding for the model: the binding key,
/// plus the lent tree mapped onto the prompt's `$SOURCE_DIR` vocabulary.
fn binding_note(ctx: &Context<'_>, input: &SourceInput) -> Result<String, Error> {
    let key = ctx.source_key.as_deref().ok_or_else(|| {
        Error::InvalidRequest("source dispatch carries no source-binding key".to_string())
    })?;
    if input.root().is_none() {
        return Err(Error::InvalidRequest(
            "typescript reads a tree (`path:`) binding; got an inline value".to_string(),
        ));
    }
    Ok(format!(
        "Source binding key: `{key}`. The engine resolved the binding and lent its \
         prepared TypeScript / JavaScript source tree to you as your working directory — \
         the working directory you were given is the read-only tree the prompt calls \
         `$SOURCE_DIR`. Treat that tree as read-only; every input you need is the tree \
         and this prompt — do not resolve the binding yourself."
    ))
}

/// TypeScript / JavaScript source trees → leads and code Evidence.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Source for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "typescript",
        version: env!("CARGO_PKG_VERSION"),
    };

    fn metadata() -> SourceMetadata {
        SourceMetadata {
            emery_floor: Some("0.38.0".to_string()),
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn survey<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput,
    ) -> Result<Vec<Lead>, Error> {
        let note = binding_note(ctx, input)?;
        let system = registry::body("prompts/survey.md").to_string();
        let user = format!(
            "Survey the TypeScript / JavaScript source bound to adapter `{id}` using the \
         prompt's framework grammar.\n\n\
         {note}\n\n\
         When the caller already holds leads for this source, treat this call as a \
         re-survey: return the complete current lead set — the caller replaces prior \
         leads by their `(source, lead)` pairs (the prompt's stable re-survey \
         handle).\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         the same `lead` / `synopsis` / optional `topics` content as the prompt's lead \
         blocks. The engine persists the leads; do not write them yourself.",
            id = ctx.adapter_id,
        );
        repaired(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA, leads_tail).await
    }

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput, lead: &Lead,
    ) -> Result<Evidence, Error> {
        let note = binding_note(ctx, input)?;
        let system = registry::body("prompts/extract.md").to_string();
        let user = format!(
            "Extract Evidence from the TypeScript / JavaScript source bound to adapter \
         `{id}` for this lead:\n\n{lead}\n\n\
         {note}\n\n\
         The prompt's references is served over this call's MCP grant — load the \
         reference bodies on demand when the lead's surface needs deeper analysis.\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority`, `claims`) the prompt describes, without the envelope `lead` key — \
         this call names the lead. The engine persists the document; do not write it \
         yourself.",
            id = ctx.adapter_id,
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
