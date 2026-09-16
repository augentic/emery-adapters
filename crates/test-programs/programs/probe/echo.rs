//! A probe answering `maximal()` without a model call.
//!
//! A suite proves over it that every record field survives the bindings'
//! lowering and lift.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{AdapterMetadata, Context, Error, Evidence, SourceKind};
use test_programs::maximal;

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Behaviour)
}

fn extract(_ctx: &Context<'_>) -> impl Future<Output = Result<Evidence, Error>> {
    ready(Ok(maximal()))
}
