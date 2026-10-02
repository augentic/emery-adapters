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

use std::path::Path;

use emery_sdk::survey::Dialect;
use emery_sdk::survey::route::Spelling;
use emery_sdk::survey::tests::{Test, scenarios, stated};
use emery_sdk::workspace::Entry;
use emery_sdk::{Claim, Error, INLINE_BYTES, Seam, SourceContent, SourceInput, bad_request, kebab};

use self::parse::Module;
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
            let module = Module::parse("value.py", text.clone());
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
    let tree = Tree::read(root, modules, data);
    let tests = tests(root, emery_sdk::workspace::list(workspace, is_test)?, &tree);

    Ok(Preparation::Workspace(Prepared {
        source: source.clone(),
        root: workspace.clone(),
        tree,
        tests,
    }))
}

pub fn seams(prepared: &Prepared, surfaces: &[Surface]) -> Survey {
    let tree = &prepared.tree;
    logged(&prepared.source, "surveyed", surfaces);

    // choose the cut by size and by whether any surface was named
    let size: usize = tree.modules.values().map(|module| module.text.len()).sum();
    let fits = tree.modules.len() == 1 || u64::try_from(size).unwrap_or(u64::MAX) <= INLINE_BYTES;
    let leads = match (surfaces.is_empty(), fits) {
        (false, true) => vec![Lead::whole(tree, surfaces)],
        (false, false) => Lead::by_stem(tree, surfaces),
        (true, true) => vec![Lead::unsurfaced(prepared)],
        (true, false) => Lead::by_directory(prepared),
    };
    let seams: Vec<Seam> = leads.into_iter().map(|lead| lead.finish(prepared, surfaces)).collect();

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
                "lines": surface.lines.anchor(),
                "ids": surface.ids,
            })
        })
        .collect();
    let json = emery_sdk::serde_json::Value::Array(listed).to_string();
    emery_sdk::tracing::trace!(%source, surfaces = %json, "{what}");
}

// What the SDK's lookups read of Python.
static DIALECT: Dialect = Dialect {
    self_name: "self",
    generic_stems: &[
        "__init__",
        "__main__",
        "main",
        "app",
        "views",
        "urls",
        "routes",
        "api",
        "handlers",
        "endpoints",
        "tasks",
    ],
    structural: &[
        "map",
        "filter",
        "sorted",
        "sort",
        "reduce",
        "partial",
        "wraps",
        "run",
        "run_until_complete",
        "create_task",
        "gather",
        "ensure_future",
        "run_in_executor",
        "to_thread",
        "register_blueprint",
        "include_router",
        "add_middleware",
        "mount",
        "setdefault",
        "Depends",
        "Security",
        "raises",
        "fixture",
        "parametrize",
        "field",
        "Field",
        "Column",
        "mapped_column",
        "relationship",
    ],
    listeners: &[],
    lifecycle_events: &[],
    // `patch` by its spelling whole: the same name on an application, a
    // router, or an HTTP client is a verb.
    mocking: &["patch", "mock.patch", "unittest.mock.patch", "mocker.patch"],
    // Django's admin site among them: it serves what it registers on the
    // source's behalf, by call or by decorator.
    lifecycle: &[
        "signal.signal",
        "atexit.register",
        "add_signal_handler",
        "on_event",
        "add_event_handler",
        "add_exception_handler",
        "register_error_handler",
        "teardown_appcontext",
        "teardown_request",
        "lifespan",
        "site.register",
        "admin.register",
        "admin.action",
        "admin.display",
    ],
    decorator_noise: &[
        "dataclass",
        "define",
        "frozen",
        "property",
        "setter",
        "getter",
        "deleter",
        "staticmethod",
        "classmethod",
        "cached_property",
        "lru_cache",
        "cache",
        "wraps",
        "overload",
        "abstractmethod",
        "override",
        "final",
        "contextmanager",
        "asynccontextmanager",
        "login_required",
        "permission_required",
        "csrf_exempt",
        "require_http_methods",
        "require_POST",
        "require_GET",
        "atomic",
        "retry",
        "validator",
        "field_validator",
        "model_validator",
        "root_validator",
        "computed_field",
        "total_ordering",
        "unique",
        "fixture",
        "parametrize",
        "mark",
        "skip",
        "skipif",
    ],
    decorator_noise_prefixes: &[],
    decorator_hooks: &[
        "connect",
        "receiver",
        "listens_for",
        "exception_handler",
        "errorhandler",
        "error_handler",
        "before_request",
        "after_request",
        "before_first_request",
        "middleware",
        "context_processor",
        "template_filter",
        "url_value_preprocessor",
        "url_defaults",
    ],
    hook_keywords: &[
        "lifespan",
        "on_startup",
        "on_shutdown",
        "exception_handlers",
        "default_factory",
        "default",
        "key",
        "callback",
        "dependencies",
        "middleware",
        "result_callback",
    ],
    type_imports_reach: false,
    openers: &['[', '{', '('],
    comment_prefixes: &["#"],
    globals: &["open"],
    // A pattern spells a wildcard, a brace, a `<converter>`, or a regex.
    route: Spelling {
        pattern: &['*', '{', '(', '[', '<', '?', '\\'],
        param_name: surface::param_name,
    },
};

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

// Each key's values together, the keys in first-occurrence order.
fn grouped<K: PartialEq, V>(items: impl IntoIterator<Item = (K, V)>) -> Vec<(K, Vec<V>)> {
    let mut groups: Vec<(K, Vec<V>)> = Vec::new();
    for (key, value) in items {
        match groups.iter_mut().find(|(known, _)| *known == key) {
            Some((_, values)) => values.push(value),
            None => groups.push((key, vec![value])),
        }
    }
    groups
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
                statements: scenarios(&text),
            });
            continue;
        }
        let mut module = Module::parse(&path, text);
        tree.resolver.settle(&mut module);
        tests.push(Test {
            path,
            imports: module.reached(&DIALECT).into_iter().map(str::to_owned).collect(),
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

// A data file within this many bytes is laid directly after the first module
// naming it, a larger one after every module. Laid after a long module such
// as a generated table, a small file could fall past where the laid run
// ends, and a `criterion` could not cite its value.
const DATA_BESIDE_BYTES: u64 = 8 * 1024;

impl Prepared {
    fn fallback_stem(&self) -> String {
        let named = self.tree.manifest.name.as_deref().and_then(kebab);
        named
            .or_else(|| {
                Path::new(&self.root).file_name().and_then(|name| name.to_str()).and_then(kebab)
            })
            .unwrap_or_else(|| "module".to_owned())
    }

    fn with_data(&self, modules: Vec<String>) -> Vec<String> {
        let mut files: Vec<String> = Vec::with_capacity(modules.len());
        let mut large: Vec<String> = Vec::new();
        for path in modules {
            let named =
                self.tree.modules.get(&path).map(|module| module.data()).unwrap_or_default();
            files.push(path);
            for data in named {
                if files.iter().chain(&large).any(|file| file == data) {
                    continue;
                }
                let size = std::fs::metadata(Path::new(&self.root).join(data))
                    .map_or(u64::MAX, |m| m.len());
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
}

const NO_SURFACE: &str = "No surface was found in this source: its survey named no route, command, \
                          job, consumer, or exported API — no bootstrap the manifest names or a \
                          conventional entry holds, no handler registered with a package, \
                          no function or class under a package's decorator, \
                          and no function or class exported at an entry module for a caller. \
                          Read it as a library is read — for what its exports do for a caller — \
                          and claim what the code exhibits.";

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

    // Every module, the surfaces' closures first.
    fn whole(tree: &Tree, surfaces: &[Surface]) -> Self {
        let files = unique(
            surfaces
                .iter()
                .flat_map(|surface| surface.closure.iter().cloned())
                .chain(tree.modules.keys().cloned()),
        );
        let stems = unique(surfaces.iter().map(|surface| surface.stem.clone()));
        let text = format!(
            "The surfaces of this source, found by reading its code — where control enters it \
             from outside the process:\n\n{}\n\nEvery `requirement` and `criterion` belongs to \
             one of these surfaces: lead its id with that surface's id, and claim a behaviour \
             under the surface whose caller observes it, once.",
            listed(surfaces)
        );
        Self::new(text, files, stems)
    }

    fn by_stem(tree: &Tree, surfaces: &[Surface]) -> Vec<Self> {
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
                    "This call mines the {count} under the stem `{stem}` alone:\n\n{}\n\nThe \
                     files below are what {reach} from {whose} entry, the entry first. What the \
                     tree does for another surface is that surface's call to claim, even in a \
                     module the two share.",
                    listed(under.iter().copied()),
                );
                Self {
                    widened,
                    ..Self::new(text, files, vec![stem.to_owned()])
                }
            })
            .collect()
    }

    fn unsurfaced(prepared: &Prepared) -> Self {
        let files: Vec<String> = prepared.tree.modules.keys().cloned().collect();
        let stem = prepared.fallback_stem();
        let text = format!(
            "{NO_SURFACE} The modules below are the whole source, mined under the one stem \
             `{stem}`: lead every `requirement` and `criterion` id with it, and name each \
             behaviour for the export that exhibits it."
        );
        Self::new(text, files, vec![stem])
    }

    fn by_directory(prepared: &Prepared) -> Vec<Self> {
        let tree = &prepared.tree;

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
            let stem = kebab(name).unwrap_or_else(|| prepared.fallback_stem());
            match groups.iter_mut().find(|(s, ..)| *s == stem) {
                Some((_, _, files)) => files.push(path.clone()),
                None => groups.push((stem, format!("`{dir}/`"), vec![path.clone()])),
            }
        }

        // the root's own modules join the first group
        if groups.is_empty() {
            groups.push((prepared.fallback_stem(), "the root".to_owned(), Vec::new()));
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
                    "{NO_SURFACE} It is past the budget of one call and cut by directory: this \
                     call mines the {} under {dir} alone, under the stem `{stem}` — lead every \
                     `requirement` and `criterion` id with it. What another directory's modules \
                     do is another call's to claim, even where these import them.",
                    if files.len() == 1 {
                        "module".to_owned()
                    } else {
                        format!("{} modules", files.len())
                    },
                );
                Self::new(text, files, vec![stem])
            })
            .collect()
    }

    // A tree with no surface has no anchors, so its exports are read for what
    // they do. Neither a data file nor a test is an anchor. A test importing
    // no module of the tree follows every seam.
    fn finish(self, prepared: &Prepared, surfaces: &[Surface]) -> Seam {
        let tree = &prepared.tree;
        let files = &self.seam.files;
        let under: Vec<&Surface> =
            surfaces.iter().filter(|surface| self.seam.stems.contains(&surface.stem)).collect();
        let anchors = if surfaces.is_empty() {
            Vec::new()
        } else {
            skeleton::anchors(tree, files, under.iter().copied())
        };
        let attached: Vec<&Test> = prepared
            .tests
            .iter()
            .filter(|test| {
                test.imports.is_empty() || test.imports.iter().any(|m| files.contains(m))
            })
            .collect();
        let text = self.brief(tree, &attached);
        let Seam { files, stems, .. } = self.seam;
        let mut files = prepared.with_data(files);
        files.extend(attached.iter().map(|test| test.path.clone()));
        Seam {
            text,
            files,
            stems,
            anchors,
        }
    }

    fn brief(&self, tree: &Tree, tests: &[&Test]) -> String {
        let files = &self.seam.files;
        let modules = || files.iter().filter_map(|path| tree.modules.get(path));
        let mut sections = vec![self.seam.text.clone()];
        sections.extend(skeleton::boundaries(modules()));
        sections.extend(skeleton::packages(modules()));
        sections.extend(skeleton::calls(tree, files));
        sections.extend(skeleton::decisions(modules()));
        sections.extend(skeleton::data(modules()));
        sections.extend(stated(tests.iter().copied(), skeleton::DESCRIBED));
        sections.extend(skeleton::unfollowed(modules(), self.widened));
        let unparsed: Vec<String> =
            modules().filter(|m| !m.parsed).map(|m| format!("`{}`", m.path)).collect();
        if !unparsed.is_empty() {
            sections.push(format!(
                "The parser could not read {} whole; what {} declares is not in the lists above \
                 and is read from the text alone.",
                unparsed.join(", "),
                if unparsed.len() == 1 { "it" } else { "each" }
            ));
        }
        sections.join("\n\n")
    }
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
