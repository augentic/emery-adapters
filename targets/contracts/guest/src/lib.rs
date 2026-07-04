//! The `wasm32` shim of the contracts adapter guest (RFC-61 Step 2).
//!
//! Owns the platform glue the wasm-free `specify-contracts-core` cannot:
//! `wit_bindgen::generate!` over the vendored `wit/` pin, the
//! `target-adapter` world export routing `guidance` / `build` / `merge`
//! into [`specify_contracts_core::operations`], and the MCP reference
//! shelf serving [`specify_contracts_core::registry`] over
//! `wasi:http/incoming-handler`.
//!
//! The bindgen + export glue lands with Milestone D; until then this
//! crate anchors the workspace member and its core dependency.

pub use specify_contracts_core as core;
