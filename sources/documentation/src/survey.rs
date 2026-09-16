//! The survey of a documentation tree: one seam per top-level directory.

use emery_sdk::survey::Tree;
use emery_sdk::{Context, Error, Seam, SourceContent};

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

    let groups = Tree::list(root, |entry| !entry.hidden())?.by_directory(2);
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }

    Ok(groups.into_iter().map(Seam::Files).collect())
}
