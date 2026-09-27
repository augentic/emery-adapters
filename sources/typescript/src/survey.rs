//! Discovers the caller-facing surfaces exposed by a source tree.
//!
//! Each discovered surface becomes an independent mining seam. Inline input,
//! and a tree of one production module, require no discovery and are one
//! whole seam.

use emery_sdk::survey::Surface;
use emery_sdk::workspace::Entry;
use emery_sdk::{Context, Doc, Error, Model, Seam, SourceContent, bad_request};

// Surveys workspace input by model: each surface becomes a note seam lent the
// whole tree, so a module is mined only through the surfaces that reach it.
// An inline value, or a tree of one production module, cannot be cut and is
// one whole seam with no survey turn spent.
pub async fn survey<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    let source = &ctx.input.name;

    // inline brief instead of a path — nothing to list or survey
    let SourceContent::Workspace(workspace) = &ctx.input.content else {
        tracing::debug!(%source, "inline value; one whole seam");
        return Ok(vec![Seam::Whole]);
    };

    // count modules discovered
    match emery_sdk::workspace::list(workspace, include)?.len() {
        1 => {
            tracing::debug!(%source, "one module found");
            return Ok(vec![Seam::Whole]);
        }
        0 => return Err(bad_request!("`{source}` has no public modules.")),
        _ => {}
    }

    // survey the workspace
    tracing::info!(%source, %workspace, "identifying typescript surfaces");
    let surfaces = emery_sdk::survey::surfaces(ctx, docs, include).await?;
    if surfaces.is_empty() {
        return Err(bad_request!(
            "`{source}` exposes no surfaces — no routes, commands, jobs, or APIs."
        ));
    }

    // create note seams for each surface
    let seams: Vec<_> = surfaces.iter().map(|surface| Seam::Note(mine_note(surface))).collect();
    tracing::debug!(%source, seams = seams.len(), "note seams");

    Ok(seams)
}

const SKIP_DIRS: &[&str] =
    &["node_modules", "vendor", "target", "dist", "build", "tests", "__tests__"];
const SKIP_INFIXES: &[&str] = &["config", "d", "spec", "test"];
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

    // skip non-code infixes
    !matches!(stem.rsplit_once('.'), Some((_, infix)) if SKIP_INFIXES.contains(&infix))
}

// What to mine.
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
