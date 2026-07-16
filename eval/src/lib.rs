//! The Rust-native execution shim behind `specify-dev` and the eval
//! harness.
//!
//! The wasm guest crosses the `specify:adapter` WIT boundary for its
//! judgment seams; this crate binds the same capabilities natively so
//! the whole workflow — handlers, orchestrators, adapter operations — runs
//! without a wasm runtime:
//!
//! - [`provider`] — [`provider::Provider`]: `project::handler::Anchor` +
//!   `omnia_guest::Model` + `Source` / `Target` as an
//!   in-process dispatch table over the sibling adapter crates'
//!   `operations` modules, plus the matching metadata runner.
//! - [`native`] — the guest-side [`omnia_guest::Model`] over a host-side
//!   `WasiModelCtx` backend: the same request mapping, request gate, and
//!   answer projection the wasm default body and the host boundary
//!   perform together in a deployment. Mirrors the engine eval crate's
//!   module of the same name — the seam a shared eval core would own.
//! - [`model`] — [`model::DevModel`]: the binary's lazily connected
//!   cursor backend behind [`native::Native`], so deterministic verbs
//!   never require cursor-agent on `PATH`. Tests bypass it and bind
//!   `omnia_testkit::model::Scripted` through the provider's generic
//!   parameter.
//! - [`mcp`] — the per-adapter reference shelves, mounted at
//!   `/mcp/<name>` on the serve-mode listener via
//!   `omnia_guest::mcp::router`.
//!
//! The trial (`specify-dev eval`) and the single-operation prompt
//! scenarios (`specify-dev eval scenario`) live with the binary in
//! `main.rs`'s modules, mirroring the engine's `crates/eval` layout.
//!
//! What this shim cannot prove: WIT bindings, Omnia's dispatch-by-id,
//! and mount/preopen wiring — that surface stays with the composed
//! tests and the wasm change example (see `TESTING.md`).

pub mod catalog;
pub mod fs;
pub mod mcp;
pub mod model;
pub mod native;
pub mod provider;
