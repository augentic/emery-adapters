//! A probe with nothing of its own: a two-document corpus mined whole.
//!
//! A suite proves over it, under the runtime, what the SDK does for every
//! adapter, without riding a shipped prompt.

#![cfg(target_arch = "wasm32")]

use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
use emery_sdk::model::WasiModel;
use emery_sdk::{Context, Doc, Seam, SourceInput, SourceKind};

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

struct Adapter;
export::export!(Adapter with_types_in export);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        export::metadata(SourceKind::Documentation)
    }

    async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
        let input = SourceInput::from(input);
        let ctx = Context {
            adapter_id: &id,
            input: &input,
        };
        Ok(emery_sdk::mine(&WasiModel, &ctx, DOCS, &[Seam::Whole]).await?.into())
    }
}
