//! A source adapter with nothing of its own — an inline two-document corpus
//! and `Material::Bound` — so a suite can prove what the SDK does for every
//! adapter under the runtime: the request it opens over the host model, the
//! reference tools answered from the embedded corpus, the lend following the
//! input, and the claim gate refusing a candidate until the backend's budget
//! is spent — without riding a shipped adapter's prompt. Stands in for the
//! adapter under test when the SDK's side of the seam is the subject.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Material, Model, SourceAdapter};

/// The corpus: the extraction prompt and one reference, sorted by path as
/// the embed walker would emit them.
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
    const SOURCE: &'static str = "gated probe";

    fn docs() -> &'static [Doc] {
        DOCS
    }

    async fn extract<P: Model>(model: &P, ctx: &Context<'_>) -> Result<Evidence, Error> {
        Self::evidence(model, ctx, Material::Bound).await
    }
}
