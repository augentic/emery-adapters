//! The Rust-native execution shim.
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
//! - [`model`] — the native `Model` backends: [`model::CursorModel`]
//!   (cursor-agent, live-only) behind the lazily connected
//!   [`model::DevModel`]; tests bind `omnia_testkit::model::Scripted`
//!   through the provider's generic parameter.
//! - [`mcp`] — the per-adapter reference shelves, mounted at
//!   `/mcp/<name>` on the serve-mode listener via
//!   `omnia_guest::mcp::router`.
//! - [`eval`] — the live-model trial: the operator rhythm
//!   (`init → plan → execute → finalize`) over a persistent
//!   `sandbox/eval/` project with real adapters, graded by
//!   deterministic validators only.
//!
//! What this shim cannot prove: WIT bindings, Omnia's dispatch-by-id,
//! and mount/preopen wiring — that surface stays with the shipped
//! guest (see `harness/README.md`).

pub mod catalog;
pub mod eval;
pub mod mcp;
pub mod model;
pub mod provider;
