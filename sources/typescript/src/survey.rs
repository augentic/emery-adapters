//! Discovers the caller-facing surfaces exposed by a source tree.
//!
//! Each discovered surface becomes an independent mining seam. Inline input,
//! and a tree of one production module, require no discovery and are one
//! whole seam.

use emery_sdk::survey::Surface;
use emery_sdk::workspace::Entry;
use emery_sdk::{Context, Doc, Error, Model, Seam, SourceContent, bad_request};

const SKIP_DIRS: &[&str] =
    &["node_modules", "vendor", "target", "dist", "build", "tests", "__tests__"];
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
const MARKERS: &[&str] = &["d", "test", "spec"];

// Surveys workspace input by model: each surface becomes a note seam lent the
// whole tree, so a module is mined only through the surfaces that reach it.
// An inline value, or a tree of one production module, cannot be cut and is
// one whole seam with no survey turn spent.
pub async fn survey<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    // a tree that cannot be cut
    let source = &ctx.input.name;
    let SourceContent::Workspace(root) = &ctx.input.content else {
        tracing::debug!(%source, "inline value; one whole seam");
        return Ok(vec![Seam::Whole]);
    };
    let modules = emery_sdk::workspace::list(root, keep)?;
    match modules.as_slice() {
        [] => {
            return Err(bad_request!(
                "`{source}`: the source holds no production module. Nothing under the root is a \
                 TypeScript or JavaScript file this adapter mines."
            ));
        }
        [only] => {
            tracing::debug!(%source, module = %only, "one production module; one whole seam");
            return Ok(vec![Seam::Whole]);
        }
        _ => {}
    }

    // the survey turn
    tracing::info!(%source, %root, "identifying typescript surfaces");
    let surfaces = emery_sdk::survey::surfaces(ctx, docs, keep).await?;
    if surfaces.is_empty() {
        return Err(bad_request!(
            "`{source}`: the source exposes no surface. Nothing under the root registers a route, \
             a command, a job, or an exported API."
        ));
    }

    // one note seam per surface
    let seams: Vec<_> = surfaces.iter().map(|surface| Seam::Note(note(root, surface))).collect();
    tracing::debug!(%source, seams = seams.len(), "note seams");
    Ok(seams)
}

fn keep(entry: Entry<'_>) -> bool {
    !entry.hidden()
        && match entry {
            Entry::Dir(_) => !SKIP_DIRS.contains(&entry.name()),
            Entry::File(_) => production(entry.name()),
        }
}

// The root is lent whole, so the call can follow the surface wherever it reaches.
// The first line names the surface, so it is the seam's label in the run's log.
fn note(root: &str, surface: &Surface) -> String {
    format!(
        "Surface `{name}` — entry `{entry}`.\n\n\
         `$SOURCE_DIR` is the read-only view at `{root}` — the TypeScript / JavaScript source \
         tree. This call mines that one surface alone. Start at its entry and follow what the \
         surface reaches through the whole tree — its handler, the modules it imports, the \
         services and stores it calls, the types it takes and returns — and emit claims for the \
         behaviour a caller observes through this surface alone. What the tree does for another \
         surface is that surface's call to claim, even in a module the two share. Anchor every \
         `path` relative to `$SOURCE_DIR`. Nothing outside it is reachable; extract mines only \
         this source.",
        name = surface.name,
        entry = surface.entry,
    )
}

// A mined extension on a stem not marked as a declaration file or a test.
fn production(name: &str) -> bool {
    let mut segments = name.rsplit('.');
    let (Some(extension), Some(before)) = (segments.next(), segments.next()) else {
        return false;
    };
    let marked = segments.next().is_some() && MARKERS.contains(&before);
    EXTENSIONS.contains(&extension) && !marked
}
