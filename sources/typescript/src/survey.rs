//! Discovers the caller-facing surfaces exposed by a source tree.
//!
//! Each discovered surface becomes an independent mining seam. Inline input,
//! and a tree of one production module, require no discovery and are one
//! whole seam.

use emery_sdk::survey::Surface;
use emery_sdk::workspace::Entry;
use emery_sdk::{Context, Doc, Error, Model, Seam, SourceContent, bad_request};

// Each surface is a note seam lent the whole tree, so a module is mined only
// through the surfaces that reach it.
pub async fn survey<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    let source = &ctx.input.name;

    // pass an inline value through whole
    let SourceContent::Workspace(workspace) = &ctx.input.content else {
        return Ok(vec![Seam::Whole]);
    };

    // count modules discovered
    match emery_sdk::workspace::list(workspace, include)?.len() {
        1 => return Ok(vec![Seam::Whole]),
        0 => return Err(bad_request!("`{source}` has no public modules.")),
        _ => {}
    }

    // survey the workspace
    let surfaces = emery_sdk::survey::surfaces(ctx, docs, include).await?;
    if surfaces.is_empty() {
        return Err(bad_request!(
            "`{source}` exposes no surfaces — no routes, commands, jobs, or APIs."
        ));
    }

    Ok(surfaces.iter().map(|surface| Seam::Note(mine_note(surface))).collect())
}

const SKIP_DIRS: &[&str] =
    &["node_modules", "vendor", "target", "dist", "build", "test", "tests", "__tests__"];
const SKIP_INFIXES: &[&str] = &["d", "spec", "test"];
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

fn include(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !SKIP_DIRS.contains(&entry.name());
    }
    let Some((stem, extension)) = entry.name().rsplit_once('.') else {
        return false;
    };
    if !EXTENSIONS.contains(&extension) {
        return false;
    }

    !matches!(stem.rsplit_once('.'), Some((_, infix)) if SKIP_INFIXES.contains(&infix))
}

fn mine_note(surface: &Surface) -> String {
    format!(
        "Surface `{name}` — entry `{entry}`.\n\n\
         This call mines that one surface alone. Start at its entry and follow what the surface \
         reaches through the whole TypeScript / JavaScript tree under `$SOURCE_DIR` — its \
         handler, the modules it imports, the services and stores it calls, the types it takes \
         and returns — and emit claims for the behaviour a caller observes through this surface \
         alone. What the tree does for another surface is that surface's call to claim, even in \
         a module the two share. Anchor every `path` relative to `$SOURCE_DIR`; extract mines \
         only this source.",
        name = surface.name,
        entry = surface.entry,
    )
}
