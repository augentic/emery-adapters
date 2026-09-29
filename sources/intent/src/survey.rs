//! Validates an operator brief and returns it as one mining seam.

use std::path::Path;

use emery_sdk::{Error, Seam, SourceContent, SourceInput, bad_request, server_error};

pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let seam = match &input.content {
        SourceContent::Value(value) => {
            if value.trim().is_empty() {
                return Err(bad_request!("intent brief is empty"));
            }
            Seam::whole()
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

    // the SDK lays the one file into the turn and holds every anchor to it
    Ok(Seam {
        text: format!("The operator's brief is `{file}`, the one file of the bound tree."),
        ..Seam::files([file.as_str()])
    })
}
