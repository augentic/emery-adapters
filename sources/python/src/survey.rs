//! Chooses the seams of a Python source and the claims its code states.
//!
//! An inline value is one seam mined whole. A workspace is listed under this
//! adapter's keep, every production module parsed, and its surfaces named
//! by the model over the facts the parser read (`model`). The seams are cut
//! from the surfaces:
//!
//! - a tree within the SDK's inline budget, or of one module, is one seam
//!   over every module, held to every stem its surfaces carry
//! - a larger tree is one seam per stem, over the modules the surfaces
//!   under it reach, the entry first; where one of those imports what the
//!   resolver cannot follow or loads a module by a computed name, the rest
//!   of the tree follows
//! - a tree the survey names no surface in is cut mechanically: one seam
//!   under the package's or the root directory's name within the budget,
//!   one per top-level directory past it
//!
//! A seam's data files sit among its modules — a small one directly after
//! the first module naming it, a large one after them all — and the tree's
//! own tests follow those. A test is read for what it states, never mined:
//! its statements are listed in the brief of each seam whose modules it
//! imports. Only a tree with no production module is refused.

use std::collections::BTreeMap;
use std::path::Path;

use emery_sdk::workspace::Entry;
use emery_sdk::{Claim, Error, INLINE_BYTES, Seam, SourceContent, SourceInput, bad_request};

use self::resolve::{Manifest, Resolver};
use self::skeleton::Test;
use self::surface::{Surface, Tree};

pub mod model;
mod parse;
mod resolve;
mod skeleton;
mod surface;

pub struct Survey {
    pub seams: Vec<Seam>,
    // Declared by the modules the seams reach; the guest joins them after
    // the model's answer.
    pub types: Vec<Claim>,
}

pub enum Preparation {
    // An inline value needs no survey turn: one seam, its declarations
    // unanchored.
    Value(Survey),
    Workspace(Prepared),
}

pub struct Prepared {
    // For the log.
    pub source: String,
    // The lent workspace root.
    pub root: String,
    pub tree: Tree,
    tests: Vec<Test>,
}

// Refuses a workspace with no production module, and nothing else.
pub fn prepare(input: &SourceInput) -> Result<Preparation, Error> {
    let source = &input.name;

    // an inline value is one seam, with no survey
    let workspace = match &input.content {
        SourceContent::Value(text) => {
            let module = parse::parse("value.py", text.clone());
            let types = skeleton::types([&module], false);
            return Ok(Preparation::Value(Survey {
                seams: vec![Seam::whole()],
                types,
            }));
        }
        SourceContent::Workspace(workspace) => workspace,
    };

    // one walk lists the modules and the data files a string may name
    let (modules, data): (Vec<String>, Vec<String>) =
        emery_sdk::workspace::list(workspace, |entry| include(entry) || is_data(entry))?
            .into_iter()
            .partition(|path| Path::new(path).extension().is_some_and(|ext| ext == "py"));
    if modules.is_empty() {
        return Err(bad_request!(
            "`{source}` has no production module: no Python source outside tests, stubs, \
             virtualenvs, caches, build output, and migrations."
        ));
    }

    let root = Path::new(workspace);
    let tree = read(root, modules, data);
    let tests = tests(root, emery_sdk::workspace::list(workspace, is_test)?, &tree);

    Ok(Preparation::Workspace(Prepared {
        source: source.clone(),
        root: workspace.clone(),
        tree,
        tests,
    }))
}

pub fn seams(prepared: &Prepared, surfaces: &[Surface]) -> Survey {
    let Prepared {
        source,
        root: workspace,
        tree,
        tests,
    } = prepared;

    logged(source, "surveyed", surfaces);

    // choose the cut by size and by whether any surface was named
    let size: usize = tree.modules.values().map(|module| module.text.len()).sum();
    let fits = tree.modules.len() == 1 || u64::try_from(size).unwrap_or(u64::MAX) <= INLINE_BYTES;
    let leads = match (surfaces.is_empty(), fits) {
        (false, true) => vec![whole(tree, surfaces)],
        (false, false) => by_stem(tree, surfaces),
        (true, true) => vec![unsurfaced(tree, workspace)],
        (true, false) => by_directory(tree, workspace),
    };
    let seams: Vec<Seam> =
        leads.into_iter().map(|lead| finish(workspace, tree, lead, surfaces, tests)).collect();

    let reached = unique(seams.iter().flat_map(|seam| seam.files.iter().map(String::as_str)));
    let types = skeleton::types(reached.iter().filter_map(|path| tree.modules.get(*path)), true);

    Survey { seams, types }
}

fn logged(source: &str, what: &str, surfaces: &[Surface]) {
    if !emery_sdk::tracing::enabled!(emery_sdk::tracing::Level::TRACE) {
        return;
    }
    let listed: Vec<emery_sdk::serde_json::Value> = surfaces
        .iter()
        .map(|surface| {
            emery_sdk::serde_json::json!({
                "name": surface.name,
                "entry": surface.entry,
                "stem": surface.stem,
                "ids": surface.ids,
            })
        })
        .collect();
    let json = emery_sdk::serde_json::Value::Array(listed).to_string();
    emery_sdk::tracing::trace!(%source, surfaces = %json, "{what}");
}

// Never a production module's: dependencies, environments, caches, build
// output, coverage, documentation, tests, and schema history.
const SKIP_DIRS: &[&str] = &[
    "__pycache__",
    "node_modules",
    "vendor",
    "target",
    "venv",
    "env",
    "site-packages",
    "build",
    "dist",
    "htmlcov",
    "docs",
    "test",
    "tests",
    "testing",
    "__tests__",
    "features",
    "migrations",
];
// Build and test configuration, not the program. `manage.py` stays: it is
// Django's bootstrap. `setup.py` is the packaging script at the root alone;
// beneath a package it is a module like any other.
const SKIP_FILES: &[&str] = &["conftest.py", "noxfile.py"];
const SKIP_ROOT_FILES: &[&str] = &["setup.py"];
const EXTENSIONS: &[&str] = &["py"];
const DATA_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "csv", "ini"];

fn push_unique<T: PartialEq>(into: &mut Vec<T>, item: T) {
    if !into.contains(&item) {
        into.push(item);
    }
}

// First-occurrence order.
fn unique<T: PartialEq>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut list = Vec::new();
    for item in items {
        push_unique(&mut list, item);
    }
    list
}

// Alembic's `versions` is schema history, not a package.
fn skipped_dir(entry: Entry<'_>) -> bool {
    let name = entry.name();
    SKIP_DIRS.contains(&name)
        || name.ends_with(".egg-info")
        || (name == "versions" && entry.path().ends_with("alembic/versions"))
}

fn include(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !skipped_dir(entry);
    }
    let name = entry.name();
    if SKIP_FILES.contains(&name) || (SKIP_ROOT_FILES.contains(&name) && entry.path() == name) {
        return false;
    }
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    EXTENSIONS.contains(&extension) && !is_test_name(stem)
}

fn is_test_name(stem: &str) -> bool {
    stem.starts_with("test_") || stem.ends_with("_test") || stem == "tests"
}

// Candidates only: a data file is laid only where a module names it by path.
fn is_data(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !skipped_dir(entry);
    }
    entry.extension().is_some_and(|extension| DATA_EXTENSIONS.contains(&extension))
}

const TEST_DIRS: &[&str] = &["test", "tests", "testing", "__tests__", "features"];
const NEVER_DIRS: &[&str] = &[
    "__pycache__",
    "node_modules",
    "vendor",
    "target",
    "venv",
    "env",
    "site-packages",
    "build",
    "dist",
    "htmlcov",
    "docs",
    "migrations",
];

fn is_test(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !NEVER_DIRS.contains(&entry.name()) && !entry.name().ends_with(".egg-info");
    }
    let name = entry.name();
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    if extension == "feature" {
        return true;
    }
    if !EXTENSIONS.contains(&extension) || SKIP_FILES.contains(&name) {
        return false;
    }
    is_test_name(stem) || entry.path().split('/').rev().skip(1).any(|dir| TEST_DIRS.contains(&dir))
}

fn tests(root: &Path, paths: Vec<String>, tree: &Tree) -> Vec<Test> {
    let mut tests: Vec<Test> = Vec::new();
    let mut features: Vec<Test> = Vec::new();

    // read each test for what it states and what it imports
    for path in paths {
        let text = match std::fs::read_to_string(root.join(&path)) {
            Ok(text) => text,
            Err(error) => {
                emery_sdk::tracing::warn!(path, %error, "test is not readable text; left out");
                continue;
            }
        };
        if path.ends_with(".feature") {
            features.push(Test {
                path,
                imports: Vec::new(),
                statements: skeleton::scenarios(&text),
            });
            continue;
        }
        let mut module = parse::parse(&path, text);
        tree.resolver.settle(&mut module);
        tests.push(Test {
            path,
            imports: module.reached().into_iter().map(str::to_owned).collect(),
            statements: skeleton::statements(&module),
        });
    }

    // a feature inherits the imports of the tests under its directory's parent
    for mut feature in features {
        let beside = feature
            .path
            .rsplit_once('/')
            .and_then(|(dir, _)| dir.rsplit_once('/'))
            .map_or_else(String::new, |(parent, _)| format!("{parent}/"));
        for test in tests.iter().filter(|test| test.path.starts_with(&beside)) {
            for import in &test.imports {
                push_unique(&mut feature.imports, import.clone());
            }
        }
        tests.push(feature);
    }
    tests.retain(|test| !test.statements.is_empty());
    tests
}

// Seals a lead into its seam. A tree with no surface has no anchors, so its
// exports are read for what they do. Data files sit among the modules in
// `files` and tests follow them, never among the anchors; a test importing
// no module of the tree follows every seam.
fn finish(root: &str, tree: &Tree, lead: Lead, surfaces: &[Surface], tests: &[Test]) -> Seam {
    let Lead {
        seam: Seam {
            text, files, stems, ..
        },
        widened,
    } = lead;
    let under: Vec<&Surface> =
        surfaces.iter().filter(|surface| stems.contains(&surface.stem)).collect();
    let anchors = if surfaces.is_empty() {
        Vec::new()
    } else {
        skeleton::anchors(tree, &files, under.iter().copied())
    };
    let attached: Vec<&Test> = tests
        .iter()
        .filter(|test| test.imports.is_empty() || test.imports.iter().any(|m| files.contains(m)))
        .collect();
    let text = brief(tree, text, &files, widened, &attached);
    let mut files = with_data(root, tree, files);
    files.extend(attached.iter().map(|test| test.path.clone()));
    Seam {
        text,
        files,
        stems,
        anchors,
    }
}

// A data file within this many bytes is laid directly after the first module
// naming it, so a long module later in the seam's order — a generated table
// — cannot end the laid run before a value a `criterion` would cite; a
// larger one is laid after every module.
const DATA_BESIDE_BYTES: u64 = 8 * 1024;

// The seam's modules with the data files they name placed among them: a
// small one after the first module naming it, a large one after them all.
fn with_data(root: &str, tree: &Tree, modules: Vec<String>) -> Vec<String> {
    let mut files: Vec<String> = Vec::with_capacity(modules.len());
    let mut large: Vec<String> = Vec::new();
    for path in modules {
        let named = tree.modules.get(&path).map(parse::Module::data).unwrap_or_default();
        files.push(path);
        for data in named {
            if files.iter().chain(&large).any(|file| file == data) {
                continue;
            }
            let size = std::fs::metadata(Path::new(root).join(data)).map_or(u64::MAX, |m| m.len());
            if size <= DATA_BESIDE_BYTES {
                files.push(data.to_owned());
            } else {
                large.push(data.to_owned());
            }
        }
    }
    files.extend(large);
    files
}

fn read(root: &Path, paths: Vec<String>, data: Vec<String>) -> Tree {
    let mut modules = BTreeMap::new();
    for path in paths {
        match std::fs::read_to_string(root.join(&path)) {
            Ok(text) => {
                modules.insert(path.clone(), parse::parse(&path, text));
            }
            Err(error) => {
                emery_sdk::tracing::warn!(path, %error, "module is not readable text; left out");
            }
        }
    }
    let manifest = Manifest::read(root);
    let resolver = Resolver::new(modules.keys().cloned(), data, manifest.name.as_deref());
    for module in modules.values_mut() {
        resolver.settle(module);
    }
    Tree {
        modules,
        resolver,
        manifest,
    }
}

struct Lead {
    seam: Seam,
    // The files run past the surfaces' closures to the rest of the tree.
    // Only the cut by stem sets it.
    widened: bool,
}

impl Lead {
    fn new(text: String, files: Vec<String>, stems: Vec<String>) -> Self {
        Self {
            seam: Seam {
                text,
                files,
                stems,
                ..Seam::default()
            },
            widened: false,
        }
    }
}

// Every module, the surfaces' closures first.
fn whole(tree: &Tree, surfaces: &[Surface]) -> Lead {
    let files = unique(
        surfaces
            .iter()
            .flat_map(|surface| surface.closure.iter().cloned())
            .chain(tree.modules.keys().cloned()),
    );
    let stems = unique(surfaces.iter().map(|surface| surface.stem.clone()));
    let text = format!(
        "The surfaces of this source, found by reading its code — where control enters it from \
         outside the process:\n\n{}\n\nEvery `requirement` and `criterion` belongs to one of these \
         surfaces: lead its id with that surface's id, and claim a behaviour under the surface \
         whose caller observes it, once.",
        listed(surfaces)
    );
    Lead::new(text, files, stems)
}

fn by_stem(tree: &Tree, surfaces: &[Surface]) -> Vec<Lead> {
    unique(surfaces.iter().map(|surface| surface.stem.as_str()))
        .into_iter()
        .map(|stem| {
            let under: Vec<&Surface> = surfaces.iter().filter(|s| s.stem == stem).collect();
            let mut files =
                unique(under.iter().flat_map(|surface| surface.closure.iter().cloned()));

            // widen to the rest of the tree past an import the resolver could not follow
            let unfollowed = files
                .iter()
                .filter_map(|path| tree.modules.get(path))
                .any(|module| !module.dynamic.is_empty() || !module.unresolved().is_empty());
            let rest: Vec<String> = if unfollowed {
                tree.modules.keys().filter(|path| !files.contains(*path)).cloned().collect()
            } else {
                Vec::new()
            };
            let widened = !rest.is_empty();
            files.extend(rest);

            let (count, reach, whose) = match under.len() {
                1 => ("surface".to_owned(), "it reaches", "its"),
                n => (format!("{n} surfaces"), "they reach", "their"),
            };
            let text = format!(
                "This call mines the {count} under the stem `{stem}` alone:\n\n{}\n\nThe files \
                 below are what {reach} from {whose} entry, the entry first. What the tree does for \
                 another surface is that surface's call to claim, even in a module the two share.",
                listed(under.iter().copied()),
            );
            Lead {
                widened,
                ..Lead::new(text, files, vec![stem.to_owned()])
            }
        })
        .collect()
}

const NO_SURFACE: &str = "No surface was found in this source: its survey named no route, command, \
                          job, consumer, or exported API — no bootstrap the manifest's scripts \
                          name or a conventional entry holds, no handler registered with a \
                          package, no function or class under a package's decorator, and no \
                          function or class exported at an entry module for a caller. Read it as \
                          a library is read — for what its exports do for a caller — and claim \
                          what the code exhibits.";

fn unsurfaced(tree: &Tree, root: &str) -> Lead {
    let files: Vec<String> = tree.modules.keys().cloned().collect();
    let stem = fallback_stem(tree, root);
    let text = format!(
        "{NO_SURFACE} The modules below are the whole source, mined under the one stem `{stem}`: \
         lead every `requirement` and `criterion` id with it, and name each behaviour for the \
         export that exhibits it."
    );
    Lead::new(text, files, vec![stem])
}

fn by_directory(tree: &Tree, root: &str) -> Vec<Lead> {
    // cut beneath `src/` and beneath a lone top-level package
    let packages: Vec<&str> = tree
        .modules
        .keys()
        .filter_map(|path| path.strip_suffix("/__init__.py"))
        .map(|package| package.strip_prefix("src/").unwrap_or(package))
        .filter(|package| !package.contains('/'))
        .collect();
    let package = match packages.as_slice() {
        [one] => Some(format!("{one}/")),
        _ => None,
    };

    // group the modules by top-level directory, under its name
    let mut groups: Vec<(String, String, Vec<String>)> = Vec::new();
    let mut loose: Vec<String> = Vec::new();
    for path in tree.modules.keys() {
        let rest = path.strip_prefix("src/").unwrap_or(path);
        let rest =
            package.as_deref().and_then(|package| rest.strip_prefix(package)).unwrap_or(rest);
        let Some((name, _)) = rest.split_once('/') else {
            loose.push(path.clone());
            continue;
        };
        let dir = &path[..path.len() - rest.len() + name.len()];
        let stem = surface::kebab(name).unwrap_or_else(|| fallback_stem(tree, root));
        match groups.iter_mut().find(|(s, ..)| *s == stem) {
            Some((_, _, files)) => files.push(path.clone()),
            None => groups.push((stem, format!("`{dir}/`"), vec![path.clone()])),
        }
    }

    // the root's own modules join the first group
    if groups.is_empty() {
        groups.push((fallback_stem(tree, root), "the root".to_owned(), Vec::new()));
    }
    if !loose.is_empty() {
        let (_, dir, files) = &mut groups[0];
        loose.append(files);
        *files = loose;
        dir.push_str(", with the root's own modules");
    }

    groups
        .into_iter()
        .map(|(stem, dir, files)| {
            let text = format!(
                "{NO_SURFACE} It is past the budget of one call and cut by directory: this call \
                 mines the {} under {dir} alone, under the stem `{stem}` — lead every `requirement` \
                 and `criterion` id with it. What another directory's modules do is another \
                 call's to claim, even where these import them.",
                if files.len() == 1 { "module".to_owned() } else { format!("{} modules", files.len()) },
            );
            Lead::new(text, files, vec![stem])
        })
        .collect()
}

fn fallback_stem(tree: &Tree, root: &str) -> String {
    let named = tree.manifest.name.as_deref().and_then(surface::kebab);
    named
        .or_else(|| {
            Path::new(root).file_name().and_then(|name| name.to_str()).and_then(surface::kebab)
        })
        .unwrap_or_else(|| "module".to_owned())
}

fn brief(tree: &Tree, lead: String, files: &[String], widened: bool, tests: &[&Test]) -> String {
    let modules = || files.iter().filter_map(|path| tree.modules.get(path));
    let mut sections = vec![lead];
    sections.extend(skeleton::boundaries(modules()));
    sections.extend(skeleton::packages(modules()));
    sections.extend(skeleton::calls(tree, files));
    sections.extend(skeleton::decisions(modules()));
    sections.extend(skeleton::data(modules()));
    sections.extend(skeleton::stated(tests.iter().copied()));
    sections.extend(skeleton::unfollowed(modules(), widened));
    let unparsed: Vec<String> =
        modules().filter(|m| !m.parsed).map(|m| format!("`{}`", m.path)).collect();
    if !unparsed.is_empty() {
        sections.push(format!(
            "The parser could not read {} whole; what {} declares is not in the lists above and is \
             read from the text alone.",
            unparsed.join(", "),
            if unparsed.len() == 1 { "it" } else { "each" }
        ));
    }
    sections.join("\n\n")
}

fn listed<'s>(surfaces: impl IntoIterator<Item = &'s Surface>) -> String {
    surfaces
        .into_iter()
        .map(|surface| {
            let ids: Vec<String> = surface.ids.iter().map(|id| format!("`{id}`")).collect();
            let reached: Vec<String> = surface
                .closure
                .iter()
                .filter(|path| **path != surface.entry)
                .map(|path| format!("`{path}`"))
                .collect();
            format!(
                "- Surface `{}` — entry `{}` — stem `{}`: {}; {} {}; {}.",
                surface.name,
                surface.entry,
                surface.stem,
                surface.detail.join("; "),
                if ids.len() == 1 { "id" } else { "ids" },
                ids.join(", "),
                if reached.is_empty() {
                    "reaches nothing beyond its entry".to_owned()
                } else {
                    format!("reaches {}", reached.join(", "))
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
