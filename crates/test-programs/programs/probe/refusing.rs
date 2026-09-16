//! A probe refusing its input with `bad_request!` before touching the model.
//!
//! Its failure crosses the WIT `invalid-request` arm.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
use emery_sdk::{SourceKind, bad_request};

struct Adapter;
export::export!(Adapter with_types_in export);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Documentation)
    }

    fn extract(_id: AdapterId, input: Input) -> impl Future<Output = Result<Evidence, Error>> {
        ready(Err(bad_request!("the probe refuses source `{}`", input.key).into()))
    }
}
