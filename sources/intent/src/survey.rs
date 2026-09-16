//! The survey of an operator's brief: one seam, the brief verbatim.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use emery_sdk::{Context, Error, Seam, SourceContent, bad_request};

/// Returns the one seam to mine: the brief, whichever arm carries it, never split.
///
/// An inline value rides as the SDK renders it, a [`Seam::Whole`]. A one-file
/// tree, nested or not, is read into a [`Seam::Note`] of the same shape, so
/// the brief is in the turn without a tool round; the SDK lends the tree as
/// for any workspace input. The model is never asked.
///
/// # Errors
///
/// - [`Error::BadRequest`] for an empty brief — an intent source is never
///   legitimately empty, so it fails closed before a model call is spent —
///   and for a tree holding no file or several.
/// - [`Error::ServerError`] when the tree cannot be read.
pub fn survey(ctx: &Context<'_>) -> Result<Vec<Seam>, Error> {
    let seam = match &ctx.input.content {
        SourceContent::Value(value) => {
            require_brief(value)?;
            Seam::Whole
        }
        SourceContent::Workspace(root) => {
            let intent = single_file_intent(Path::new(root))?;
            require_brief(&intent)?;
            Seam::Note(format!(
                "The bound seam is a one-file tree at `{root}`; the operator's intent string \
                 is:\n\n{intent}\n\n\
                 Nothing else is reachable; extract mines only this source."
            ))
        }
    };
    Ok(vec![seam])
}

fn require_brief(brief: &str) -> Result<(), Error> {
    if brief.trim().is_empty() {
        return Err(bad_request!("intent brief is empty"));
    }
    Ok(())
}

fn single_file_intent(root: &Path) -> Result<String, Error> {
    let files = collect_files(root)?;
    match files.as_slice() {
        [file] => Ok(std::fs::read_to_string(file)
            .with_context(|| format!("reading `{}`", file.display()))?),
        _ => Err(bad_request!("intent expects one file, found {}", files.len())),
    }
}

// The one-file tree encoding may nest, so the walk is recursive.
fn collect_files(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let reading = || format!("reading `{}`", dir.display());
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(reading)? {
        let entry = entry.with_context(reading)?;
        let file_type = entry.file_type().with_context(reading)?;
        if file_type.is_dir() {
            files.extend(collect_files(&entry.path())?);
        } else if file_type.is_file() {
            files.push(entry.path());
        }
    }
    Ok(files)
}
