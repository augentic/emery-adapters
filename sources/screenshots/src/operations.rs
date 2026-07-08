//! The judgment operations: `survey` and `extract` — schema-gated
//! legs through [`adapter::judgment`] with id-grammar tails.
//!
//! The judgment detail — the vision prerequisite, the
//! spatial-inference pipeline, the `region` / `container` / `leaf`
//! claim kinds — rides in the embedded prompts.

use adapter::answers::{
    EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, LeadsAnswer, validate_evidence, validate_leads,
};
use adapter::seam::{Context, Error, Evidence, Lead, SourceManifest};
use adapter::{JudgmentModel, judgment};

use crate::registry;

/// Resolve-time `describe` metadata: no compatibility floor.
#[must_use]
pub const fn describe() -> SourceManifest {
    SourceManifest { specify_floor: None }
}

/// Session-less state note both prompts carry.
const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `screenshots`; its `path` \
                            (relative to the workspace root) is the read-only directory of \
                            screen images the prompt calls `$SOURCE_DIR`. The prompt's vision \
                            prerequisite applies: read the images themselves, never fall back \
                            to filename or metadata inference.";

/// Survey the bound screen-image set into leads, one per screen — one
/// schema-gated leg over `prompts/survey.md`, then the id-grammar
/// tail.
///
/// # Errors
///
/// As [`adapter::judgment`]; a validation-tail failure is
/// [`Error::Internal`].
pub async fn survey<P: JudgmentModel>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
    let system = registry::body("prompts/survey.md").to_string();
    let user = format!(
        "Survey the screenshots source bound to adapter `{id}`.\n\n\
         {BINDING_NOTE}\n\n\
         When `discovery.md` at the workspace root already carries leads for this source \
         under `## Lead inventory`, treat this call as a re-survey: return the complete \
         current lead set — the caller replaces prior leads by their `(source, lead)` \
         pairs, exactly as the prompt describes.\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         the same `lead` / `synopsis` / optional `topics` content as the prompt's lead \
         blocks. The caller persists the leads into `discovery.md`; do not write it \
         yourself.",
        id = ctx.adapter_id,
    );
    let answer: LeadsAnswer =
        judgment(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA).await?;
    validate_leads(&answer.leads)?;
    Ok(answer.leads)
}

/// Extract one lead's spatial Evidence from the bound screen images —
/// one schema-gated leg over `prompts/extract.md` (emitting `region` /
/// `container` / `leaf` claims), then the claim-id tail.
///
/// # Errors
///
/// As [`adapter::judgment`]; a validation-tail failure is
/// [`Error::Internal`].
pub async fn extract<P: JudgmentModel>(
    model: &P, ctx: &Context<'_>, lead: &Lead,
) -> Result<Evidence, Error> {
    let system = registry::body("prompts/extract.md").to_string();
    let user = format!(
        "Extract Evidence from the screenshots source bound to adapter `{id}` for this \
         lead (one screen, possibly with state and platform variants):\n\n{lead}\n\n\
         {BINDING_NOTE}\n\n\
         Run the prompt's spatial-inference pipeline (`prompts/extract/pipeline.md`, \
         served over this call's MCP grant) against the lead's image(s).\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority`, `claims`) the prompt describes, without the envelope `lead` key — \
         this call names the lead. The caller persists the document under \
         `.specify/slices/<slice>/evidence/`; do not write it yourself.",
        id = ctx.adapter_id,
        lead = lead.render(),
    );
    let evidence: Evidence =
        judgment(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA).await?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}
