//! The judgment operations: `survey` and `extract`.
//!
//! Each operation is one schema-gated judgment leg bracketed by
//! deterministic guest code: the core assembles a prompt from the embedded
//! brief (the system channel) plus the call-specific context (the user
//! message), issues a single-shot `create` through the shared
//! [`specify_guest_kit::judgment`] helper, and re-checks the id grammar
//! the answer schema pins after the answer lands. The calls are
//! session-less — all working state lives in the operator's workspace
//! tree and the prompt itself, so the user message tells the spawned
//! agent where its bound source material lives (the `plan.yaml` source
//! binding) rather than assuming any prior conversation. The intent
//! source is degenerate by construction: the binding carries the
//! operator's free-form intent string inline, so both legs echo rather
//! than infer.

use specify_guest_kit::answers::{
    EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, LeadsAnswer, validate_evidence, validate_leads,
};
use specify_guest_kit::seam::{Context, Error, Evidence, Lead};
use specify_guest_kit::{Model, judgment};

use crate::registry;

/// How the spawned agent resolves its bound source material — the
/// session-less state note both prompts carry. Intent bindings are
/// inline: the operator's brief rides in the plan itself, and no source
/// tree is preopened.
const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `intent`; its inline `value` \
                            carries the operator's free-form intent string, verbatim (`path` \
                            is absent for intent bindings — no source tree is bound).";

/// Survey the inline intent binding into its single lead — one
/// schema-gated judgment leg over the embedded `briefs/survey.md`,
/// followed by the deterministic id-grammar tail.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects the request
/// as malformed, and [`Error::Internal`] for other model failures, an
/// answer that does not deserialize, or an answer that fails the
/// deterministic validation tail.
pub async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
    let system = registry::body("briefs/survey.md").to_string();
    let user = format!(
        "Survey the intent source bound to adapter `{id}`.\n\n\
         {BINDING_NOTE}\n\n\
         The lead id is the slice name the plan derived for this binding (the brief's \
         `slice-name` input): resolve it from the `plan.yaml` entry under `slices[]` \
         that binds this source. Re-running this survey replaces the prior lead by its \
         `(source, lead)` pair, exactly as the brief describes — emit the single \
         current lead.\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         exactly one lead whose `synopsis` is the operator's intent string, verbatim, per \
         the brief. The caller persists the lead into `discovery.md`; do not write it \
         yourself.",
        id = ctx.adapter_id,
    );
    let answer: LeadsAnswer =
        judgment(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA).await?;
    validate_leads(&answer.leads)?;
    Ok(answer.leads)
}

/// Extract the lead's Evidence: the single `kind: intent` claim echoing
/// the operator's intent string.
///
/// One schema-gated judgment leg over the embedded `briefs/extract.md`,
/// followed by the deterministic claim-id tail.
///
/// # Errors
///
/// Returns [`Error::InvalidRequest`] when the model rejects the request
/// as malformed, and [`Error::Internal`] for other model failures, an
/// answer that does not deserialize, or an answer that fails the
/// deterministic validation tail.
pub async fn extract<P: Model>(
    model: &P, ctx: &Context<'_>, lead: &Lead,
) -> Result<Evidence, Error> {
    let system = registry::body("briefs/extract.md").to_string();
    let user = format!(
        "Extract Evidence from the intent source bound to adapter `{id}` for this \
         lead:\n\n{lead}\n\n\
         {BINDING_NOTE}\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority: \"intent\"`, one `kind: \"intent\"` claim whose `id` equals the \
         lead id and whose `statement` carries the operator's intent string verbatim, \
         per the brief), without the envelope `lead` key — this call names the lead. \
         The caller persists the document under `.specify/slices/<slice>/evidence/`; do \
         not write it yourself.",
        id = ctx.adapter_id,
        lead = render_lead(lead),
    );
    let evidence: Evidence =
        judgment(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA).await?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}

/// Render the extract call's lead as the brief's lead-block shape.
fn render_lead(lead: &Lead) -> String {
    let topics = if lead.topics.is_empty() {
        String::new()
    } else {
        format!(
            "
- topics: [{}]",
            lead.topics.join(", ")
        )
    };
    format!(
        "- lead: {}
- synopsis: {}{topics}",
        lead.lead, lead.synopsis
    )
}
