//! Reads a TypeScript / JavaScript source into the tree the SDK surveys.
//!
//! An inline value is one seam mined whole, its declarations copied
//! unanchored, with no survey turn. A workspace is listed under this
//! adapter's keep — the production modules, the `.json` files an import may
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
use self::surface::TypeScript;

mod parse;
mod resolve;
mod surface;

pub enum Preparation {
    // An inline value needs no survey turn: one seam, its declarations
    // unanchored.
    Value(Survey),
    Workspace(Tree<TypeScript>),
}

// Refuses a workspace with no production module, and nothing else.
pub fn prepare(input: &SourceInput) -> Result<Preparation, Error> {
    let source = &input.name;

    // an inline value is one seam, with no survey
    let workspace = match &input.content {
        SourceContent::Value(text) => {
            let module = Module::parse("value.ts", text.clone());
            return Ok(Preparation::Value(Survey {
                seams: vec![Seam::whole()],
                types: emery_sdk::survey::types(&DIALECT, [&*module], false),
            }));
        }
        SourceContent::Workspace(workspace) => workspace,
    };

    // one walk lists the modules and the data files an import may name
    let (data, modules): (Vec<String>, Vec<String>) =
        emery_sdk::workspace::list(workspace, |entry| include(entry) || is_data(entry))?
            .into_iter()
            .partition(|path| Path::new(path).extension().is_some_and(|ext| ext == "json"));
    if modules.is_empty() {
        return Err(bad_request!(
            "`{source}` has no production module: no TypeScript or JavaScript source outside \
             tests, declarations, dependencies, and build output."
        ));
    }
    let tests = emery_sdk::workspace::list(workspace, is_test)?;

    let root = Path::new(workspace);
    let listing = Listing {
        modules,
        data,
        tests: tests.clone(),
    };
    let parsed = Parsed::read(root, &DIALECT, listing, Module::parse);
    let recogniser = TypeScript::new(root, &parsed, tests);
    Ok(Preparation::Workspace(parsed.settle(recogniser)))
}

// What the SDK's lookups read of TypeScript and JavaScript.
static DIALECT: Dialect = Dialect {
    self_name: "this",
    generic_stems: &["index", "main"],
    structural: &[
        "use",
        "register",
        "mount",
        "plugin",
        "decorate",
        "addHook",
        "hook",
        "listen",
        "connect",
        "disconnect",
        "close",
        "end",
        "then",
        "catch",
        "finally",
        "start",
        "stop",
        "init",
        "initialize",
        "forEach",
        "map",
        "filter",
        "reduce",
        "find",
        "findIndex",
        "some",
        "every",
        "sort",
        "flatMap",
        "Promise",
        "setTimeout",
        "setInterval",
        "setImmediate",
        "nextTick",
        "queueMicrotask",
        "describe",
        "it",
        "test",
        "beforeEach",
        "afterEach",
        "beforeAll",
        "afterAll",
    ],
    listeners: &["on", "once", "addListener", "addEventListener", "prependListener"],
    lifecycle_events: &[
        "error",
        "close",
        "connect",
        "connecting",
        "disconnect",
        "end",
        "ready",
        "open",
        "listening",
        "exit",
        "warning",
        "drain",
        "finish",
        "timeout",
        "reconnecting",
        "SIGINT",
        "SIGTERM",
        "SIGHUP",
        "unhandledRejection",
        "uncaughtException",
        "beforeExit",
    ],
    mocking: &[],
    lifecycle: &[],
    decorator_noise: &[
        "UseGuards",
        "UseInterceptors",
        "UsePipes",
        "UseFilters",
        "HttpCode",
        "Header",
        "Redirect",
        "Bind",
        "SetMetadata",
        "Roles",
        "Public",
        "Version",
        "Transactional",
        "Injectable",
        "Inject",
        "SerializeOptions",
    ],
    decorator_noise_prefixes: &["Api"],
    decorator_hooks: &[],
    hook_keywords: &[],
    type_imports_reach: true,
    openers: &['[', '{'],
    comment_prefixes: &["//", "*"],
    globals: &["fetch"],
    options: &[],
    manifests: &["package.json"],
    barrels: &["index"],
    constructs: Some("new"),
    env_object: Some("process.env"),
    enum_bases: &[],
    class_syntax: ClassSyntax::BRACED,
    described: "a case's title under its suites'",
    // A parameter is `:id`; a pattern spells a wildcard, a regex group, or a
    // bracketed segment.
    route: Spelling {
        pattern: &['*', '{', '(', '['],
        param_name: |segment| segment.trim_start_matches(':'),
    },
};

// Never a production module's: dependencies, build output, and tests.
const SKIP_DIRS: &[&str] =
    &["node_modules", "vendor", "target", "dist", "build", "test", "tests", "__tests__"];
// A declaration's or a test's file, by the infix before its extension.
const SKIP_INFIXES: &[&str] = &["d", "spec", "test"];
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

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

fn include(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !SKIP_DIRS.contains(&entry.name());
    }
    let Some((stem, extension)) = entry.name().rsplit_once('.') else {
        return false;
    };
    if !EXTENSIONS.contains(&extension) {
        return false;
    }

    !matches!(stem.rsplit_once('.'), Some((_, infix)) if SKIP_INFIXES.contains(&infix))
}

// A `.json` a module may import by path, wherever a module may sit. Only one
// an import names is ever laid.
fn is_data(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !SKIP_DIRS.contains(&entry.name());
    }
    entry.extension() == Some("json")
}

const TEST_DIRS: &[&str] = &["test", "tests", "__tests__"];
const TEST_INFIXES: &[&str] = &["spec", "test"];
// Never a test's either: dependencies and build output.
const NEVER_DIRS: &[&str] = &["node_modules", "vendor", "target", "dist", "build"];

fn is_test(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !NEVER_DIRS.contains(&entry.name());
    }
    let Some((stem, extension)) = entry.name().rsplit_once('.') else {
        return false;
    };
    if extension == "feature" {
        return true;
    }
    if !EXTENSIONS.contains(&extension) {
        return false;
    }
    let infix = stem.rsplit_once('.').map(|(_, infix)| infix);
    if infix == Some("d") {
        return false;
    }
    infix.is_some_and(|infix| TEST_INFIXES.contains(&infix))
        || entry.path().split('/').rev().skip(1).any(|dir| TEST_DIRS.contains(&dir))
}
