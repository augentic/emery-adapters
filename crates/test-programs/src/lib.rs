//! The guest scenario programs the seam suites drive, from both sides of the
//! boundary.
//!
//! On `wasm32` the crate is the programs' shared helpers; each program under
//! `programs/<group>/<scenario>.rs` is an `[[example]]` compiled to a
//! component. Natively it is the compiled artifacts: `build.rs` runs that
//! `wasm32-wasip2` build over the programs and over every `sources/*`
//! adapter, and generates one `pub const <NAME>: &str` path per component
//! plus a `foreach_<group>!` macro a suite invokes to prove every program
//! (`foreach_source!`, `foreach_probe!`) or every adapter
//! (`foreach_adapter!`) has a matching test — one `gen.rs` for both. A suite
//! runs an artifact through `omnia_test::host`.

/// The guest id every suite registers the adapter under test as, and the id
/// every driver program dispatches to over `emery:adapter/source`.
pub const ADAPTER: &str = "adapter";

#[cfg(target_arch = "wasm32")]
mod helpers;

#[cfg(target_arch = "wasm32")]
pub use helpers::*;

#[cfg(not(target_arch = "wasm32"))]
include!(concat!(env!("OUT_DIR"), "/gen.rs"));
