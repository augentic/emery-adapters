//! The survey of a documentation tree: one seam per top-level directory.

use std::collections::BTreeMap;

use emery_sdk::{Error, Seam, SourceContent, SourceInput};

// Documents a top-level directory holds before it is a seam of its own: a
// model call costs an agent start, so a directory of one is folded in.
const FLOOR: usize = 2;

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

    // list the tree
    let files = emery_sdk::survey::list(root, |entry| !entry.hidden())?;

    // group files by top-level directory
    let mut dir_files: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut root_files = Vec::new();

    for file in &files {
        match file.split_once('/') {
            Some((directory, _)) => {
                dir_files.entry(directory).or_default().push(file.clone());
            }
            None => root_files.push(file.clone()),
        }
    }

    // fold the dir_files beneath the floor into the remainder
    let mut groups = Vec::with_capacity(dir_files.len() + 1);
    for files in dir_files.into_values() {
        if files.len() >= FLOOR {
            groups.push(files);
        } else {
            root_files.extend(files);
        }
    }

    // the remaining seam
    if !root_files.is_empty() {
        root_files.sort();
        groups.push(root_files);
    }

    // a lone group cuts no finer than the tree
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }

    Ok(groups.into_iter().map(Seam::Files).collect())
}
