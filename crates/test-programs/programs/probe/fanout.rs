//! Exercises concurrent mining across two note seams.
//!
//! Both model requests must be pending together, which verifies the host
//! behaviour required by the SDK's bounded fan-out.

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

// Two seams whatever the input arm, so `mine` holds two completions pending
// at once over a workspace and over a value; no survey turn is spent
// choosing them.
async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
    let seams = [
        Seam::Note("The first half of the source.".to_owned()),
        Seam::Note("The second half of the source.".to_owned()),
    ];
    emery_sdk::extract(ctx, PROSE, &seams).await
}
