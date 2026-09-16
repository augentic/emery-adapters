//! A probe mining two `Note` seams over a one-document corpus.
//!
//! A suite proves over it, under the runtime, that the host runs the
//! completions one guest issues together: the property the SDK's fan-out
//! rests on.

#![cfg(target_arch = "wasm32")]

use emery_sdk::export::{self, AdapterId, AdapterMetadata, Error, Evidence, Guest, Input};
use emery_sdk::{Doc, Seam, SourceKind};

const DOCS: &[Doc] = &[Doc {
    path: "prompts/extract.md",
    body: "SYSTEM",
}];

struct Adapter;
export::export!(Adapter with_types_in export);

impl Guest for Adapter {
    fn metadata(_id: AdapterId) -> AdapterMetadata {
        emery_sdk::metadata(SourceKind::Documentation)
    }

    // Two seams whatever the input arm, so `mine` holds two completions
    // pending at once over a workspace and over a value; no survey turn is
    // spent choosing them.
    async fn extract(id: AdapterId, input: Input) -> Result<Evidence, Error> {
        emery_sdk::extract(id, input, DOCS, async |_| {
            Ok(vec![
                Seam::Note("The first half of the source.".to_owned()),
                Seam::Note("The second half of the source.".to_owned()),
            ])
        })
        .await
    }
}
