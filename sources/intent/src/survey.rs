//! Validates an operator brief and returns it as one mining seam.

use std::path::Path;

use emery_sdk::{Error, Note, Seam, SourceContent, SourceInput, bad_request, server_error};

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

fn to_note(root: &str) -> Result<Seam, Error> {
    let files = emery_sdk::workspace::list(root, |entry| !entry.hidden())?;
    let [file] = files.as_slice() else {
        return Err(bad_request!("intent expects one file, found {}", files.len()));
    };

    let path = Path::new(root).join(file);
    let brief = std::fs::read_to_string(&path)
        .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
    if brief.trim().is_empty() {
        return Err(bad_request!("intent brief is empty"));
    }

    // the brief rides the text itself, so no module is laid out a second time
    Ok(Seam::Note(Note::from(format!(
        "The operator's brief, `{file}` under `$SOURCE_DIR`, the one file of the bound \
         tree:\n\n{brief}\n\n\
         Nothing else is reachable; extract mines only this source."
    ))))
}
