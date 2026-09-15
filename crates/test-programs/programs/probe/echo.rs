//! A probe answering `maximal()` without a model call.
//!
//! A suite proves over it that every record field survives the bindings'
//! lowering and lift.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_sdk::{Context, Doc, Error, Evidence, Model, SourceAdapter, SourceKind};
use test_programs::maximal;

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Behaviour;

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
