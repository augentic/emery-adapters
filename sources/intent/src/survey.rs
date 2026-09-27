//! Validates an operator brief and returns it as one mining seam.

use std::path::Path;

use anyhow::Context as _;
use emery_sdk::{Error, Seam, SourceContent, SourceInput, bad_request};

// Inline input is one `Whole` seam; workspace input must hold exactly one
// file beside Emery's generated files, read into a `Note` seam. Either form
// must hold non-whitespace text, and no model call is made. Refuses
// `BadRequest` for an empty brief, a workspace of other than one file, or a
// non-UTF-8 entry name, and `ServerError` when a read fails.
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let seam = match &input.content {
        SourceContent::Value(value) => {
            if value.trim().is_empty() {
                return Err(bad_request!("intent brief is empty"));
            }
            Seam::Whole
        }
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
