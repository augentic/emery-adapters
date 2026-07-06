//! Dev-only crate hosting the composed-deployment integration tests for
//! the adapter guest components (RFC-61 Step 2).
//!
//! This crate has no runtime surface; the tests live in `tests/it.rs` —
//! they build the guest wasm, deploy it from a temp manifest on the Omnia
//! runtime, and drive the seams (host-mediated dispatch, the MCP reference
//! shelf) in-process.
