//! A probe mining two `Note` seams over a one-document corpus.
//!
//! A suite proves over it, under the runtime, that the host runs the
//! completions one guest issues together: the property the SDK's fan-out
//! rests on.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{AdapterMetadata, Context, Doc, Error, Evidence, Model, Seam, SourceKind};

const DOCS: &[Doc] = &[Doc {
    path: "prompts/extract.md",
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
    emery_sdk::mine(ctx, DOCS, &seams).await
}
