//! Divides a documentation tree into independently mined groups.

use std::collections::BTreeMap;

use emery_sdk::{Error, Seam, SourceContent, SourceInput};

// A directory of fewer documents than this is no seam of its own.
const MIN_SIZE: usize = 2;

// A directory of more documents than this is cut one level finer, where its
// subdirectories allow it, so one directory cannot hold a whole run behind
// its one turn.
const MAX_SIZE: usize = 16;

/// Returns the groups of documentation to mine.
///
/// Inline input is one [`Seam::Whole`]. Workspace input is grouped by
/// top-level directory, leaving out hidden entries and Emery's generated
/// files:
///
/// - A directory of fewer than two files joins the files directly beneath
///   the root.
/// - A directory of more than sixteen files is cut once more, by its
///   subdirectories. Each subdirectory of two or more files is a group of its
///   own; the directory's remaining files form one more, or join the first
///   subdirectory's group when fewer than two remain. A directory with no
///   such subdirectory stays one group whatever its size.
///
/// Each group becomes a [`Seam::Files`] when at least two remain; otherwise
/// the workspace is one [`Seam::Whole`]. No model call is made.
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

impl Groups {
    fn fold(mut self) -> Self {
        // fold directories beneath the min members into the root
        let folded: Vec<String> = self
            .0
            .extract_if(.., |dir, files| dir != "." && files.len() < MIN_SIZE)
            .flat_map(|(_, files)| files)
            .collect();
        if !folded.is_empty() {
            self.0.entry(".".into()).or_default().extend(folded);
        }
        self
    }

    // One seam per group in key order, a group over the cap cut in place so a
    // directory's seams stay adjacent.
    fn into_seams(self) -> Vec<Seam> {
        let seams: Vec<_> = self
            .0
            .into_iter()
            .flat_map(|(dir, files)| split(&dir, files))
            .map(Seam::Files)
            .collect();
        if seams.len() < 2 { vec![Seam::Whole] } else { seams }
    }
}

// Cuts a directory over the cap one level finer, by subdirectories.
fn split(dir: &str, files: Vec<String>) -> Vec<Vec<String>> {
    if dir == "." || files.len() <= MAX_SIZE {
        return vec![files];
    }

    // regroup by the next path segment, the directory's own files under its key
    let prefix = format!("{dir}/");
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in files {
        let sub = file
            .strip_prefix(&prefix)
            .and_then(|rest| rest.split_once('/'))
            .map_or_else(|| dir.to_owned(), |(segment, _)| format!("{prefix}{segment}"));
        groups.entry(sub).or_default().push(file);
    }

    // fold subdirectories beneath the min members into the directory's own group
    let folded: Vec<String> = groups
        .extract_if(.., |sub, files| sub != dir && files.len() < MIN_SIZE)
        .flat_map(|(_, files)| files)
        .collect();
    if !folded.is_empty() {
        groups.entry(dir.to_owned()).or_default().extend(folded);
    }

    // a remainder too small to stand alone joins the first subdirectory's group
    if groups.len() > 1 && groups.get(dir).is_some_and(|own| own.len() < MIN_SIZE) {
        let own = groups.remove(dir).unwrap_or_default();
        if let Some(first) = groups.values_mut().next() {
            first.extend(own);
        }
    }

    groups.into_values().collect()
}
