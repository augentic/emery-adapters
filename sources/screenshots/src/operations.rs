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
            "screenshots reads a tree (`path:`) binding; got an inline value".to_string(),
        ));
    }
    Ok(format!(
        "Source binding key: `{key}`. The engine resolved the binding and lent its \
         prepared directory of screen images to you as your working directory — the \
         working directory you were given is the read-only tree the prompt calls \
         `$SOURCE_DIR`; do not resolve the binding yourself. The prompt's vision \
         prerequisite applies: read the images themselves, never fall back to filename \
         or metadata inference."
    ))
}

/// Screen images → per-screen leads and spatial Evidence.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Source for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "screenshots",
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
            "Survey the screenshots source bound to adapter `{id}`.\n\n\
         {note}\n\n\
         When the caller already holds leads for this source, treat this call as a \
         re-survey: return the complete current lead set — the caller replaces prior \
         leads by their `(source, lead)` pairs, exactly as the prompt describes.\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         the same `lead` / `synopsis` / optional `topics` content as the prompt's lead \
         blocks. The caller persists the leads into `discovery.md`; do not write it \
         yourself.",
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
            "Extract Evidence from the screenshots source bound to adapter `{id}` for this \
         lead (one screen, possibly with state and platform variants):\n\n{lead}\n\n\
         {note}\n\n\
         Run the prompt's spatial-inference pipeline (`prompts/extract/pipeline.md`, \
         served over this call's MCP grant) against the lead's image(s).\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority`, `claims`) the prompt describes, without the envelope `lead` key — \
         this call names the lead. The caller persists the document under \
         `.emery/slices/<slice>/evidence/`; do not write it yourself.",
            id = ctx.adapter_id,
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
