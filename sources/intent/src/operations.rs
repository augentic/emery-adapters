//! Intent bindings carry the operator's free-form brief — inline
//! (delivery `value:` bindings) or as a single-file locator the engine
//! materialized as a one-file tree — so both legs echo rather than infer.

use std::path::{Path, PathBuf};

use adapter::answers::{EVIDENCE_ANSWER_SCHEMA, LEADS_ANSWER_SCHEMA, evidence_tail, leads_tail};
use adapter::registry::Doc;
use adapter::seam::{Context, Error, Evidence, Lead, SourceInput, SourceMetadata};
use adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

/// The binding key and intent string from the engine-prepared call. An
/// inline input carries the intent verbatim; a tree input is a file
/// locator — the prepared workspace must hold exactly one regular file,
/// whose contents are the intent.
fn binding<'a>(ctx: &'a Context<'_>, input: &SourceInput) -> Result<(&'a str, String), Error> {
    let key = ctx.source_key.as_deref().ok_or_else(|| {
        Error::InvalidRequest("source dispatch carries no source-binding key".to_string())
    })?;
    let intent = match input {
        SourceInput::Inline(content) => content.clone(),
        SourceInput::Workspace(root) => single_file_intent(Path::new(root))?,
    };
    Ok((key, intent))
}

/// Read the one regular file in the prepared tree as the intent string.
fn single_file_intent(root: &Path) -> Result<String, Error> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    match files.as_slice() {
        [file] => std::fs::read_to_string(file).map_err(|err| Error::Io(err.to_string())),
        _ => Err(Error::InvalidRequest(format!(
            "intent reads an inline `value:` binding or a single-file location; the \
             prepared tree holds {} files",
            files.len()
        ))),
    }
}

/// Collect every regular file beneath `dir` (the one-file tree encoding
/// may nest).
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir).map_err(|err| Error::Io(err.to_string()))? {
        let entry = entry.map_err(|err| Error::Io(err.to_string()))?;
        let file_type = entry.file_type().map_err(|err| Error::Io(err.to_string()))?;
        if file_type.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
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
         Source binding key: `{key}`. The engine resolved the binding to the \
         operator's free-form intent string and passed it verbatim:\n\n\
         {intent}\n\n\
         The lead id is a stable kebab-case slug derived from the intent string itself, \
         per the prompt's slug rules. Re-running this survey replaces the prior lead by \
         its `(source, lead)` pair, exactly as the prompt describes — emit the single \
         current lead.\n\n\
         Answer with one JSON object matching the gated schema: a `leads` array carrying \
         exactly one lead whose `synopsis` is the operator's intent string, verbatim, per \
         the prompt. The engine persists the lead; do not write it yourself.",
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
         Source binding key: `{key}`. The engine resolved the binding to the \
         operator's intent string and passed it verbatim:\n\n\
         {intent}\n\n\
         Answer with one JSON object matching the gated schema: the Evidence body \
         (`authority: \"intent\"`, one `kind: \"intent\"` claim whose `id` equals the \
         lead id and whose `statement` carries the operator's intent string verbatim, \
         per the prompt), without the envelope `lead` key — this call names the lead. \
         The engine persists the document; do not write it yourself.",
            id = ctx.adapter_id,
            lead = lead.render(),
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}
