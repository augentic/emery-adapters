//! Probe adapter that emits a tracing span during extraction.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{Context, Error, Evidence, Metadata, SourceAdapter, SourceKind};
use test_programs::maximal;
use tracing::Level;

struct Adapter;

impl SourceAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            kind: SourceKind::Documentation,
        }
    }

    fn extract<P>(_ctx: &Context<'_, P>) -> impl Future<Output = Result<Evidence, Error>> {
        traced();
        ready(Ok(maximal()))
    }
}

#[tracing::instrument(level = Level::ERROR)]
fn traced() {}

emery_sdk::export_source!(Adapter);
