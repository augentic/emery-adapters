//! The survey of an operator's brief: one seam, the brief verbatim.

use std::path::Path;

use anyhow::Context as _;
use emery_sdk::{Error, Seam, SourceContent, SourceInput, bad_request};

/// Returns the one seam to mine: the brief, whichever arm carries it, never split.
///
/// An inline value rides as the SDK renders it, a [`Seam::Whole`]. A one-file
/// tree, nested or not, is read into a [`Seam::Note`] of the same shape, so
/// the brief is in the turn without a tool round; the SDK lends the tree as
/// for any workspace input. The engine's own files beside the brief —
/// `spec.md`, `design.md`, `.omnia/` — are not files of the tree. The model
/// is never asked.
///
/// # Errors
///
/// - [`Error::BadRequest`] for an empty brief — an intent source is never
///   legitimately empty, so it fails closed before a model call is spent —
///   and for a tree holding no file or several.
/// - [`Error::ServerError`] when the tree cannot be read.
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let seam = match &input.content {
        SourceContent::Value(value) => {
            require_brief(value)?;
            Seam::Whole
        }
        SourceContent::Workspace(root) => {
            let intent = single_file_intent(root)?;
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

fn single_file_intent(root: &str) -> Result<String, Error> {
    let files = emery_sdk::workspace::list(root, |_| true)?;
    match files.as_slice() {
        [file] => {
            let path = Path::new(root).join(file);
            Ok(std::fs::read_to_string(&path)
                .with_context(|| format!("reading `{}`", path.display()))?)
        }
        _ => Err(bad_request!("intent expects one file, found {}", files.len())),
    }
}
