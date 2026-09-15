//! TypeScript / JavaScript sources are code trees. Extract mines them one
//! top-level directory at a time with the whole tree in view: a handler's
//! behaviour runs through its imports and `tsconfig.json`, so each material
//! is lent the root and told which files are its own.

use std::fmt::Write as _;
use std::path::Path;

use emery_prose::registry::Doc;
use emery_sdk::survey::{self, Entry};
use emery_sdk::{Context, Error, Material, SourceAdapter, SourceContent, SourceKind};

use crate::registry;

/// TypeScript / JavaScript source trees → one code Evidence document.
#[derive(Debug)]
pub struct Adapter;

// Source files a directory holds before it is mined on its own; a smaller
// one folds into the root's material. A model call costs an agent start, so
// a directory of one module is not worth one.
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
    const SOURCE: &'static str = "TypeScript / JavaScript";

    fn docs() -> &'static [Doc] {
        registry::docs()
    }

    // One `Prepared` note per top-level directory of at least `FLOOR` source
    // files and one for the rest of the tree, each lent the whole root and
    // naming the files it mines. A tree that cuts no finer than itself, or an
    // inline value, is the bound input whole — a single call, as before.
    fn survey(ctx: &Context<'_>) -> Result<Vec<Material>, Error> {
        let SourceContent::Workspace(root) = &ctx.input.content else {
            return Ok(vec![Material::Bound]);
        };

        let files = survey::files(Path::new(root), |path, entry| match entry {
            Entry::Dir => !hidden(path) && !SKIP_DIRS.contains(&name(path)),
            Entry::File => production(path),
        })?;
        let groups = survey::by_directory(files, FLOOR);
        if groups.len() < 2 {
            return Ok(vec![Material::Bound]);
        }
        Ok(groups.into_iter().map(|group| Material::Prepared(note(root, &group))).collect())
    }
}

// The turn's material: the root is lent whole, so imports and `tsconfig.json`
// resolve, and the files this call mines are named.
fn note(root: &str, files: &[String]) -> String {
    let mut note = format!(
        "`$SOURCE_DIR` is the read-only view at `{root}` — the TypeScript / JavaScript source \
         tree. This call mines these files beneath it, and emits claims for them alone:"
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
