//! Divides a documentation tree into independently mined groups.

use std::collections::BTreeMap;

use emery_sdk::{Error, Seam, SourceContent, SourceInput};

/// Returns the groups of documentation to mine.
///
/// Workspace files are grouped by their top-level directory. A directory
/// containing fewer than two files is folded into the root group. When at
/// least two groups remain, each becomes a [`Seam::Files`]; otherwise the
/// input becomes one [`Seam::Whole`].
///
/// Inline input is always returned as one whole seam. No model call is
/// required. Hidden entries and Emery output do not influence workspace
/// grouping, but remain visible when the workspace is mined whole.
///
/// # Errors
///
/// - Returns [`Error::BadRequest`] when an entry name is not UTF-8.
/// - Returns [`Error::ServerError`] when a directory cannot be read.
pub fn survey(input: &SourceInput) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &input.content else {
        return Ok(vec![Seam::Whole]);
    };

    let files = emery_sdk::workspace::list(root, |entry| !entry.hidden())?;
    Ok(Groups::from(files).fold().into())
}

struct Groups(BTreeMap<String, Vec<String>>);

impl From<Vec<String>> for Groups {
    fn from(files: Vec<String>) -> Self {
        // group files by top-level directory.
        let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for file in files {
            let dir = file.split_once('/').map_or(".", |(dir, _)| dir).to_owned();
            groups.entry(dir).or_default().push(file);
        }
        Self(groups)
    }
}

impl From<Groups> for Vec<Seam> {
    fn from(seams: Groups) -> Self {
        seams.into_seams()
    }
}

const MIN_MEMBERS: usize = 2;

impl Groups {
    fn fold(mut self) -> Self {
        // fold directories beneath the min members into the root
        let folded: Vec<String> = self
            .0
            .extract_if(.., |dir, files| dir != "." && files.len() < MIN_MEMBERS)
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
