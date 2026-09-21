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
    let seams = [
        Seam::Note("The first half of the source.".to_owned()),
        Seam::Note("The second half of the source.".to_owned()),
    ];
    emery_sdk::extract(ctx, PROSE, &seams).await
}
