//! Intent sources carry the operator's free-form brief — inline
//! (`value:`) or as a one-file tree. Extract preserves the brief
//! verbatim and lifts its directives into requirement claims.

use std::path::{Path, PathBuf};

use emery_adapter::types::{Context, Evidence, SourceContent, SourceInput};
use emery_adapter::{Error, Model, SourceAdapter, bad_request, evidence, server_error};
use emery_prose::registry::Doc;

use crate::registry;

/// Intent source → one Evidence document with one `kind: intent` claim.
#[derive(Clone, Copy, Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
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
            content = brief_note(input)?,
        );
        evidence(model, ctx, system, user).await
    }
}

// The prompt's note on the operator's brief: the SDK's content note for an
// inline value; a one-file tree is read into the same shape.
fn brief_note(input: &SourceInput) -> Result<String, Error> {
    match &input.content {
        SourceContent::Value(value) => {
            require_brief(value)?;
            Ok(emery_adapter::content_note(input, ""))
        }
        SourceContent::Workspace(root) => {
            let intent = single_file_intent(Path::new(root))?;
            require_brief(&intent)?;
            Ok(format!(
                "The bound material is a one-file tree at `{root}`; the operator's intent \
                 string is:\n\n{intent}\n\n\
                 Nothing else is reachable; extract mines only this source."
            ))
        }
    }
}

// An intent source is never legitimately empty: fail closed before
// spending a model call, never answer an empty success.
fn require_brief(brief: &str) -> Result<(), Error> {
    if brief.trim().is_empty() {
        return Err(bad_request!("intent brief is empty"));
    }
    Ok(())
}

fn single_file_intent(root: &Path) -> Result<String, Error> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    match files.as_slice() {
        [file] => std::fs::read_to_string(file)
            .map_err(|err| server_error!("reading `{}`: {err}", file.display())),
        _ => Err(bad_request!("intent expects one file, found {}", files.len())),
    }
}

// The one-file tree encoding may nest, so the walk is recursive.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    let unreadable = |err: std::io::Error| server_error!("reading `{}`: {err}", dir.display());
    for entry in std::fs::read_dir(dir).map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let file_type = entry.file_type().map_err(unreadable)?;
        if file_type.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}
