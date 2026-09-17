//! Exercises SDK mining and claim validation through a component.
//!
//! The probe mines a two-document corpus as one seam, allowing shared SDK
//! behaviour to be tested independently of a shipped adapter prompt.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{AdapterMetadata, Context, Doc, Error, Evidence, Model, Seam, SourceKind};

const PROSE: &[Doc] = &[
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
    emery_sdk::extract(ctx, PROSE, &[Seam::Whole]).await
}
