use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceMetadata};
use adapter::{Model, Source, repaired};

use crate::registry;

const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `documentation`; its `path` \
                            (relative to the workspace root) is the read-only documentation \
                            tree the prompt calls `$SOURCE_DIR`.";

/// Written specifications / documentation trees → leads and Evidence.
#[derive(Clone, Copy, Debug)]
pub struct Documentation;

impl Source for Documentation {
    const NAME: &'static str = "documentation";

    fn metadata() -> SourceMetadata {
        SourceMetadata { specify_floor: None }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
        let system = registry::body("prompts/survey.md").to_string();
        let user = format!(
            "Survey the documentation source bound to adapter `{id}`.\n\n\
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
            "Extract Evidence from the documentation source bound to adapter `{id}` for this \
         lead:\n\n{lead}\n\n\
         {BINDING_NOTE}\n\n\
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
