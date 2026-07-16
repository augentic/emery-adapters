//! The [`Typescript`] adapter: `survey` and `extract` — schema-gated
//! legs through [`adapter::repaired`], with the id-grammar answer
//! tails repaired inside its bounded loop.
//!
//! The judgment detail — the framework survey grammar, the
//! `excerpt` / `type` / `call` extraction depth — rides in the
//! embedded prompts and references.

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceMetadata};
use adapter::{Model, Source, repaired};

use crate::registry;

/// Session-less state note both prompts carry.
const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `typescript`; its `path` \
                            (relative to the workspace root) is the read-only TypeScript / \
                            JavaScript source tree the prompt calls `$SOURCE_DIR`. Treat that \
                            tree as read-only.";

/// The typescript source adapter: TypeScript / JavaScript source
/// trees surveyed into leads and extracted into code Evidence.
#[derive(Clone, Copy, Debug)]
pub struct Typescript;

impl Source for Typescript {
    const NAME: &'static str = "typescript";

    /// Resolve-time `metadata`: no compatibility floor.
    fn metadata() -> SourceMetadata {
        SourceMetadata { specify_floor: None }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    /// Survey the bound source tree into leads.
    ///
    /// One schema-gated leg over `prompts/survey.md`, with the id-grammar
    /// tail repaired inside the bounded loop.
    ///
    /// # Errors
    ///
    /// As [`adapter::repaired`]; a tail failure that survives the
    /// repair budget is [`Error::Internal`].
    async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
        let system = registry::body("prompts/survey.md").to_string();
        let user = format!(
            "Survey the TypeScript / JavaScript source bound to adapter `{id}` using the \
         prompt's framework grammar.\n\n\
         {BINDING_NOTE}\n\n\
         When `discovery.md` at the workspace root already carries leads for this source \
         under `## Lead inventory`, treat this call as a re-survey: return the complete \
         current lead set — the caller replaces prior leads by their `(source, lead)` \
         pairs (the prompt's stable re-survey handle).\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         the same `lead` / `synopsis` / optional `topics` content as the prompt's lead \
         blocks. The caller persists the leads into `discovery.md`; do not write it \
         yourself.",
            id = ctx.adapter_id,
        );
        repaired(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA, leads_tail).await
    }

    /// Extract one lead's behavioural Evidence from the bound source tree.
    ///
    /// One schema-gated leg over `prompts/extract.md` (emitting
    /// `excerpt` / `type` / `call` claims), with the claim-id tail
    /// repaired inside the bounded loop.
    ///
    /// # Errors
    ///
    /// As [`adapter::repaired`]; a tail failure that survives the
    /// repair budget is [`Error::Internal`].
    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, lead: &Lead,
    ) -> Result<Evidence, Error> {
        let system = registry::body("prompts/extract.md").to_string();
        let user = format!(
            "Extract Evidence from the TypeScript / JavaScript source bound to adapter \
         `{id}` for this lead:\n\n{lead}\n\n\
         {BINDING_NOTE}\n\n\
         The prompt's references is served over this call's MCP grant — load the \
         reference bodies on demand when the lead's surface needs deeper analysis.\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority`, `claims`) the prompt describes, without the envelope `lead` key — \
         this call names the lead. The caller persists the document under \
         `.specify/slices/<slice>/evidence/`; do not write it yourself.",
            id = ctx.adapter_id,
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
