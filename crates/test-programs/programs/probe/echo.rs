//! A `source-adapter` probe of the `behaviour` kind that answers the maximal
//! evidence without a turn: the boundary's lift and lower, field by field.

#![cfg(target_arch = "wasm32")]

use std::future::{Future, ready};

use emery_sdk::{Context, Error, Evidence, SourceAdapter, SourceKind};
use test_programs::maximal;

struct Adapter;

emery_sdk::export_source!(Adapter);

impl SourceAdapter for Adapter {
    const KIND: SourceKind = SourceKind::Behaviour;

    fn extract<P>(_ctx: &Context<'_, P>) -> impl Future<Output = Result<Evidence, Error>> {
        ready(Ok(maximal()))
    }
}
