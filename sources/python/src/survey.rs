//! Chooses the seams of a Python source and what the code states on its own.
//!
//! An inline value is one seam mined whole. A workspace is listed under this
//! adapter's keep, every production module parsed, and its surfaces named
//! by the model in one turn over the facts the parser read ([`model`]) —
//! the manifest and its scripts, the bootstrap, every call that hands a
//! function to a package's receiver, every `def` and class under a
//! package's decorator, what the entry modules export — each anchored where
//! the code registers or declares it; the code then holds the answer to the
//! tree and derives the rest from the anchors. A tree whose modules fit
//! within the SDK's inline budget is one seam over every module, laid into
//! the turn, held to every stem its surfaces carry; a larger tree is one
//! seam per stem, over the modules the surfaces under it reach, its entry
//! first — and, where one of those imports what the resolver cannot follow
//! or loads a module by a computed name, the rest of the tree after them.
//! The data files a seam's modules name by path are laid after the modules.
//! Each seam's brief names its surfaces — each with the id its claims lead
//! with and the modules it reaches — the boundaries its modules spell, the
//! packages they import, its data files, and what could not be followed. A
//! tree the survey names no surface in is cut mechanically instead: within
//! the budget, one seam over every module under one stem — the package the
//! manifest names, else the root directory's name, else `module`; past it,
//! one seam per top-level directory beneath `src/` and the one top-level
//! package (or the root) under that directory's name, the root's own modules
//! joined to the first. Only a tree with no production module is refused.
//!
//! The tree's own tests are never modules: they are read for what they
//! state — a test's names and docstrings, a feature's scenarios — and each
//! test file follows the seams whose modules it imports (a feature file, the
//! step modules beside it), its statements listed in the brief and the file
//! itself laid after the seam's modules, so the model reads what the code
//! confirms of them and invents nothing the code does not hold.
//!
//! Beside the seams, the survey yields the `type` claims the reached modules
//! declare, copied from the code for the guest to join after the model's
//! answer.

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

/// What a source is mined by: its seams, and the claims its code states.
pub struct Survey {
    /// The seams, each one gated turn.
    pub seams: Vec<Seam>,
    /// The `type` claims of every module a seam reaches, declaration verbatim.
    pub types: Vec<Claim>,
}

/// A source read and ready for its surfaces to be decided.
pub enum Preparation {
    /// An inline value: one seam mined whole, its declarations copied unanchored.
    Value(Survey),
    /// A workspace, parsed, whose surfaces decide its seams.
    Workspace(Prepared),
}

/// A workspace parsed, with what the seams are cut from once its surfaces
/// are decided.
pub struct Prepared {
    /// The source's name, for the log.
    pub source: String,
    /// The lent root.
    pub root: String,
    /// The parsed tree.
    pub tree: Tree,
    tests: Vec<Test>,
}

/// Reads the source: an inline value is one seam at once; a workspace is
/// listed, every production module parsed, and its tests read.
pub fn prepare(input: &SourceInput) -> Result<Preparation, Error> {
    let source = &input.name;

    // pass an inline value through whole, its declarations copied unanchored
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

    // one walk: the production modules, and the data files a string may name
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

/// Cuts the seams of a prepared workspace from its `surfaces`, however they
/// were decided, and copies the `type` claims the seams' modules declare.
pub fn seams(prepared: &Prepared, surfaces: &[Surface]) -> Survey {
    let Prepared {
        source,
        root: workspace,
        tree,
        tests,
    } = prepared;

    logged(source, "surveyed", surfaces);

    // a tree small enough to lay into one turn, or of one module, cuts no
    // finer; one the code exposes no surface of is cut mechanically
    let size: usize = tree.modules.values().map(|module| module.text.len()).sum();
    let fits = tree.modules.len() == 1 || u64::try_from(size).unwrap_or(u64::MAX) <= INLINE_BYTES;
    let leads = match (surfaces.is_empty(), fits) {
        (false, true) => vec![whole(tree, surfaces)],
        (false, false) => by_stem(tree, surfaces),
        (true, true) => vec![unsurfaced(tree, workspace)],
        (true, false) => by_directory(tree, workspace),
    };
    let seams: Vec<Seam> =
        leads.into_iter().map(|lead| finish(tree, lead, surfaces, tests)).collect();

    let reached = unique(seams.iter().flat_map(|seam| seam.files.iter().map(String::as_str)));
    let types = skeleton::types(reached.iter().filter_map(|path| tree.modules.get(*path)), true);

    Survey { seams, types }
}

// One line the run's log carries at TRACE: the surfaces as `surveyed`, each
// with its name, entry, stem, and the ids its requirements lead with, as one
// JSON array.
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

// The directories that never hold a production module: dependencies and
// environments, caches, build output, coverage and documentation, tests,
// and schema history.
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
// The files beside the modules that configure a build or a test run rather
// than belong to the program; `manage.py` is kept, since it is Django's
// bootstrap.
const SKIP_FILES: &[&str] = &["setup.py", "conftest.py", "noxfile.py"];
const EXTENSIONS: &[&str] = &["py"];
const DATA_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "csv", "ini"];

// Pushes `item` unless the list holds it.
fn push_unique<T: PartialEq>(into: &mut Vec<T>, item: T) {
    if !into.contains(&item) {
        into.push(item);
    }
}

// The items once each, in first-occurrence order.
fn unique<T: PartialEq>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut list = Vec::new();
    for item in items {
        push_unique(&mut list, item);
    }
    list
}

// A directory the keep prunes: a skip directory by name, a distribution's
// `.egg-info`, or Alembic's `versions` of schema history.
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
    if SKIP_FILES.contains(&name) {
        return false;
    }
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    EXTENSIONS.contains(&extension) && !is_test_name(stem)
}

// A module named as a test: `test_*`, `*_test`, or Django's `tests`.
fn is_test_name(stem: &str) -> bool {
    stem.starts_with("test_") || stem.ends_with("_test") || stem == "tests"
}

// A data file a module may name by path — a table, a mapping, a schedule, a
// configuration — wherever a module may sit. Only one a string names is
// ever laid.
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

// The tree's own tests: a module under a test directory or named as a test,
// and a Gherkin feature file — never a stub, a fixture configuration, a
// dependency, build output, or a dot entry.
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

// Every test file read for what it states and which modules of the tree it
// imports; a feature file states its scenarios and imports what the step
// modules beside it — the parsed tests under its directory's parent — do.
// One that states nothing is dropped; one that is not UTF-8 text is left
// out with a warning.
fn tests(root: &Path, paths: Vec<String>, tree: &Tree) -> Vec<Test> {
    let mut tests: Vec<Test> = Vec::new();
    let mut features: Vec<Test> = Vec::new();
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

// The seam sealed: its anchors — where its modules' behaviour starts, and
// the lines of the surfaces under its stems; none for a tree with no
// surface, whose exports are read for what they do — then the data files its
// modules name, appended to its files after the modules so a criterion may
// cite a value in one, then the tests that follow its modules — the ones
// importing one of them, and the ones importing no module of the tree at all
// — appended after those, never among the anchors, and its brief rendered.
fn finish(tree: &Tree, lead: Lead, surfaces: &[Surface], tests: &[Test]) -> Seam {
    let Lead {
        seam:
            Seam {
                text,
                mut files,
                stems,
                ..
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
    let data = unique(
        files.iter().filter_map(|path| tree.modules.get(path)).flat_map(parse::Module::data),
    );
    files.extend(data.into_iter().map(str::to_owned));
    files.extend(attached.iter().map(|test| test.path.clone()));
    Seam {
        text,
        files,
        stems,
        anchors,
    }
}

// Every module parsed and each of its imports settled to what it names; one
// that is not UTF-8 text is left out with a warning.
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

// What opens a seam: the text that leads its brief, its modules, its stems
// — and whether the modules run past its surfaces' closures to the rest of
// the tree, which only the cut by stem decides.
struct Lead {
    seam: Seam,
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

// One seam over the whole tree: the bootstrap and what it reaches first,
// then the rest, every surface named, every stem held. This and the cuts
// below yield the leads for `finish` to seal.
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

// One seam per stem, over the modules the surfaces under it reach — and,
// where one of those imports what the resolver could not follow or loads a
// module by a computed name, the rest of the tree after them, so what the
// import names is still within the seam's reach.
fn by_stem(tree: &Tree, surfaces: &[Surface]) -> Vec<Lead> {
    unique(surfaces.iter().map(|surface| surface.stem.as_str()))
        .into_iter()
        .map(|stem| {
            let under: Vec<&Surface> = surfaces.iter().filter(|s| s.stem == stem).collect();
            let mut files =
                unique(under.iter().flat_map(|surface| surface.closure.iter().cloned()));
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

// What a seam of a tree with no surface is told in place of its surfaces.
const NO_SURFACE: &str = "No surface was found in this source: its survey named no route, command, \
                          job, consumer, or exported API — no bootstrap the manifest's scripts \
                          name or a conventional entry holds, no handler registered with a \
                          package, no function or class under a package's decorator, and no \
                          function or class exported at an entry module for a caller. Read it as \
                          a library is read — for what its exports do for a caller — and claim \
                          what the code exhibits.";

// One seam over a tree the survey names no surface in: every module, under
// the one stem the tree is named by.
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

// One seam per top-level directory of a tree the survey names no surface in
// — beneath `src/` where a module sits under it, and beneath the one
// top-level package where the tree has exactly one, else beneath the root —
// under the directory's name, the root's own modules joined to the first.
fn by_directory(tree: &Tree, root: &str) -> Vec<Lead> {
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

// The stem a tree with no surface is held to: the package the manifest
// names, else the root directory's name, else `module`.
fn fallback_stem(tree: &Tree, root: &str) -> String {
    let named = tree.manifest.name.as_deref().and_then(surface::kebab);
    named
        .or_else(|| {
            Path::new(root).file_name().and_then(|name| name.to_str()).and_then(surface::kebab)
        })
        .unwrap_or_else(|| "module".to_owned())
}

// The seam's brief: its lead, then what its modules state on their own —
// the boundaries they spell, the packages they import, the calls they make
// through them, the points where they decide — the data files they name,
// what the tests that follow them state, what the resolver could not follow
// (and that the rest of the tree follows the closure when `widened`), and
// which modules the parser could not read whole.
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

// One line per surface: its name, its entry, its stem, the notes the survey
// made of it, the ids its requirements lead with, and the modules it reaches
// beyond its entry.
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
