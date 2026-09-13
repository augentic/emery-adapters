//! Fails with `bad_gateway!` before touching the model — the WIT `internal`
//! arm every class but a refusal shares.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, bad_gateway};

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const SOURCE: &'static str = "upstream probe";

    fn docs() -> &'static [Doc] {
        &[]
    }

    // Nothing to await.
    fn extract<P: Model>(
        _model: &P, ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Evidence, Error>> + Send {
        ready(Err(bad_gateway!("the probe's upstream failed for source `{}`", ctx.input.key)))
    }
}
