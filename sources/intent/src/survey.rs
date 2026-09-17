//! Validates an operator brief and returns it as one mining seam.

use std::path::Path;

use anyhow::Context as _;
use emery_sdk::{Error, Seam, SourceContent, SourceInput, bad_request};

/// Returns the validated brief as a single seam.
///
/// Inline input becomes [`Seam::Whole`]. Workspace input must contain exactly
/// one regular file after Emery's generated files are excluded; that file is
/// read into a [`Seam::Note`]. In either form, the brief must contain
/// non-whitespace text. No model call is required.
///
/// # Errors
///
/// - Returns [`Error::BadRequest`] when the brief is empty, the workspace
///   does not contain exactly one source file, or an entry name is not UTF-8.
/// - Returns [`Error::ServerError`] when the workspace or brief file cannot
///   be read.
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let seam = match &input.content {
        SourceContent::Value(value) => {
            if value.trim().is_empty() {
                return Err(bad_request!("intent brief is empty"));
            }
            Seam::Whole
        }
        // intent is in a file
        SourceContent::Workspace(root) => {
            let files = emery_sdk::workspace::list(root, |_| true)?;
            let [file] = files.as_slice() else {
                return Err(bad_request!("intent expects one file, found {}", files.len()));
            };
            let path = Path::new(root).join(file);
            let intent = std::fs::read_to_string(&path)
                .with_context(|| format!("reading `{}`", path.display()))?;
            if intent.trim().is_empty() {
                return Err(bad_request!("intent brief is empty"));
            }
            Seam::Note(format!(
                "The bound seam is a one-file tree at `{root}`; the operator's intent string \
                 is:\n\n{intent}\n\n\
                 Nothing else is reachable; extract mines only this source."
            ))
        }
    };

    Ok(vec![seam])
}
