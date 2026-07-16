//! `engine` — the first-party adapters linked into the shared
//! native harness, plus the repository's trial and scenario data
//! locators.

use std::path::{Path, PathBuf};

use captures::Captures;
use contracts::Contracts;
use documentation::Documentation;
use harness::catalog::{Binding, Catalog};
use intent::Intent;
use omnia_guest::Model;
use omnia_target::Omnia;
use screenshots::Screenshots;
use typescript::Typescript;
use vectis::Vectis;

/// Every first-party adapter linked into `engine`.
///
/// One builder call per adapter: the harness monomorphizes each
/// implementor's operation legs behind compile-checked trait bounds, so
/// adding an adapter is one line here plus its Cargo dependency.
#[must_use]
pub fn catalog<M: Model>() -> Catalog<M> {
    Catalog::builder()
        .source::<Captures>()
        .target::<Contracts>()
        .source::<Documentation>()
        .source::<Intent>()
        .target::<Omnia>()
        .source::<Screenshots>()
        .source::<Typescript>()
        .target::<Vectis>()
        .build()
}

/// The first-party binding handed to the shared harness entrypoints.
#[derive(Clone, Copy, Debug)]
pub struct FirstParty;

impl Binding for FirstParty {
    fn catalog<M: Model>() -> Catalog<M> {
        catalog()
    }
}

/// The checked-in shared trial definition (`examples/change/trial.env`).
#[must_use]
pub fn trial_env() -> PathBuf {
    examples_change().join("trial.env")
}

/// The shared seed tree both rungs copy into their sandbox.
#[must_use]
pub fn seed_dir() -> PathBuf {
    examples_change().join("seed")
}

/// The committed prompt-scenario root (`eval/scenarios/`).
#[must_use]
pub fn scenarios_dir() -> PathBuf {
    manifest().join("../scenarios")
}

/// The persistent trial sandbox project root (`sandbox/eval`).
#[must_use]
pub fn eval_sandbox() -> PathBuf {
    repo_root().join("sandbox/eval")
}

/// The scenario scratch base (`sandbox/scenarios`).
#[must_use]
pub fn scenarios_sandbox() -> PathBuf {
    repo_root().join("sandbox/scenarios")
}

fn examples_change() -> PathBuf {
    repo_root().join("examples/change")
}

fn repo_root() -> PathBuf {
    manifest().join("../..")
}

fn manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}
