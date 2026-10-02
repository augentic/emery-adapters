//! The guest programs the root suites drive, from both sides of the boundary.
//!
//! On `wasm32` the crate is the programs' shared helpers; natively it is the
//! generated table of every built component and the `foreach_*!` macros that
//! hold the suites to it.

/// The guest name every driver addresses the adapter under test by.
pub const ADAPTER: &str = "adapter";

#[cfg(target_arch = "wasm32")]
mod helpers;

#[cfg(target_arch = "wasm32")]
pub use helpers::*;

#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/gen.rs"));
