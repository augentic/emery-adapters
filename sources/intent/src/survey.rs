//! Validates an operator brief and returns it as one mining seam.

use std::path::Path;

use anyhow::Context as _;
use emery_sdk::{Error, Seam, SourceContent, SourceInput, bad_request};

// An inline brief is one `Whole` seam; the one file of a tree is read into a
// `Note` seam. No model turn is spent.
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let seam = match &input.content {
        SourceContent::Value(value) => {
            if value.trim().is_empty() {
                return Err(bad_request!("intent brief is empty"));
            }
            Seam::Whole
        }
        SourceContent::Workspace(root) => to_note(root)?,
    };

    Ok(vec![seam])
}

// The one file of the tree, read into the seam that carries it.
fn to_note(root: &str) -> Result<Seam, Error> {
    let files = emery_sdk::workspace::list(root, |entry| !entry.hidden())?;
    let [file] = files.as_slice() else {
        return Err(bad_request!("intent expects one file, found {}", files.len()));
    };

    let path = Path::new(root).join(file);
    let brief =
        std::fs::read_to_string(&path).with_context(|| format!("reading `{}`", path.display()))?;
    if brief.trim().is_empty() {
        return Err(bad_request!("intent brief is empty"));
    }

    Ok(Seam::Note(format!(
        "The operator's brief, `{file}` under `$SOURCE_DIR`, the one file of the bound \
         tree:\n\n{brief}\n\n\
         Nothing else is reachable; extract mines only this source."
    )))
}
