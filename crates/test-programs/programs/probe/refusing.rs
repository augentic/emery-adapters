//! A source adapter that refuses its input before touching the model: the
//! SDK lowers its `bad_request!` onto the WIT `invalid-request` arm, and the
//! caller side lifts it back to `bad_request` with no model call recorded.
//! Stands in for the adapter under test when a suite proves the refusal
//! path of the seam itself.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, bad_request};

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const SOURCE: &'static str = "refusing probe";

    // No corpus: the probe never reaches the model.
    fn docs() -> &'static [Doc] {
        &[]
    }

    async fn extract<P: Model>(_model: &P, ctx: &Context<'_>) -> Result<Evidence, Error> {
        Err(bad_request!("the probe refuses source `{}`", ctx.input.key))
    }
}
