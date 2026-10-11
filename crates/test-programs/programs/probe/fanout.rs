//! A `source-adapter` probe that mines two seams at once, so the suite can
//! hold the host to keeping one guest's completions pending together.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{Context, Doc, Error, Evidence, Model, Seam, SourceAdapter, SourceKind};

const PROSE: &[Doc] = &[Doc {
    path: "extract.md",
    body: "SYSTEM",
}];

struct Adapter;

emery_sdk::export_source!(Adapter);

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        let seams = [
            Seam::note("The first half of the source."),
            Seam::note("The second half of the source."),
        ];
        emery_sdk::extract(ctx, PROSE, &seams).await
    }
}
