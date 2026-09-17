//! A probe with nothing of its own: a two-document corpus mined whole.
//!
//! A suite proves over it, under the runtime, what the SDK does for every
//! adapter, without riding a shipped prompt.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{AdapterMetadata, Context, Doc, Error, Evidence, Model, Seam, SourceKind};

const DOCS: &[Doc] = &[
    Doc {
        path: "prompts/extract.md",
        body: "SYSTEM",
    },
    Doc {
        path: "references/greeting.md",
        body: "Greet warmly.",
    },
];

emery_sdk::source_adapter!(metadata, extract);

fn metadata() -> AdapterMetadata {
    emery_sdk::metadata(SourceKind::Documentation)
}

async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
    emery_sdk::mine(ctx, DOCS, &[Seam::Whole]).await
}
