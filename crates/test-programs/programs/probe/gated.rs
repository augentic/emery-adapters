//! An adapter with nothing of its own — an inline two-document corpus over
//! `Material::Bound` — so a suite can prove what the SDK does for every
//! adapter under the runtime without riding a shipped prompt.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Material, Model, SourceAdapter, SourceKind};

/// Sorted by path, as the walker emits them.
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

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;
    const SOURCE: &'static str = "gated probe";

    fn docs() -> &'static [Doc] {
        DOCS
    }

    async fn extract<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Evidence, Error> {
        Self::evidence(model, ctx, Material::Bound).await
    }
}
