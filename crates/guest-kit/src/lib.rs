//! Shared guest support for Specify adapter components.
//!
//! Owns everything an adapter guest repeats verbatim, so per-adapter
//! crates stay thin:
//!
//! - [`model`] — the **local `Model` capability trait**, the stand-in for
//!   the `Model` capability RFC-61 adds to `omnia-guest::capabilities`.
//!   Wasm-free adapter cores take `P: Model` bounds and issue judgment
//!   calls through the trait; on `wasm32` the default method body
//!   delegates to the `omnia-wasi-model` bindings, and off `wasm32` tests
//!   bind [`MockModel`]. When the upstream capability lands, the swap is a
//!   one-line import change.
//! - [`seam`] — the wasm-free DTO vocabulary mirroring the
//!   `augentic:specify` WIT records, shared by every adapter core.
//! - [`answers`] — the vendored judgment-answer schema pins and the
//!   deserializers projecting schema-gated answers onto the seam types.
//! - [`judgment`] — the shared judgment-call helper: one schema-gated
//!   `create` with the reference grant and workspace lend attached.
//! - [`phase`] — the shared per-leg scaffolding for target operation
//!   templates: the internal phase-answer shape, prompt renderers, and
//!   the deterministic report-coherence checks.
//! - [`registry`] — the embedded prose-registry vocabulary the
//!   `specify-prose-registry` codegen plugs into, plus the
//!   [`embed_registry!`] module generator.
//! - [`shelf`] — the MCP URL env convention plus (on `wasm32`) the
//!   generic `McpServer` reference shelf over an embedded doc table.
//! - [`source_adapter!`] / [`target_adapter!`] — the `wasm32` shim
//!   macros every adapter-root crate invokes.

pub mod adapter;
pub mod answers;
mod call;
pub mod model;
pub mod phase;
pub mod registry;
pub mod seam;
pub mod shelf;

#[cfg(not(target_arch = "wasm32"))]
mod mock;

pub use call::judgment;
#[cfg(not(target_arch = "wasm32"))]
pub use mock::MockModel;
pub use model::{Error, Format, McpGrant, Message, Model, Reply, Request, Role, SchemaFormat};
