//! Provides the guest programs the component suites drive and the table of built components.
//!
//! On `wasm32`, this crate is the programs' shared helpers. Natively, it is
//! the artifact table `build.rs` generates: one path constant per compiled
//! component and a `foreach_<group>!` macro per group, so every program and
//! every `sources/*` adapter has a matching root test.

/// The guest id the adapter under test is registered as and dispatched to.
pub const ADAPTER: &str = "adapter";

#[cfg(target_arch = "wasm32")]
mod helpers;

#[cfg(target_arch = "wasm32")]
pub use helpers::*;

#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/gen.rs"));
