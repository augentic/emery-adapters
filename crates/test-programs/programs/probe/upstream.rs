//! Probe adapter that fails every input with `bad_gateway`.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{Context, Error, Evidence, SourceAdapter, SourceKind, bad_gateway};

struct Adapter;

emery_sdk::export_source!(Adapter);

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    fn extract<P>(ctx: &Context<'_, P>) -> impl Future<Output = Result<Evidence, Error>> {
        ready(Err(bad_gateway!("the probe's upstream failed for source `{}`", ctx.input.name)))
    }
}
