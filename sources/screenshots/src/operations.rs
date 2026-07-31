use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceMetadata};
use adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `screenshots`; its `path` \
                            (relative to the workspace root) is the read-only directory of \
                            screen images the prompt calls `$SOURCE_DIR`. The prompt's vision \
                            prerequisite applies: read the images themselves, never fall back \
                            to filename or metadata inference.";

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
            emery_floor: Some("0.34.0".to_string()),
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
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
        repaired(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA, leads_tail).await
    }

    async fn extract<P: Model>(
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
         `.emery/slices/<slice>/evidence/`; do not write it yourself.",
            id = ctx.adapter_id,
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
