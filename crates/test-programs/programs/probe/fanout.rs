//! A probe whose survey is two `Prepared` notes over a one-document corpus.
//!
//! A suite proves over it, under the runtime, that the host runs the
//! completions one guest issues together: the property the SDK's fan-out
//! rests on.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Material, Model, SourceAdapter, SourceKind};

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

    // Two materials whatever the input arm, so the provided `extract` holds
    // two completions pending at once over a workspace and over a value; the
    // survey itself spends none.
    fn survey<P: Model>(
        _model: &P, _ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Vec<Material>, Error>> + Send {
        ready(Ok(vec![
            Material::Prepared("The first half of the source.".to_owned()),
            Material::Prepared("The second half of the source.".to_owned()),
        ]))
    }
}
