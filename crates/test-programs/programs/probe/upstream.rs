//! A `source-adapter` probe that fails every input as `bad_gateway`, so the
//! suite can see an upstream failure lift to its omnia class.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{AdapterMetadata, Context, Error, Evidence, SourceKind, bad_gateway};

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Documentation)
}

fn extract<P>(ctx: &Context<'_, P>) -> impl Future<Output = Result<Evidence, Error>> {
    ready(Err(bad_gateway!("the probe's upstream failed for source `{}`", ctx.input.name)))
}
