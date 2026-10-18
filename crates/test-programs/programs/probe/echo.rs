//! Probe adapter returning maximal static evidence without a model turn.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{Context, Error, Evidence, Metadata, SourceAdapter, SourceKind};
use test_programs::maximal;

struct Adapter;

emery_sdk::export_source!(Adapter);

impl SourceAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            kind: SourceKind::Behaviour,
        }
    }

    fn extract<P>(_ctx: &Context<'_, P>) -> impl Future<Output = Result<Evidence, Error>> {
        ready(Ok(maximal()))
    }
}
