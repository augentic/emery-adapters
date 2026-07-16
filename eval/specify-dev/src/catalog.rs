//! The first-party adapter crates linked into the native shim.
//!
//! One builder call per adapter: the harness monomorphizes each
//! implementor's operation legs behind compile-checked trait bounds, so
//! adding an adapter is one line here plus its Cargo dependency.

use captures::Captures;
use contracts::Contracts;
use documentation::Documentation;
use harness::catalog::Catalog;
use intent::Intent;
use omnia_guest::Model;
use omnia_target::Omnia;
use screenshots::Screenshots;
use typescript::Typescript;
use vectis::Vectis;

/// Every first-party adapter linked into `specify-dev`.
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
