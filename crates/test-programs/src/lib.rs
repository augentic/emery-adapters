//! Provides components and shared helpers for adapter integration tests.
//!
//! Native builds expose generated paths and iteration macros for every test
//! component. WebAssembly builds expose the input builders and assertions
//! used by those components.

/// The guest identifier used to register the adapter under test.
pub const ADAPTER: &str = "adapter";

#[cfg(target_arch = "wasm32")]
mod helpers;

#[cfg(target_arch = "wasm32")]
pub use helpers::*;

#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/gen.rs"));
