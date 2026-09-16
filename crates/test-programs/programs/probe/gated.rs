//! A probe with nothing of its own: a two-document corpus over the default survey.
//!
//! A suite proves over it, under the runtime, what the SDK does for every
//! adapter, without riding a shipped prompt.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use emery_sdk::{Doc, SourceAdapter, SourceKind};

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

    fn docs() -> &'static [Doc] {
        DOCS
    }
}
