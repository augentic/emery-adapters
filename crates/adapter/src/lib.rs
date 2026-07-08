//! Shared guest support for Specify adapter components.
//!
//! Owns everything an adapter guest repeats verbatim, so per-adapter
//! crates stay thin:
//!
//! - [`model`] — the local `Model` capability trait; cores take
//!   `P: Model` bounds, `wasm32` delegates to `omnia-wasi-model`, tests
//!   bind the `testkit` crate's scripted `MockModel`.
//! - [`seam`] — the DTO vocabulary mirroring the `specify:adapter` WIT
//!   records.
//! - [`answers`] — the vendored judgment-answer schema pins and their
//!   deserializers.
//! - [`judgment`] — the shared judgment-call helper: one schema-gated
//!   `create` with the reference grant and workspace lend attached.
//! - [`phase`] — per-leg scaffolding for target operation templates.
//! - [`registry`] — the embedded prose vocabulary plus the
//!   [`embed_registry!`] module generator.
//! - [`references`] — the MCP URL env convention plus (on `wasm32`) the
//!   generic `McpServer` over an embedded doc table.
//! - `source` / `target` (`wasm32` only) — the `specify:adapter` world
//!   bindings each adapter-root crate hand-writes its thin shim over.

pub mod answers;
mod call;
pub mod model;
pub mod phase;
pub mod references;
pub mod registry;
pub mod seam;

#[cfg(target_arch = "wasm32")]
pub mod source;
#[cfg(target_arch = "wasm32")]
pub mod target;

pub use call::judgment;
#[cfg(target_arch = "wasm32")]
pub use model::WasiModel;
pub use model::{Error, Format, McpGrant, Message, Model, Reply, Request, Role, SchemaFormat};
