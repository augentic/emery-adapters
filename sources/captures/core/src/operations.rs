//! The judgment operations: `survey` and `extract` — schema-gated legs
//! through [`specify_guest_kit::judgment`] with deterministic id-grammar
//! tails.
//!
//! The judgment detail — the capture-tree layout, the
//! `kind: example` claim shape with `replay-digest` anchors, the 64 KiB
//! inline cap — rides in the embedded prompts and references.

use specify_guest_kit::answers::{
    EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, LeadsAnswer, validate_evidence, validate_leads,
};
use specify_guest_kit::seam::{Context, Error, Evidence, Lead, SourceManifest};
use specify_guest_kit::{Model, judgment};

use crate::registry;

/// The adapter's deterministic self-description (RFC-64).
///
/// Resolve-time metadata answered from compiled-in constants: no
/// compatibility floor is declared, matching the retired manifest.
#[must_use]
pub const fn describe() -> SourceManifest {
    SourceManifest { specify_floor: None }
}

/// How the spawned agent resolves its bound source material — the
/// session-less state note both prompts carry.
const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `captures`; its `path` \
                            (relative to the workspace root) is the read-only runtime capture \
                            tree the prompt calls `$SOURCE_DIR` (the \
                            `tests/data/replays/<handler>/` layout `/capture:wiretapper` \
                            writes).";

/// Survey the bound capture tree into leads (one per captured handler) —
/// one schema-gated judgment leg over the embedded `prompts/survey.md`,
/// followed by the deterministic id-grammar tail.
///
/// # Errors
///
/// As [`specify_guest_kit::judgment`]; a validation-tail failure is
/// [`Error::Internal`].
pub async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
    let system = registry::body("prompts/survey.md").to_string();
    let user = format!(
        "Survey the runtime-capture source bound to adapter `{id}`.\n\n\
         {BINDING_NOTE}\n\n\
         When `discovery.md` at the workspace root already carries leads for this source \
         under `## Lead inventory`, treat this call as a re-survey: return the complete \
         current lead set — the caller replaces prior leads by their `(source, lead)` \
         pairs (the prompt sorts blocks by `lead` for byte-stable re-survey diffs).\n\n\
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

/// Extract one lead's behavioural Evidence from the bound capture tree.
///
/// One schema-gated judgment leg over the embedded `prompts/extract.md`
/// (emitting `kind: example` claims with `replay-digest` anchors),
/// followed by the deterministic claim-id tail.
///
/// # Errors
///
/// As [`specify_guest_kit::judgment`]; a validation-tail failure is
/// [`Error::Internal`].
pub async fn extract<P: Model>(
    model: &P, ctx: &Context<'_>, lead: &Lead,
) -> Result<Evidence, Error> {
    let system = registry::body("prompts/extract.md").to_string();
    let user = format!(
        "Extract Evidence from the runtime-capture source bound to adapter `{id}` for \
         this lead (one captured handler):\n\n{lead}\n\n\
         {BINDING_NOTE}\n\n\
         The prompt's references (`capture-format.md`, `extraction-mapping.md`) are \
         served over this call's MCP grant — load both, as the prompt requires.\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority: \"behaviour\"`, `kind: \"example\"` claims carrying the \
         `replay-digest` / `input` / `output` body fields the prompt describes), without \
         the envelope `lead` key — this call names the lead. The caller persists the \
         document under `.specify/slices/<slice>/evidence/`; do not write it yourself.",
        id = ctx.adapter_id,
        lead = lead.render(),
    );
    let evidence: Evidence =
        judgment(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA).await?;
    validate_evidence(&evidence)?;
    Ok(evidence)
}
