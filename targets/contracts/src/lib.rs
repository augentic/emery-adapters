//! The contracts target adapter: [`operations`] (format sub-flows,
//! verify-repair loop, validate-before-visible enforcement),
//! [`validate`] (baseline-contract validators), and [`registry`]
//! (embedded prose). The wasm32-only `guest` module owns bindings
//! and export glue.

pub mod operations;
pub mod registry;
pub mod validate;

#[cfg(target_arch = "wasm32")]
mod guest;
