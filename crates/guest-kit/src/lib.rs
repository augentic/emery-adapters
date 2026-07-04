//! Shared guest support for Specify adapter components.
//!
//! Owns the **local `Model` capability trait** — the stand-in for the
//! `Model` capability RFC-61 adds to `omnia-guest::capabilities`. Wasm-free
//! adapter cores take `P: Model` bounds and issue judgment calls through the
//! trait; on `wasm32` the default method body delegates to the
//! `omnia-wasi-model` bindings, and off `wasm32` tests bind [`MockModel`].
//! When the upstream capability lands, the swap is a one-line import change.

pub mod model;

#[cfg(not(target_arch = "wasm32"))]
mod mock;

#[cfg(not(target_arch = "wasm32"))]
pub use mock::MockModel;
pub use model::{Error, Format, McpGrant, Message, Model, Reply, Request, Role, SchemaFormat};
