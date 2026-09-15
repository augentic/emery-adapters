//! A probe whose survey is two `Note` seams over a one-document corpus.
//!
//! A suite proves over it, under the runtime, that the host runs the
//! completions one guest issues together: the property the SDK's fan-out
//! rests on.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_sdk::{Context, Doc, Error, Model, Seam, SourceAdapter, SourceKind};

const DOCS: &[Doc] = &[Doc {
    path: "prompts/extract.md",
    body: "SYSTEM",
}];

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Documentation;

    fn docs() -> &'static [Doc] {
        DOCS
    }

    // Two seams whatever the input arm, so the provided `extract` holds
    // two completions pending at once over a workspace and over a value; the
    // survey itself spends none.
    fn survey<P: Model>(
        _model: &P, _ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Vec<Seam>, Error>> + Send {
        ready(Ok(vec![
            Seam::Note("The first half of the source.".to_owned()),
            Seam::Note("The second half of the source.".to_owned()),
        ]))
    }
}
