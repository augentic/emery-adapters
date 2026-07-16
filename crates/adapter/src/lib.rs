//! Shared guest support for Specify adapter components.
//!
//! Owns everything an adapter guest repeats verbatim, so per-adapter
//! crates stay thin:
//!
//! - the model capability is the upstream [`omnia_guest::Model`]
//!   (re-exported here with its request/reply vocabulary); cores take
//!   `P: Model` bounds, `wasm32` binds `WasiModel`, tests bind the
//!   recorded scripted harness from `omnia-testkit`.
//! - [`Source`] / [`Target`] — the per-axis operations traits: the one
//!   Rust statement of what an adapter implements, consumed by the wasm
//!   export macros and every native harness.
//! - [`seam`] — the DTO vocabulary mirroring the `specify:adapter` WIT
//!   records.
//! - [`answers`] — the vendored judgment-answer schema pins and their
//!   deserializers.
//! - [`judgment`] — the shared judgment-call helper: one schema-gated
//!   `create` with the reference grant and workspace lend attached.
//! - [`repaired`] — the source-leg variant: the same call bracketed
//!   by a deterministic answer tail inside a bounded repair loop.
//! - [`phase`] — per-leg scaffolding for target operation templates.
//! - [`registry`] — the embedded prose vocabulary plus the
//!   [`embed_registry!`] module generator.
//! - [`references`] — the MCP URL env convention plus the target-neutral
//!   generic `McpServer` over an embedded doc table (only the
//!   `wasi:http` `serve` bridge is `wasm32`-gated).
//! - `source` / `target` (`wasm32` only) — the `specify:adapter` world
//!   bindings plus the `source!` / `target!` export macros that wire a
//!   trait implementor into a component's exports; an adapter's whole
//!   wasm shim is one macro invocation.

pub mod answers;
mod call;
mod operations;
pub mod phase;
pub mod references;
pub mod registry;
pub mod seam;

#[cfg(target_arch = "wasm32")]
pub mod source;
#[cfg(target_arch = "wasm32")]
pub mod target;

pub use call::{MAX_REPAIRS, judgment, repaired};
pub use omnia_guest::Model;
#[cfg(target_arch = "wasm32")]
pub use omnia_guest::model::WasiModel;
pub use omnia_guest::model::{
    Error, Format, McpGrant, Message, Reply, Request, Role, SchemaFormat, Tool,
};
pub use operations::{Source, Target};
/// Re-exported for the `source!` / `target!` macro expansions, which
/// wire the references server into the leaf crate's `wasi:http` export.
#[cfg(target_arch = "wasm32")]
pub use wasip3;
