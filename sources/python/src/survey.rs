//! Reads a Python source into the tree the SDK surveys.
//!
//! An inline value is one seam mined whole, its declarations copied
//! unanchored, with no survey turn. A workspace is listed under this
//! adapter's keep — the production modules, the data files a string may
//! name, and the tree's own tests — every module parsed, and the tree
//! settled through the adapter's recogniser ([`surface`]) for the SDK's
//! `seams` to survey and cut. Only a tree with no production module is
//! refused.

use std::path::Path;

use emery_sdk::survey::code::{Listing, Parsed, Tree};
use emery_sdk::survey::route::Spelling;
use emery_sdk::survey::{ClassSyntax, Dialect, Survey};
use emery_sdk::workspace::Entry;
use emery_sdk::{Error, Seam, SourceContent, SourceInput, bad_request};

use self::parse::Module;
use self::surface::Python;

mod parse;
mod resolve;
mod surface;

pub enum Preparation {
    // An inline value needs no survey turn: one seam, its declarations
    // unanchored.
    Value(Survey),
    Workspace(Tree<Python>),
}

// Refuses a workspace with no production module, and nothing else.
pub fn prepare(input: &SourceInput) -> Result<Preparation, Error> {
    let source = &input.name;

    // an inline value is one seam, with no survey
    let workspace = match &input.content {
        SourceContent::Value(text) => {
            let module = Module::parse("value.py", text.clone());
            return Ok(Preparation::Value(Survey {
                seams: vec![Seam::whole()],
                types: emery_sdk::survey::types(&DIALECT, [&*module], false),
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
    let tests = emery_sdk::workspace::list(workspace, is_test)?;

    let root = Path::new(workspace);
    let listing = Listing { modules, data, tests };
    let parsed = Parsed::read(root, &DIALECT, listing, Module::parse);
    let recogniser = Python::new(root, &parsed);
    Ok(Preparation::Workspace(parsed.settle(recogniser)))
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
    options: &["add_argument", "add_option", "option", "argument", "Option", "Argument"],
    manifests: &["pyproject.toml", "setup.cfg"],
    barrels: &["__init__"],
    constructs: None,
    env_object: None,
    enum_bases: &["Enum", "Flag"],
    class_syntax: ClassSyntax::INDENTED,
    described: "a test's docstring or name under its class's",
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
