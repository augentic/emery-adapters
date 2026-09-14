//! Refuses its input with `bad_request!` before touching the model — the
//! WIT `invalid-request` arm.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, SourceKind, bad_request};

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;
    const SOURCE: &'static str = "refusing probe";

    fn docs() -> &'static [Doc] {
        &[]
    }

    // Nothing to await.
    fn extract<P: Model>(
        _model: &P, ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Evidence, Error>> + Send {
        ready(Err(bad_request!("the probe refuses source `{}`", ctx.input.key)))
    }
}
