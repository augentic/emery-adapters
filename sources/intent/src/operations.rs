//! The judgment operations: `survey` and `extract` — schema-gated
//! legs through [`adapter::judgment`] with id-grammar tails.
//!
//! The intent source is degenerate by construction: the binding
//! carries the operator's free-form intent string inline, so both legs
//! echo rather than infer.

use adapter::answers::{
    EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, LeadsAnswer, validate_evidence, validate_leads,
};
use adapter::seam::{Context, Error, Evidence, Lead, SourceManifest};
use adapter::{Model, judgment};

use crate::registry;

/// Resolve-time `describe` metadata: no compatibility floor.
#[must_use]
pub const fn describe() -> SourceManifest {
    SourceManifest { specify_floor: None }
}

/// Session-less state note both prompts carry. Intent bindings are
/// inline: the operator's brief rides in the plan itself, and no
/// source tree is preopened.
const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `intent`; its inline `value` \
                            carries the operator's free-form intent string, verbatim (`path` \
                            is absent for intent bindings — no source tree is bound).";

/// Survey the inline intent binding into its single lead — one
/// schema-gated leg over `prompts/survey.md`, then the id-grammar
/// tail.
///
/// # Errors
///
/// As [`adapter::judgment`]; a validation-tail failure is
/// [`Error::Internal`].
pub async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
    let system = registry::body("prompts/survey.md").to_string();
    let user = format!(
        "Survey the intent source bound to adapter `{id}`.\n\n\
         {BINDING_NOTE}\n\n\
         The lead id is the slice name the plan derived for this binding (the prompt's \
         `slice-name` input): resolve it from the `plan.yaml` entry under `slices[]` \
         that binds this source. Re-running this survey replaces the prior lead by its \
         `(source, lead)` pair, exactly as the prompt describes — emit the single \
         current lead.\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         exactly one lead whose `synopsis` is the operator's intent string, verbatim, per \
         the prompt. The caller persists the lead into `discovery.md`; do not write it \
         yourself.",
        id = ctx.adapter_id,
    );
    let answer: LeadsAnswer =
        judgment(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA).await?;
    validate_leads(&answer.leads)?;
    Ok(answer.leads)
}

/// Extract the lead's Evidence: the single `kind: intent` claim
/// echoing the operator's intent string. One schema-gated leg over
/// `prompts/extract.md`, then the claim-id tail.
///
/// # Errors
///
/// As [`adapter::judgment`]; a validation-tail failure is
/// [`Error::Internal`].
pub async fn extract<P: Model>(
    model: &P, ctx: &Context<'_>, lead: &Lead,
) -> Result<Evidence, Error> {
    let system = registry::body("prompts/extract.md").to_string();
    let user = format!(
        "Extract Evidence from the intent source bound to adapter `{id}` for this \
         lead:\n\n{lead}\n\n\
         {BINDING_NOTE}\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority: \"intent\"`, one `kind: \"intent\"` claim whose `id` equals the \
         lead id and whose `statement` carries the operator's intent string verbatim, \
         per the prompt), without the envelope `lead` key — this call names the lead. \
         The caller persists the document under `.specify/slices/<slice>/evidence/`; do \
         not write it yourself.",
        id = ctx.adapter_id,
        lead = lead.render(),
    );
    let evidence: Evidence =
        judgment(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA).await?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}
