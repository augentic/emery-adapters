//! Probe adapter that emits a tracing span during extraction.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, SourceKind};
use test_programs::maximal;
use tracing::Level;

struct Adapter;

emery_sdk::export_source!(Adapter);

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    async fn extract<P: Model>(_ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        traced().await;
        Ok(maximal())
    }
}

#[tracing::instrument(level = Level::ERROR)]
async fn traced() {}
