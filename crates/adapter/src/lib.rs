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
//!   `specify:adapter` WIT records, shared by every adapter core.
//! - [`answers`] — the vendored judgment-answer schema pins and the
//!   deserializers projecting schema-gated answers onto the seam types.
//! - [`judgment`] — the shared judgment-call helper: one schema-gated
//!   `create` with the reference grant and workspace lend attached.
//! - [`phase`] — the shared per-leg scaffolding for target operation
//!   templates: the internal phase-answer shape, prompt renderers, and
//!   the deterministic report-coherence checks.
//! - [`registry`] — the embedded prose vocabulary the
//!   `prose` codegen plugs into, plus the
//!   [`embed_registry!`] module generator.
//! - [`shelf`] — the MCP URL env convention plus (on `wasm32`) the
//!   generic `McpServer` reference shelf over an embedded doc table.
//! - `source` / `target` (`wasm32` only) — the `specify:adapter` world bindings,
//!   generated once per axis with a `pub` `export!` macro (omnia's
//!   `wasi-*` guest convention), plus the seam-type [`From`] mappings.
//!   Each adapter-root crate hand-writes its own thin shim over these.

pub mod answers;
mod call;
pub mod model;
pub mod phase;
pub mod registry;
pub mod seam;
pub mod shelf;

#[cfg(target_arch = "wasm32")]
pub mod source;
#[cfg(target_arch = "wasm32")]
pub mod target;

#[cfg(not(target_arch = "wasm32"))]
mod mock;

pub use call::judgment;
#[cfg(not(target_arch = "wasm32"))]
pub use mock::MockModel;
#[cfg(target_arch = "wasm32")]
pub use model::WasiModel;
pub use model::{Error, Format, McpGrant, Message, Model, Reply, Request, Role, SchemaFormat};
