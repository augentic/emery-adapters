//! The documentation adapter and its survey by directory.
//!
//! A documentation source is a written tree — specifications, guides,
//! decision records. The survey cuts it one top-level directory at a time,
//! each mined under a lend no wider than itself, and the SDK joins the claims
//! into one document.

use std::future::{Future, ready};
use std::path::Path;

use emery_sdk::{
    Context, Doc, Error, Model, Seam, SourceAdapter, SourceContent, SourceKind, survey,
};

use crate::registry;

/// The adapter over a tree of written documentation.
#[derive(Debug)]
pub struct Adapter;

// Documents a directory holds before it is mined on its own; a smaller one
// folds into the root's seam. A model call costs an agent start, so a
// directory of one document is not worth one.
const FLOOR: usize = 2;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    // One `Files` seam per top-level directory of at least `FLOOR` documents
    // and one for the rest of the tree, each lent its own directory. A tree
    // that cuts no finer than itself, or an inline value, is the bound input
    // whole — a single call, as before. The cut is by directory alone, so
    // the model is never asked.
    fn survey<P: Model>(
        _model: &P, ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Vec<Seam>, Error>> + Send {
        ready(cut(&ctx.input.content))
    }
}

// The seams of one input: the tree cut by directory, or the input whole.
fn cut(content: &SourceContent) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = content else {
        return Ok(vec![Seam::Whole]);
    };

    let files = survey::files(Path::new(root), |path, _| !hidden(path))?;
    let groups = survey::by_directory(files, FLOOR);
    if groups.len() < 2 {
        return Ok(vec![Seam::Whole]);
    }
    Ok(groups.into_iter().map(Seam::Files).collect())
}

// A dot entry — `.git`, `.github`, an editor's scratch — is tooling, not
// documentation.
fn hidden(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with('.'))
}
