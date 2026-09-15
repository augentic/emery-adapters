//! Intent sources carry the operator's free-form brief — inline
//! (`value:`) or as a one-file tree. Extract preserves the brief
//! verbatim and lifts its directives into requirement claims.

use std::future::{Future, ready};
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use emery_prose::registry::Doc;
use emery_sdk::{
    Context, Error, Material, Model, SourceAdapter, SourceContent, SourceKind, bad_request,
};

use crate::registry;

/// Intent source → one Evidence document.
///
/// One `kind: intent` claim carries the brief verbatim, then one
/// `requirement` claim per directive it states.
#[derive(Debug)]
pub struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Intent;

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    // A brief is never split: one material, whichever arm carries it, chosen
    // without the model.
    fn survey<P: Model>(
        _model: &P, ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Vec<Material>, Error>> + Send {
        ready(brief(&ctx.input.content).map(|material| vec![material]))
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
        // The SDK lends the tree as for any workspace input; the note puts
        // the brief in the turn without a tool round.
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
    let reading = || format!("reading `{}`", dir.display());
    for entry in std::fs::read_dir(dir).with_context(reading)? {
        let entry = entry.with_context(reading)?;
        let file_type = entry.file_type().with_context(reading)?;
        if file_type.is_dir() {
            collect_files(&entry.path(), files)?;
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(())
}
