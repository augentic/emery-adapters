//! Intent bindings carry the operator's free-form brief inline, so both
//! legs echo rather than infer.

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceInput, SourceMetadata};
use adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

/// The binding key and inline intent string from the engine-prepared
/// call, rejecting a tree-form input (intent bindings are `value:`-only).
fn binding<'a>(ctx: &'a Context<'_>, input: &'a SourceInput) -> Result<(&'a str, &'a str), Error> {
    let key = ctx.source_key.as_deref().ok_or_else(|| {
        Error::InvalidRequest("source dispatch carries no source-binding key".to_string())
    })?;
    let intent = input.content().ok_or_else(|| {
        Error::InvalidRequest("intent reads an inline `value:` binding; got a tree".to_string())
    })?;
    Ok((key, intent))
}

/// Inline intent binding → one lead and one `kind: intent` claim.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl Source for Adapter {
    const IDENTITY: AdapterIdentity = AdapterIdentity {
        name: "intent",
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
        let (key, intent) = binding(ctx, input)?;
        let system = registry::body("prompts/survey.md").to_string();
        let user = format!(
            "Survey the intent source bound to adapter `{id}`.\n\n\
         Source binding key: `{key}`. The engine resolved the binding and passed its \
         inline `value` — the operator's free-form intent string, verbatim (no source \
         tree is bound for intent):\n\n\
         {intent}\n\n\
         The lead id is a stable kebab-case slug derived from the intent string itself, \
         per the prompt's slug rules. Re-running this survey replaces the prior lead by \
         its `(source, lead)` pair, exactly as the prompt describes — emit the single \
         current lead.\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         exactly one lead whose `synopsis` is the operator's intent string, verbatim, per \
         the prompt. The caller persists the lead into `discovery.md`; do not write it \
         yourself.",
            id = ctx.adapter_id,
        );
        repaired(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA, leads_tail).await
    }

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput, lead: &Lead,
    ) -> Result<Evidence, Error> {
        let (key, intent) = binding(ctx, input)?;
        let system = registry::body("prompts/extract.md").to_string();
        let user = format!(
            "Extract Evidence from the intent source bound to adapter `{id}` for this \
         lead:\n\n{lead}\n\n\
         Source binding key: `{key}`. The engine resolved the binding and passed its \
         inline `value` — the operator's intent string, verbatim (no source tree is \
         bound for intent):\n\n\
         {intent}\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority: \"intent\"`, one `kind: \"intent\"` claim whose `id` equals the \
         lead id and whose `statement` carries the operator's intent string verbatim, \
         per the prompt), without the envelope `lead` key — this call names the lead. \
         The caller persists the document under `.emery/slices/<slice>/evidence/`; do \
         not write it yourself.",
            id = ctx.adapter_id,
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
