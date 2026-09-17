//! The survey of a documentation tree: one seam per top-level directory.

use std::collections::BTreeMap;

use emery_sdk::{Error, Seam, SourceContent, SourceInput};

/// Returns the seams to mine: the tree cut by directory, or the input whole.
///
/// One [`Seam::Files`] per top-level directory of at least two documents and
/// one for the rest of the tree. A tree that cuts no finer than itself, or an
/// inline value, is the bound input whole — a single call. The cut is by
/// directory alone, so the model is never asked. A dot entry — `.git`,
/// `.github`, an editor's scratch — is tooling, not documentation, and is
/// left out.
///
/// # Errors
///
/// Returns [`Error::ServerError`] when a directory cannot be read, and
/// [`Error::BadRequest`] for an entry whose name is not UTF-8.
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &input.content else {
        return Ok(vec![Seam::Whole]);
    };

    let files = emery_sdk::survey::list(root, |entry| !entry.hidden())?;
    Ok(Seams::from(files).fold().into_seams())
}

struct Seams(BTreeMap<String, Vec<String>>);

impl From<Vec<String>> for Seams {
    fn from(files: Vec<String>) -> Self {
        let mut seams: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in files {
            let dir = file.split_once('/').map_or(".", |(dir, _)| dir).to_owned();
            seams.entry(dir).or_default().push(file);
        }
        Self(seams)
    }
}

impl From<Seams> for Vec<Seam> {
    fn from(seams: Seams) -> Self {
        seams.into_seams()
    }
}

// Documents a top-level directory holds before it is a seam of its own: a
// model call costs an agent start, so a directory of one is folded in.
const SEAM_SIZE: usize = 2;

impl Seams {
    fn fold(mut self) -> Self {
        let folded: Vec<String> = self
            .0
            .extract_if(.., |dir, files| dir != "." && files.len() < SEAM_SIZE)
            .flat_map(|(_, files)| files)
            .collect();
        if !folded.is_empty() {
            self.0.entry(".".into()).or_default().extend(folded);
        }
        self
    }

    fn into_seams(self) -> Vec<Seam> {
        let seams: Vec<_> = self.0.into_values().map(Seam::Files).collect();
        if seams.len() < 2 { vec![Seam::Whole] } else { seams }
    }
}
