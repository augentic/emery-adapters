//! Intent bindings carry the operator's free-form brief inline, so both
//! legs echo rather than infer.

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceMetadata};
use adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

const BINDING_NOTE: &str = "The operator's project workspace is lent to you, and there is no \
                            session: every input you need lives in the workspace tree and this \
                            prompt. Resolve the bound source material from the plan — read \
                            `plan.yaml` at the workspace root and find the binding under \
                            `sources.<key>` whose `adapter` is `intent`; its inline `value` \
                            carries the operator's free-form intent string, verbatim (`path` \
                            is absent for intent bindings — no source tree is bound).";

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
            emery_floor: Some("0.36.0".to_string()),
        }
    }

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Lead>, Error> {
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
        repaired(model, ctx, system, user, "leads", LEADS_ANSWER_SCHEMA, leads_tail).await
    }

    async fn extract<P: Model>(
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
         The caller persists the document under `.emery/slices/<slice>/evidence/`; do \
         not write it yourself.",
            id = ctx.adapter_id,
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
