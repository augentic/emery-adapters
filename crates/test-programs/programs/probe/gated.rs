//! A probe with nothing of its own: a two-document corpus mined whole.
//!
//! A suite proves over it, under the runtime, what the SDK does for every
//! adapter, without riding a shipped prompt.

#![cfg(target_arch = "wasm32")]

use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
use emery_sdk::{Doc, Model, Seam, SourceKind};

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

// The probe's capabilities on the WASI defaults: the model alone.
struct Provider;
impl Model for Provider {}

struct Adapter;
export::export!(Adapter with_types_in export);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Documentation)
    }

    async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
        emery_sdk::extract(&Provider, id, input, DOCS, async |_, _| Ok(vec![Seam::Whole])).await
    }
}
