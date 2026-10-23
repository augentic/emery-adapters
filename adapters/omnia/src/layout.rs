//! The layout a slice lands in, as data: the lists and their anchors, the stem rules, and the reads of a tree the scaffold and the checks share.

mod manifest;

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use emery_sdk::{Error, bad_request, server_error};
use omnia_template::Manifest;

pub use self::manifest::Toml;
use crate::syntax;

// The root lists a stem's lines are added to, and the topic table.
pub const ROUTES: &str = "src/routes.rs";
pub const MESSAGING: &str = "src/messaging.rs";
pub const TOPICS: &str = "crates/common/src/topic.rs";

// The line of the root manifest the path list follows.
pub const PATH_LIST_ANCHOR: &str = "common = { path = \"crates/common\" }";
// The line each root list takes a stem's statement before.
pub const ROUTE_LIST_ANCHOR: &str = "router.with_state(";
pub const TOPIC_LIST_ANCHOR: &str = "tracing::debug!(";
// The comment each array of a stem's entry opens under.
pub const ROUTES_ARRAY_ANCHOR: &str = "// one entry per part with routes, each added at the end";
pub const TOPICS_ARRAY_ANCHOR: &str = "// one entry per part with topics, each added at the end";

// The crates every stem's manifest names, whether or not each module
// reaches them.
pub const STEM_DEPENDENCIES: [&str; 6] =
    ["axum", "common", "omnia-sdk", "serde", "serde_json", "tracing"];
// The crate a stem's manifest names beside them where the slice probes,
// for the `Row` its tests arrange the store's answer with, or reaches a
// SQL store, for the `TableStore` it queries through; laid by the scaffold
// and held to no use, since a probe over the state store alone never
// spells it.
pub const PROBE_DEPENDENCY: &str = "omnia-wasi-sql";

// The topic table as a tree holds it: every `pub const` of the table, its
// topic where the value is a string literal.
#[derive(Default)]
pub struct TopicTable {
    pub constants: Vec<syntax::Const>,
}

impl TopicTable {
    // The table of the tree at `root`, empty where the tree lacks it.
    pub fn read(root: &Path) -> Result<Self, Error> {
        let file = root.join(TOPICS);
        let text = match fs::symlink_metadata(&file) {
            Ok(meta) if meta.is_file() => read(&file)?,
            Ok(_) => return Err(bad_request!("`{TOPICS}` is not a regular file")),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => return Err(server_error!("reading `{TOPICS}`: {err}")),
        };
        let parsed = syntax::parse(&text).map_err(|unparsed| {
            bad_request!(
                "`{TOPICS}` does not parse at line {}, so the slice's topics cannot be read \
                 against it: {}",
                unparsed.line,
                unparsed.message
            )
        })?;
        Ok(Self::of(&parsed))
    }

    pub fn of(file: &syn::File) -> Self {
        Self {
            constants: syntax::pub_consts(file),
        }
    }
}

// The directory under `crates/` a stem's crate lives at: `common-stem` for
// `common`, since `crates/common/` is the shared crate, and `<stem>`
// otherwise.
pub fn crate_dir(stem: &str) -> &str {
    if stem == "common" { "common-stem" } else { stem }
}

// A stem named as a crate the template pins, or as `common` or `guest`,
// takes the package name `<stem>-stem`, so the names never collide.
pub fn package(stem: &str) -> String {
    if stem == "common" || stem == "guest" || template_pins(stem) {
        format!("{stem}-stem")
    } else {
        stem.to_owned()
    }
}

fn template_pins(name: &str) -> bool {
    omnia_template::entry(Manifest::PATH).is_some_and(|entry| Toml::read(entry.body).pins(name))
}

pub fn is_start_stem(name: &str) -> bool {
    name == "start"
        || name
            .strip_prefix("start")
            .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
}

fn is_start_crate(dir: &Path) -> bool {
    dir.file_name().and_then(|name| name.to_str()).is_some_and(is_start_stem)
}

// Whether a crate directory under `crates/` is a stem's: `common` is the
// shared crate and a `start` crate the process rule's.
pub fn is_stem_crate(dir: &str) -> bool {
    dir != "common" && !is_start_stem(dir)
}

pub fn read(file: &Path) -> Result<String, Error> {
    fs::read_to_string(file).map_err(|err| server_error!("reading `{}`: {err}", file.display()))
}

pub fn template(path: &str) -> Result<&'static str, Error> {
    omnia_template::entry(path)
        .map(|entry| entry.body)
        .ok_or_else(|| server_error!("the template holds no `{path}`"))
}

pub fn refusal(err: &omnia_template::Error) -> Error {
    match err {
        omnia_template::Error::Occupied { .. } => bad_request!("{err}"),
        _ => server_error!("{err}"),
    }
}

pub fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root).unwrap_or(file).display().to_string()
}

fn entries(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(server_error!("listing `{}`: {err}", dir.display())),
    };
    let mut paths = listing
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| server_error!("listing `{}`: {err}", dir.display()))?;
    paths.sort();
    Ok(paths)
}

// The directories under `crates/` that hold a file, by name; an empty one
// `delete` left behind is none.
pub fn crates(root: &Path) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    for path in entries(&root.join("crates"))? {
        let meta = fs::symlink_metadata(&path)
            .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
        if !meta.is_dir() || !has_files(&path)? {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

// Every file under a `start` crate, root-relative and sorted.
pub fn start_files(root: &Path) -> Result<Vec<String>, Error> {
    let mut found = Vec::new();
    for dir in entries(&root.join("crates"))? {
        if !is_start_crate(&dir) {
            continue;
        }
        for file in files(&dir)? {
            found.push(relative(root, &file));
        }
    }
    found.sort();
    Ok(found)
}

// Every file of the tree, root-relative with `/` between segments, as a
// commit would carry it: nothing under `.git` or `.emery`, and nothing
// under the root's `target/`, which the checks fill.
pub fn listing(root: &Path) -> Result<BTreeSet<String>, Error> {
    let mut found = BTreeSet::new();
    let skip = |dir: &Path, name: &str| {
        name == ".git" || name == ".emery" || (dir == root && name == "target")
    };
    walk(root, skip, |path, _| {
        let relative: Vec<String> = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        found.insert(relative.join("/"));
        ControlFlow::Continue(())
    })?;
    Ok(found)
}

fn has_files(dir: &Path) -> Result<bool, Error> {
    let mut found = false;
    walk(
        dir,
        |_, _| false,
        |_, meta| {
            found = meta.is_file();
            if found { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
        },
    )?;
    Ok(found)
}

// Every regular file under `dir`, sorted; a link is none.
pub fn files(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut found = Vec::new();
    walk(
        dir,
        |_, _| false,
        |path, meta| {
            if meta.is_file() {
                found.push(path);
            }
            ControlFlow::Continue(())
        },
    )?;
    found.sort();
    Ok(found)
}

// Walks `dir` depth first, each directory's entries in name order and no
// link followed: an entry `skip` names, by the directory it stands in and
// its name, is passed over with whatever lies beneath it, a directory is
// entered, and every other entry is handed to `each` with what stands at it
// until `each` breaks.
fn walk(
    dir: &Path, skip: impl Fn(&Path, &str) -> bool,
    mut each: impl FnMut(PathBuf, &fs::Metadata) -> ControlFlow<()>,
) -> Result<(), Error> {
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in entries(&dir)? {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
            if skip(&dir, name) {
                continue;
            }
            let meta = fs::symlink_metadata(&path)
                .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
            if meta.is_dir() {
                pending.push(path);
            } else if each(path, &meta).is_break() {
                return Ok(());
            }
        }
    }
    Ok(())
}

pub fn sources(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    Ok(files(dir)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect())
}
