//! A probe answering `maximal()` without a model call.
//!
//! A suite proves over it that every record field survives the bindings'
//! lowering and lift.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::SourceKind;
use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
use test_programs::maximal;

struct Adapter;
export::export!(Adapter with_types_in export);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Behaviour)
    }

    fn extract(_id: AdapterId, _input: Input) -> impl Future<Output = Result<Evidence, Error>> {
        ready(Ok(maximal().into()))
    }
}
