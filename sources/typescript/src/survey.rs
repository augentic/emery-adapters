//! The survey of a code tree: the model finds the surfaces it exposes, one seam each.

use emery_sdk::survey::{Entry, Surface};
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

/// Returns the seams to mine: one per surface the source exposes, or an inline value whole.
///
/// The survey turn is put to the model `ctx` carries under the
/// `prompts/survey.md` among `docs`, the adapter's embedded corpus, with the
/// root lent so the model reads the tree itself. A surface's entry must be a
/// production module of the tree — dependencies, build output, tests,
/// declaration files, and dot entries are not production source, and an
/// entry named there goes back to the model as a finding. Each surface the
/// model finds — a route, a command, a job, an exported API — is one
/// [`Seam::Note`] lent the whole root and told the surface's name and entry,
/// so the extract call starts there, follows what the surface reaches
/// through the tree, and claims what a caller observes through it. A module
/// is mined through the surfaces that reach it, never on its own: there is
/// no remainder. An inline value is the bound input whole, with no survey
/// turn spent.
///
/// # Errors
///
/// - [`Error::BadRequest`] when the model finds no surface in the tree — a
///   source no caller reaches is incomplete input or a failed discovery, and
///   none of it is mined — or when the model's inventory could not be
///   brought within its rounds.
/// - [`Error::ServerError`] when `docs` holds no `prompts/survey.md`.
/// - [`Error::BadGateway`] for a tool or transport failure.
pub async fn survey<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &ctx.input.content else {
        return Ok(vec![Seam::Whole]);
    };

    let surfaces = emery_sdk::survey::surfaces(ctx, docs, keep).await?;
    if surfaces.is_empty() {
        return Err(bad_request!(
            "`{key}`: the source exposes no surface — nothing under the root registers a route, \
             a command, a job, or an exported API, so no caller reaches it; bind the tree that \
             declares its entry points",
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
