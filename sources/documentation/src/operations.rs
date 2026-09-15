//! Documentation sources are written trees — specifications, guides,
//! decision records. Extract mines them one top-level directory at a time,
//! each directory lent on its own, and joins the claims into one document.

use std::path::Path;

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Material, SourceAdapter, SourceContent, SourceKind, survey};

use crate::registry;

/// Written specifications / documentation trees → one Evidence document.
#[derive(Debug)]
pub struct Adapter;

// Documents a directory holds before it is mined on its own; a smaller one
// folds into the root's material. A model call costs an agent start, so a
// directory of one document is not worth one.
const FLOOR: usize = 2;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;
    const SOURCE: &'static str = "documentation";

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    // One `Within` per top-level directory of at least `FLOOR` documents and
    // one for the rest of the tree, each lent its own directory. A tree that
    // cuts no finer than itself, or an inline value, is the bound input
    // whole — a single call, as before.
    fn survey(ctx: &Context<'_>) -> Result<Vec<Material>, Error> {
        let SourceContent::Workspace(root) = &ctx.input.content else {
            return Ok(vec![Material::Bound]);
        };

        let files = survey::files(Path::new(root), |path, _| !hidden(path))?;
        let groups = survey::by_directory(files, FLOOR);
        if groups.len() < 2 {
            return Ok(vec![Material::Bound]);
        }
        Ok(groups.into_iter().map(Material::Within).collect())
    }
}

// A dot entry — `.git`, `.github`, an editor's scratch — is tooling, not
// documentation.
fn hidden(path: &Path) -> bool {
    path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with('.'))
}
