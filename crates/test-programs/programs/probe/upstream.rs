//! A source adapter whose own failure crosses the seam: the SDK lowers its
//! `bad_gateway!` onto the WIT `internal` arm — the arm every class but a
//! refusal shares — and the caller side lifts it to `bad_gateway`, naming
//! the source, with no model call recorded. Stands in for the adapter under
//! test when a suite proves the failure path of the seam itself.

#![cfg(target_arch = "wasm32")]

emery_sdk::source!(crate::Adapter);

use std::future::{Future, ready};

use emery_prose::registry::Doc;
use emery_sdk::{Context, Error, Evidence, Model, SourceAdapter, bad_gateway};

#[derive(Debug)]
struct Adapter;

impl SourceAdapter for Adapter {
    const SOURCE: &'static str = "upstream probe";

    // No corpus: the probe never reaches the model.
    fn docs() -> &'static [Doc] {
        &[]
    }

    // Nothing to await: the failure is ready before the model is asked.
    fn extract<P: Model>(
        _model: &P, ctx: &Context<'_>,
    ) -> impl Future<Output = Result<Evidence, Error>> + Send {
        ready(Err(bad_gateway!("the probe's upstream failed for source `{}`", ctx.input.key)))
    }
}
