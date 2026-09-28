//! Discovers the caller-facing surfaces exposed by a source tree.
//!
//! A tree whose production modules fit within the SDK's inline budget is one
//! seam over those modules, laid into the turn whole, with no survey turn
//! spent, as is a tree of one module; an inline value is one whole seam. A
//! larger tree is surveyed by model, and each discovered surface becomes an
//! independent mining seam held to the surface's stem.

use emery_sdk::survey::Surface;
use emery_sdk::workspace::Entry;
use emery_sdk::{Context, Doc, Error, INLINE_BYTES, Model, Note, Seam, SourceContent, bad_request};

pub async fn survey<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    let source = &ctx.input.name;

    // pass an inline value through whole
    let SourceContent::Workspace(workspace) = &ctx.input.content else {
        return Ok(vec![Seam::Whole]);
    };

    // a tree small enough to lay into one turn cuts no finer
    let modules = emery_sdk::workspace::list(workspace, include)?;
    if modules.is_empty() {
        return Err(bad_request!("`{source}` has no public modules."));
    }
    if modules.len() == 1 || emery_sdk::workspace::size(workspace, &modules)? <= INLINE_BYTES {
        return Ok(vec![Seam::Files(modules)]);
    }

    // survey the workspace
    let surfaces = emery_sdk::survey::surfaces(ctx, docs, include).await?;
    if surfaces.is_empty() {
        return Err(bad_request!(
            "`{source}` exposes no surfaces — no routes, commands, jobs, or APIs."
        ));
    }

    Ok(surfaces.iter().map(|surface| Seam::Note(surface_note(surface))).collect())
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

// Each surface is a note seam lent the whole tree and held to its stem, so a
// module is mined only through the surfaces that reach it.
fn surface_note(surface: &Surface) -> Note {
    Note {
        text: format!(
            "Surface `{name}` — entry `{entry}` — stem `{stem}`.\n\n\
             This call mines that one surface alone. Start at its entry and follow what the \
             surface reaches through the whole TypeScript / JavaScript tree under `$SOURCE_DIR` \
             — its handler, the modules it imports, the services and stores it calls, the types \
             it takes and returns — and emit claims for the behaviour a caller observes through \
             this surface alone. What the tree does for another surface is that surface's call \
             to claim, even in a module the two share. Anchor every `path` relative to \
             `$SOURCE_DIR`; extract mines only this source.",
            name = surface.name,
            entry = surface.entry,
            stem = surface.stem,
        ),
        stems: vec![surface.stem.clone()],
    }
}
