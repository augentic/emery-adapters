//! A `source-adapter` probe that refuses every input as `bad_request`, so the
//! suite can see a guest's typed refusal lift to its omnia class.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{AdapterMetadata, Context, Error, Evidence, SourceKind, bad_request};

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Documentation)
}

fn extract<P>(ctx: &Context<'_, P>) -> impl Future<Output = Result<Evidence, Error>> {
    ready(Err(bad_request!("the probe refuses source `{}`", ctx.input.name)))
}
