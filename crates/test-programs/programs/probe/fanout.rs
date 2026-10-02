//! A `source-adapter` probe that mines two seams at once, so the suite can
//! hold the host to keeping one guest's completions pending together.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{AdapterMetadata, Context, Doc, Error, Evidence, Model, Seam, SourceKind};

const PROSE: &[Doc] = &[Doc {
    path: "extract.md",
    body: "SYSTEM",
}];

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Documentation)
}

async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
    let seams =
        [Seam::note("The first half of the source."), Seam::note("The second half of the source.")];
    emery_sdk::extract(ctx, PROSE, &seams).await
}
