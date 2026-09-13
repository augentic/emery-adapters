//! A source adapter answering the maximal evidence without a model call, so
//! a suite can prove every field of the contract's records — each claim
//! kind, both `backing` arms, every anchor form, extras beyond strings —
//! survives the WIT bindings' lowering on export and lift on the caller's
//! side. Stands in for the adapter under test when the lowering itself is
//! the subject.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter};
use test_programs::maximal;

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const SOURCE: &'static str = "echo probe";

    // No corpus: the probe never reaches the model.
    fn docs() -> &'static [Doc] {
        &[]
    }

    // Nothing to await: the answer is ready before the model is asked.
    fn extract<P: Model>(
        _model: &P, _ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Evidence, Error>> + Send {
        ready(Ok(maximal()))
    }
}
