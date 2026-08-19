//! Intent bindings carry the operator's free-form brief — inline
//! (`value:` bindings) or as a single-file locator the engine
//! materialized as a one-file tree. Extract preserves the brief
//! verbatim and lifts its directives into requirement claims.

use std::path::{Path, PathBuf};

use emery_adapter::answers::{EVIDENCE_ANSWER_SCHEMA, evidence_tail};
use emery_adapter::registry::Doc;
use emery_adapter::seam::{Context, Error, Evidence, SourceContent, SourceInput, SourceMetadata};
use emery_adapter::{AdapterIdentity, Model, Source, repaired};

use crate::registry;

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

/// Intent binding → one Evidence document with one `kind: intent` claim.
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

    async fn extract<P: Model>(
        model: &P, ctx: &Context<'_>, input: &SourceInput,
    ) -> Result<Evidence, Error> {
        let system = registry::body("prompts/extract.md").to_string();
        let user = format!(
            "Extract the claim set of the intent source bound to adapter `{id}` \
             (source key `{key}`).\n\n\
             {content}\n\n\
             Answer with one JSON object matching the gated schema: the Evidence body \
             (`authority: \"intent\"`; first one `kind: \"intent\"` claim whose `id` \
             equals the source key and whose `statement` carries the operator's brief \
             verbatim, then one `kind: \"requirement\"` claim per distinct behavioural \
             directive the brief states, per the prompt). The caller persists the \
             document; do not write it yourself.",
            id = ctx.adapter_id,
            key = input.key,
            content = content_note(input)?,
        );
        repaired(model, ctx, system, user, "evidence", EVIDENCE_ANSWER_SCHEMA, evidence_tail).await
    }
}

fn content_note(input: &SourceInput) -> Result<String, Error> {
    match &input.content {
        SourceContent::Value(value) => {
            require_brief(value)?;
            Ok(format!(
                "The bound material is this inline value; no `$SOURCE_DIR` is lent:\n\n{value}\n\n\
                 Nothing else is reachable; extract works only from this value."
            ))
        }
        SourceContent::Workspace(view) => {
            let intent = single_file_intent(Path::new(&view.root))?;
            require_brief(&intent)?;
            Ok(format!(
                "The bound material is a one-file tree at `{}`; the operator's intent \
                 string is:\n\n{intent}\n\n\
                 Nothing else is reachable; extract works only from this value.",
                view.root
            ))
        }
    }
}

/// An intent binding is never legitimately empty: fail closed before
/// spending a model call, never answer an empty success.
fn require_brief(brief: &str) -> Result<(), Error> {
    if brief.trim().is_empty() {
        return Err(Error::InvalidRequest(
            "the bound intent brief is empty; intent extract fails closed".to_string(),
        ));
    }
    Ok(())
}
