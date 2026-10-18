//! Probe adapter mining one seam through the SDK claim gate.

#![cfg(target_arch = "wasm32")]

use emery_sdk::{Context, Doc, Error, Evidence, Metadata, Model, Seam, SourceAdapter, SourceKind};

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

emery_sdk::export_source!(Adapter);

impl SourceAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            kind: SourceKind::Documentation,
        }
    }

    async fn extract<P: Model>(ctx: &Context<'_, P>) -> Result<Evidence, Error> {
        emery_sdk::extract(ctx, PROSE, &[Seam::whole()]).await
    }
}
