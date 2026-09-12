//! Intent sources carry the operator's free-form brief — inline
//! (`value:`) or as a one-file tree. Extract preserves the brief
//! verbatim and lifts its directives into requirement claims.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use emery_adapter::types::{Context, Evidence, SourceContent, SourceInput};
use emery_adapter::{
    Error, EvidenceTurn, Model, SourceAdapter, bad_request, evidence, server_error,
};
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
        let system = registry::body("prompts/extract.md")
            .ok_or_else(|| server_error!("`prompts/extract.md` is not embedded"))?;
        let turn = EvidenceTurn::prepared("intent", brief_note(input)?);
        evidence(model, ctx, input, system, turn).await
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
        [file] => Ok(std::fs::read_to_string(file)
            .with_context(|| format!("reading `{}`", file.display()))?),
        _ => Err(bad_request!("intent expects one file, found {}", files.len())),
    }
}

// The one-file tree encoding may nest, so the walk is recursive.
fn collect_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading `{}`", dir.display()))? {
        let entry = entry.with_context(|| format!("reading `{}`", dir.display()))?;
        let file_type =
            entry.file_type().with_context(|| format!("reading `{}`", dir.display()))?;
        if file_type.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}
