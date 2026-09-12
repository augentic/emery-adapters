//! Intent sources carry the operator's free-form brief — inline
//! (`value:`) or as a one-file tree. Extract preserves the brief
//! verbatim and lifts its directives into requirement claims.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use emery_adapter::{
    Context, Error, Evidence, Material, Model, SourceAdapter, SourceContent, bad_request,
};
use emery_prose::registry::Doc;

use crate::registry;

/// Intent source → one Evidence document with one `kind: intent` claim.
#[derive(Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
    const SOURCE: &'static str = "intent";

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    async fn extract<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Evidence, Error> {
        Self::evidence(model, ctx, brief(&ctx.input.content)?).await
    }
}

// The operator's brief as the turn's material: an inline value rides as the
// SDK renders it; a one-file tree is read into a note of the same shape.
fn brief(content: &SourceContent) -> Result<Material, Error> {
    match content {
        SourceContent::Value(value) => {
            require_brief(value)?;
            Ok(Material::Bound)
        }
        SourceContent::Workspace(root) => {
            let intent = single_file_intent(Path::new(root))?;
            require_brief(&intent)?;
            Ok(Material::Prepared(format!(
                "The bound material is a one-file tree at `{root}`; the operator's intent \
                 string is:\n\n{intent}\n\n\
                 Nothing else is reachable; extract mines only this source."
            )))
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
