//! A probe failing with `bad_gateway!` before touching the model.
//!
//! Its failure crosses the WIT `internal` arm, which every class but a
//! refusal shares.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{AdapterMetadata, Context, Error, Evidence, SourceKind, bad_gateway};

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Documentation)
}

fn extract(ctx: &Context<'_>) -> impl Future<Output = Result<Evidence, Error>> {
    ready(Err(bad_gateway!("the probe's upstream failed for source `{}`", ctx.input.key)))
}
