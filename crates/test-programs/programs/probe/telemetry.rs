#![cfg(target_arch = "wasm32")]

use emery_sdk::{AdapterMetadata, Context, Error, Evidence, Model, SourceKind};
use test_programs::maximal;
use tracing::Level;

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Documentation)
}

async fn extract<P: Model>(_ctx: &Context<'_, P>) -> Result<Evidence, Error> {
    traced().await;
    Ok(maximal())
}

#[tracing::instrument(level = Level::ERROR)]
async fn traced() {}
