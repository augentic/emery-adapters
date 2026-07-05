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
//! binding) rather than assuming any prior conversation. The judgment
//! detail — the vision prerequisite, the spatial-inference pipeline, the
//! `region` / `container` / `leaf` claim kinds — rides in the embedded
//! briefs.

use specify_guest_kit::answers::{
    EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, LeadsAnswer, validate_evidence, validate_leads,
};
use specify_guest_kit::seam::{Context, Error, Evidence, Lead};
use specify_guest_kit::{Model, judgment};

use crate::registry;

/// How the spawned agent resolves its bound source material — the
/// session-less state note both prompts carry.
const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `screenshots`; its `path` \
                            (relative to the workspace root) is the read-only directory of \
                            screen images the brief calls `$SOURCE_DIR`. The brief's vision \
                            prerequisite applies: read the images themselves, never fall back \
                            to filename or metadata inference.";

/// Survey the bound screen-image set into leads, one per screen.
///
/// One schema-gated judgment leg over the embedded `briefs/survey.md`
/// (vision inference rides in the brief), followed by the deterministic
/// id-grammar tail.
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
        "Survey the screenshots source bound to adapter `{id}`.\n\n\
         {BINDING_NOTE}\n\n\
         When `discovery.md` at the workspace root already carries leads for this source \
         under `## Lead inventory`, treat this call as a re-survey: return the complete \
         current lead set — the caller replaces prior leads by their `(source, lead)` \
         pairs, exactly as the brief describes.\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         the same `lead` / `synopsis` / optional `topics` content as the brief's lead \
         blocks. The caller persists the leads into `discovery.md`; do not write it \
         yourself.",
        id = ctx.adapter_id,
    );
    let answer: LeadsAnswer =
        judgment(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA).await?;
    validate_leads(&answer.leads)?;
    Ok(answer.leads)
}

/// Extract one lead's spatial Evidence from the bound screen images.
///
/// One schema-gated judgment leg over the embedded `briefs/extract.md`
/// (emitting `region` / `container` / `leaf` claims), followed by the
/// deterministic claim-id tail.
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
        "Extract Evidence from the screenshots source bound to adapter `{id}` for this \
         lead (one screen, possibly with state and platform variants):\n\n{lead}\n\n\
         {BINDING_NOTE}\n\n\
         Run the brief's spatial-inference pipeline (`briefs/extract/pipeline.md`, \
         served over this call's MCP grant) against the lead's image(s).\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority`, `claims`) the brief describes, without the envelope `lead` key — \
         this call names the lead. The caller persists the document under \
         `.specify/slices/<slice>/evidence/`; do not write it yourself.",
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
