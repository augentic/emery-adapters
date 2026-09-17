//! Exercises round-trip conversion of a fully populated evidence document.
//!
//! The probe returns `maximal()` without a model request, allowing every field
//! to be checked after conversion through the component interface.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{AdapterMetadata, Context, Error, Evidence, SourceKind};
use test_programs::maximal;

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Behaviour)
}

fn extract<P>(_ctx: &Context<'_, P>) -> impl Future<Output = Result<Evidence, Error>> {
    ready(Ok(maximal()))
}
