//! `engine` — the first-party adapters linked into the shared
//! native harness, plus the repository's trial and scenario data
//! locators.

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
pub const TRIAL_ENV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/change/trial.env");

/// The shared seed tree both rungs copy into their sandbox.
pub const SEED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/change/seed");

/// The committed prompt-scenario root (`eval/scenarios/`).
pub const SCENARIOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../scenarios");

/// The sandbox root: the trial project, and the parent of scenario
/// scratch trees (`sandbox/<adapter>/<name>/run-…`).
pub const SANDBOX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sandbox");
