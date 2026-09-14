//! Answers `maximal()` without a model call, so a suite can prove every
//! record field survives the bindings' lowering and lift.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, SourceKind};
use test_programs::maximal;

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Behaviour;
    const SOURCE: &'static str = "echo probe";

    fn docs() -> &'static [Doc] {
        &[]
    }

    // Nothing to await.
    fn extract<P: Model>(
        _model: &P, _ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Evidence, Error>> + Send {
        ready(Ok(maximal()))
    }
}
