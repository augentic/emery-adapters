//! The survey of a documentation tree: one seam per top-level directory.

use std::path::Path;

use emery_sdk::{Context, Error, Seam, SourceContent};

// Documents a directory holds before it is mined on its own; a smaller one
// folds into the root's seam. A model call costs an agent start, so a
// directory of one document is not worth one.
const FLOOR: usize = 2;

/// Returns the seams to mine: the tree cut by directory, or the input whole.
///
/// One [`Seam::Files`] per top-level directory of at least two documents and
/// one for the rest of the tree, each lent its own directory. A tree that
/// cuts no finer than itself, or an inline value, is the bound input whole —
/// a single call. The cut is by directory alone, so the model is never asked.
/// A dot entry — `.git`, `.github`, an editor's scratch — is tooling, not
/// documentation, and is left out.
///
/// # Errors
///
/// Returns [`Error::ServerError`] when a directory cannot be read, and
/// [`Error::BadRequest`] for an entry whose name is not UTF-8.
pub fn survey(ctx: &Context<'_>) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &ctx.input.content else {
        return Ok(vec![Seam::Whole]);
    };

    let files = emery_sdk::survey::files(Path::new(root), |path, _| !hidden(path))?;
    let groups = emery_sdk::survey::by_directory(files, FLOOR);
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }
    
    Ok(groups.into_iter().map(Seam::Files).collect())
}

fn hidden(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with('.'))
}
