//! A probe failing with `bad_gateway!` before touching the model.
//!
//! Its failure crosses the WIT `internal` arm, which every class but a
//! refusal shares.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
use emery_sdk::{SourceKind, bad_gateway};

struct Adapter;
export::export!(Adapter with_types_in export);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        export::metadata(SourceKind::Documentation)
    }

    fn extract(_id: AdapterId, input: Input) -> impl Future<Output = Result<Evidence, Error>> {
        ready(Err(bad_gateway!("the probe's upstream failed for source `{}`", input.key).into()))
    }
}
