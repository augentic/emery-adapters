//! The intent source adapter: [`Intent`] (the `adapter::Source`
//! implementor carrying the survey / extract judgment legs) and
//! `registry` (embedded prose). The wasm32-only `guest` module is
//! one `adapter::source!` invocation.

mod operations;
mod registry;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::source!(crate::Intent);
}

pub use operations::Intent;
