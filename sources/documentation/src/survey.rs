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
/// Workspace files are grouped by their top-level directory. A directory
/// containing fewer than two files is folded into the root group. A directory
/// containing more than sixteen files is cut once more, by its
/// subdirectories: each subdirectory of two or more files becomes a group of
/// its own, and the directory's remaining files — those directly beneath it
/// and those of any smaller subdirectory — one more, or join the first
/// subdirectory's group when fewer than two remain; a directory with no
/// subdirectory of two or more files stays one group whatever its size. When
/// at least two groups remain, each becomes a [`Seam::Files`]; otherwise the
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

// Cuts a directory over the cap one level finer, by its subdirectories, and
// leaves it whole where nothing cuts: the root, whose files were folded there
// from directories too small to stand alone; a directory within the cap; or
// one with no subdirectory of enough files to stand alone.
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
