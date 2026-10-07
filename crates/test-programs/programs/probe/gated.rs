//! A `source-adapter` probe that mines one seam through the SDK, so the suite
//! can drive the claim gate's repair rounds and its spent-rounds refusal.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{Context, Doc, Error, Evidence, Model, Seam, SourceAdapter, SourceKind};

const PROSE: &[Doc] = &[
    Doc {
        path: "extract.md",
        body: "SYSTEM",
    },
    Doc {
        path: "references/greeting.md",
        body: "Greet warmly.",
    },
];

struct Adapter;

emery_sdk::source_adapter!(Adapter);

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        emery_sdk::extract(ctx, PROSE, &[Seam::whole()]).await
    }
}
