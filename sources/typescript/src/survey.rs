//! The survey of a code tree: the model groups the modules by the surface they serve.

use std::fmt::Write as _;
use std::path::Path;

use emery_sdk::survey::Entry;
use emery_sdk::{Context, Doc, Error, Model, Seam, SourceContent};

// Source files a group holds before it is mined on its own; a smaller one
// folds into the remainder's seam. A model call costs an agent start, so
// a surface of one module is not worth one.
const FLOOR: usize = 2;

// Directories holding no production source of the estate's own: dependencies,
// build output, tests. Dot directories are skipped besides.
const SKIP_DIRS: &[&str] =
    &["node_modules", "vendor", "target", "dist", "build", "tests", "__tests__"];

// The extensions this adapter mines.
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

// The segment before the extension that marks a file as no production
// source: a declaration file, or a test.
const MARKERS: &[&str] = &["d", "test", "spec"];

/// Returns the seams to mine: one per surface the model discerns, or the input whole.
///
/// One [`Seam::Note`] per group the model's survey cuts — the modules of one
/// surface, of at least two files — and one for the rest of the tree, each
/// lent the whole root and naming the files it mines. The survey turn runs
/// under the `prompts/survey.md` among `docs`, the adapter's embedded
/// corpus. Dependencies, build output, tests, declaration files, and dot
/// entries are not production source and are never offered. A tree no
/// directory cut would split, or an inline value, is the bound input whole —
/// a single call, with no survey turn spent on it.
///
/// # Errors
///
/// - [`Error::BadRequest`] when the model's grouping could not be brought
///   within its rounds, or for an entry whose name is not UTF-8.
/// - [`Error::ServerError`] when a directory cannot be read, or `docs`
///   holds no `prompts/survey.md`.
/// - [`Error::BadGateway`] for a tool or transport failure.
#[cfg(target_arch = "wasm32")]
pub async fn survey(ctx: &Context<'_>) -> Result<Vec<Seam>, Error> {
    cut(&emery_sdk::model::WasiModel, ctx, crate::guest::DOCS).await
}

/// Returns the seams to mine: one per surface the model discerns, or the input whole.
///
/// One [`Seam::Note`] per group the model's survey cuts — the modules of one
/// surface, of at least two files — and one for the rest of the tree, each
/// lent the whole root and naming the files it mines. The survey turn runs
/// under the `prompts/survey.md` among `docs`, the adapter's embedded
/// corpus. Dependencies, build output, tests, declaration files, and dot
/// entries are not production source and are never offered. A tree no
/// directory cut would split, or an inline value, is the bound input whole —
/// a single call, with no survey turn spent on it.
///
/// # Errors
///
/// - [`Error::BadRequest`] when the model's grouping could not be brought
///   within its rounds, or for an entry whose name is not UTF-8.
/// - [`Error::ServerError`] when a directory cannot be read, or `docs`
///   holds no `prompts/survey.md`.
/// - [`Error::BadGateway`] for a tool or transport failure.
#[cfg(not(target_arch = "wasm32"))]
pub async fn survey<P: Model>(
    model: &P, ctx: &Context<'_>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    cut(model, ctx, docs).await
}

async fn cut<P: Model>(
    model: &P, ctx: &Context<'_>, docs: &'static [Doc],
) -> Result<Vec<Seam>, Error> {
    let SourceContent::Workspace(root) = &ctx.input.content else {
        return Ok(vec![Seam::Whole]);
    };

    let files = emery_sdk::survey::files(Path::new(root), |path, entry| match entry {
        Entry::Dir => !hidden(path) && !SKIP_DIRS.contains(&name(path)),
        Entry::File => production(path),
    })?;
    // A tree of one directory is too small to be worth a survey turn.
    if emery_sdk::survey::by_directory(files.clone(), FLOOR).len() < 2 {
        return Ok(vec![Seam::Whole]);
    }

    let groups = emery_sdk::survey::by_model(model, ctx, docs, &files, FLOOR).await?;
    Ok(groups.into_iter().map(|group| Seam::Note(note(root, &group))).collect())
}

// The turn's seam: the root is lent whole, so imports and `tsconfig.json`
// resolve, and the files this call mines — one surface's modules, or the rest
// of the tree — are named.
fn note(root: &str, files: &[String]) -> String {
    let mut note = format!(
        "`$SOURCE_DIR` is the read-only view at `{root}` — the TypeScript / JavaScript source \
         tree. This call mines these files beneath it — the modules that serve one surface of \
         the estate, or what serves none in particular — and emits claims for them alone:"
    );
    for file in files {
        // Writing to a `String` cannot fail.
        let _ = write!(note, "\n- `{file}`");
    }
    note.push_str(
        "\n\nRead anything else under `$SOURCE_DIR` only to resolve what they reach — imports, \
         `tsconfig.json` paths, the types they use — never to mine it: another call covers it. \
         Anchor every `path` relative to `$SOURCE_DIR`. Nothing outside it is reachable; extract \
         mines only this source.",
    );
    note
}

// A production source file: a mined extension on a stem that is not marked
// as a declaration file or a test (`types.d.ts`, `mail.test.ts`).
fn production(path: &Path) -> bool {
    let name = name(path);
    if name.starts_with('.') {
        return false;
    }
    let mut segments = name.rsplit('.');
    let (Some(extension), Some(before)) = (segments.next(), segments.next()) else {
        return false;
    };
    let marked = segments.next().is_some() && MARKERS.contains(&before);
    EXTENSIONS.contains(&extension) && !marked
}

fn hidden(path: &Path) -> bool {
    name(path).starts_with('.')
}

// The entry's own name; `survey::files` offers UTF-8 paths alone.
fn name(path: &Path) -> &str {
    path.file_name().and_then(|name| name.to_str()).unwrap_or_default()
}
