//! The TypeScript / JavaScript adapter and its survey by model.
//!
//! A code tree is mined one externally visible surface at a time — a route, a
//! command, a job, an exported API — with the whole tree in view. Which
//! modules serve one surface is no directory layout's to state, so the survey
//! asks the model once under `prompts/survey.md`; and a handler's behaviour
//! runs through its imports and `tsconfig.json`, so each seam is lent the
//! root and told which files are its own.

use std::fmt::Write as _;
use std::path::Path;

use emery_sdk::survey::{self, Entry};
use emery_sdk::{Context, Doc, Error, Model, Seam, SourceAdapter, SourceContent, SourceKind};

use crate::registry;

/// The adapter over a TypeScript or JavaScript source tree.
#[derive(Debug)]
pub struct Adapter;

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

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Behaviour;

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    // One `Note` per group the model's survey cuts — the modules of
    // one surface, of at least `FLOOR` files — and one for the rest of the
    // tree, each lent the whole root and naming the files it mines. A tree
    // no directory cut would split, or an inline value, is the bound input
    // whole — a single call, with no survey turn spent on it.
    async fn survey<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Vec<Seam>, Error> {
        let SourceContent::Workspace(root) = &ctx.input.content else {
            return Ok(vec![Seam::Whole]);
        };

        let files = survey::files(Path::new(root), |path, entry| match entry {
            Entry::Dir => !hidden(path) && !SKIP_DIRS.contains(&name(path)),
            Entry::File => production(path),
        })?;
        // A tree of one directory is too small to be worth a survey turn.
        if survey::by_directory(files.clone(), FLOOR).len() < 2 {
            return Ok(vec![Seam::Whole]);
        }

        let groups = survey::by_model(model, ctx, registry::docs(), &files, FLOOR).await?;
        Ok(groups.into_iter().map(|group| Seam::Note(note(root, &group))).collect())
    }
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
