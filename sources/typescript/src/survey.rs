//! Discovers the caller-facing surfaces exposed by a source tree.
//!
//! Each discovered surface becomes an independent mining seam. Inline input
//! requires no discovery and is returned as one whole seam.

use emery_sdk::survey::Surface;
use emery_sdk::workspace::Entry;
use emery_sdk::{Context, Doc, Error, Model, Seam, SourceContent, bad_request};

// Directories holding no production source of the estate's own: dependencies,
// build output, tests. Dot directories are skipped besides.
const SKIP_DIRS: &[&str] =
    &["node_modules", "vendor", "target", "dist", "build", "tests", "__tests__"];

// The extensions this adapter mines.
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

// The segment before the extension that marks a file as no production
// source: a declaration file, or a test.
const MARKERS: &[&str] = &["d", "test", "spec"];

/// Returns one mining seam for each surface exposed by the source.
///
/// Inline input produces one [`Seam::Whole`] without querying the model. For
/// workspace input, the model identifies each surface and its entry module.
/// Entry modules must be production TypeScript or JavaScript files; hidden
/// entries, dependencies, build output, tests, and declaration files are
/// rejected.
///
/// Each surface receives the full source tree so extraction can follow its
/// imports. A module is mined only through the surfaces that reach it.
///
/// # Errors
///
/// - Returns [`Error::BadRequest`] when no surface is found or the model
///   cannot produce a valid inventory within its available rounds.
/// - Returns [`Error::ServerError`] when `docs` does not contain `survey.md`.
/// - Returns [`Error::BadGateway`] when a model tool or transport fails.
pub async fn survey<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &ctx.input.content else {
        return Ok(vec![Seam::Whole]);
    };

    let surfaces = emery_sdk::survey::surfaces(ctx, docs, keep).await?;
    if surfaces.is_empty() {
        return Err(bad_request!(
            "`{key}`: the source exposes no surface. Nothing under the root registers a route, \
             a command, a job, or an exported API.",
            key = ctx.input.key,
        ));
    }

    Ok(surfaces.iter().map(|surface| Seam::Note(note(root, surface))).collect())
}

// What a surface may be entered at: a production module, in no dependency,
// build, or test directory and no dot entry.
fn keep(entry: Entry<'_>) -> bool {
    !entry.hidden()
        && match entry {
            Entry::Dir(_) => !SKIP_DIRS.contains(&entry.name()),
            Entry::File(_) => production(entry.name()),
        }
}

// The turn's seam: the root is lent whole, so the surface can be followed
// wherever it reaches — imports, `tsconfig.json` paths, the types it uses —
// and the call is told which surface is its own and where a caller enters it.
fn note(root: &str, surface: &Surface) -> String {
    format!(
        "`$SOURCE_DIR` is the read-only view at `{root}` — the TypeScript / JavaScript source \
         tree. This call mines one surface the source exposes:\n\n\
         - surface: {name}\n\
         - entry: `{entry}`\n\n\
         Start at the entry and follow what the surface reaches through the whole tree — its \
         handler, the modules it imports, the services and stores it calls, the types it takes \
         and returns — and emit claims for the behaviour a caller observes through this surface \
         alone. What the tree does for another surface is that surface's call to claim, even in \
         a module the two share. Anchor every `path` relative to `$SOURCE_DIR`. Nothing outside \
         it is reachable; extract mines only this source.",
        name = surface.name,
        entry = surface.entry,
    )
}

// A production source file: a mined extension on a stem that is not marked
// as a declaration file or a test (`types.d.ts`, `mail.test.ts`).
fn production(name: &str) -> bool {
    let mut segments = name.rsplit('.');
    let (Some(extension), Some(before)) = (segments.next(), segments.next()) else {
        return false;
    };
    let marked = segments.next().is_some() && MARKERS.contains(&before);
    EXTENSIONS.contains(&extension) && !marked
}
