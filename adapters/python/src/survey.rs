//! Reads a Python source into the tree the SDK surveys.
//!
//! An inline value is one seam mined whole, its declarations copied
//! unanchored, with no survey turn.
//!
//! A workspace is listed under this adapter's keep:
//!
//! - the production modules
//! - the data files a string may name
//! - the tree's own tests
//!
//! Every module is parsed, and the tree is settled through the adapter's
//! recogniser ([`surface`]) for the SDK's `seams` to survey and cut. Only a
//! tree with no production module is refused.

mod dialect;
mod parse;
mod resolve;
mod surface;

use std::path::Path;

use emery_sdk::survey::code::{Listing, Parsed};
use emery_sdk::survey::{self, Survey};
use emery_sdk::workspace::{self, Entry};
use emery_sdk::{Context, Error, Model, SourceContent, bad_request};

use self::dialect::DIALECT;
use self::parse::Module;
use self::surface::Python;
use crate::PROSE;

pub async fn survey<P: Model>(ctx: &Context<'_, P>) -> Result<Survey, Error> {
    let source = &ctx.input.name;

    // an inline value is one seam, with no survey
    let workspace = match &ctx.input.content {
        SourceContent::Value(text) => {
            let module = Module::parse("value.py", text.clone());
            return Ok(Survey::value(&DIALECT, &module));
        }
        SourceContent::Workspace(workspace) => workspace,
    };

    // one walk lists the modules and the data files a string may name
    let (modules, data): (Vec<String>, Vec<String>) =
        workspace::list(workspace, |entry| is_module(entry) || is_data(entry))?
            .into_iter()
            .partition(|path| Path::new(path).extension().is_some_and(|ext| ext == "py"));
    if modules.is_empty() {
        return Err(bad_request!(
            "`{source}` has no production module: no Python source outside tests, stubs, \
             virtualenvs, caches, build output, and migrations."
        ));
    }

    // the settled tree is the model's to survey
    let listing = Listing {
        modules,
        data,
        tests: workspace::list(workspace, is_test)?,
    };
    let parsed = Parsed::read(Path::new(workspace), &DIALECT, listing, Module::parse);
    let recogniser = Python::new(&parsed);
    let tree = parsed.settle(recogniser);
    survey::seams(ctx, PROSE, &tree).await
}

// Build and test configuration, not the program. `manage.py` stays: it is
// Django's bootstrap. `setup.py` is the packaging script at the root alone;
// beneath a package it is a module like any other.
const SKIP_FILES: &[&str] = &["conftest.py", "noxfile.py"];
const SKIP_ROOT_FILES: &[&str] = &["setup.py"];
const EXTENSIONS: &[&str] = &["py"];
const DATA_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "csv", "ini"];
const TEST_DIRS: &[&str] = &["test", "tests", "testing", "__tests__", "features"];
// Never a production module's or a test's: dependencies, environments,
// caches, build output, coverage, documentation, and schema history.
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

// Alembic's `versions` is schema history, not a package.
fn never_dir(entry: Entry<'_>) -> bool {
    let name = entry.name();
    NEVER_DIRS.contains(&name)
        || name.ends_with(".egg-info")
        || (name == "versions" && entry.path().ends_with("alembic/versions"))
}

fn skipped_dir(entry: Entry<'_>) -> bool {
    never_dir(entry) || TEST_DIRS.contains(&entry.name())
}

fn is_module(entry: Entry<'_>) -> bool {
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

fn is_test(entry: Entry<'_>) -> bool {
    if entry.hidden() {
        return false;
    }
    if let Entry::Dir(_) = entry {
        return !never_dir(entry);
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
