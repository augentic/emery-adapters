//! A probe refusing its input with `bad_request!` before touching the model.
//!
//! Its failure crosses the WIT `invalid-request` arm.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{AdapterMetadata, Context, Error, Evidence, SourceKind, bad_request};

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Documentation)
}

fn extract(ctx: &Context<'_>) -> impl Future<Output = Result<Evidence, Error>> {
    ready(Err(bad_request!("the probe refuses source `{}`", ctx.input.key)))
}
