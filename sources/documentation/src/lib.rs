//! The documentation source adapter: [`operations`] (survey / extract
//! judgment legs) and [`registry`] (embedded prose). The wasm32-only
//! `guest` module owns bindings and export glue.

pub mod operations;
pub mod registry;

#[cfg(target_arch = "wasm32")]
mod guest;
