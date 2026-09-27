//! Divides a documentation tree into independently mined groups.

use std::collections::BTreeMap;

use emery_sdk::{Error, Seam, SourceContent, SourceInput};

const MIN_SIZE: usize = 2;

// Over this, a directory is cut one level finer, so it cannot hold a run behind
// one turn.
const MAX_SIZE: usize = 16;

// Groups workspace input by top-level directory, hidden entries aside. A
// directory of fewer than two files joins the root's; one of more than
// sixteen is cut once more by subdirectory, each subdirectory of two or more
// files its own group and the directory's leftovers one beside them, or
// folded into the first group when fewer than two remain. Each group becomes
// a `Files` seam, or the workspace one `Whole` seam when fewer than two
// remain; inline input is always one `Whole` seam. No model call is made.
// Refuses `BadRequest` for a non-UTF-8 entry name and `ServerError` when a
// directory cannot be read.
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

    // A group over the cap is cut in place, so a directory's seams stay adjacent.
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
