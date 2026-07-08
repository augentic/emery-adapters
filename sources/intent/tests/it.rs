//! Consolidated integration binary for the `intent` adapter's
//! wasm-free modules.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once. See
//! `TESTING.md` at the workspace root.
//!
//! Full operation coverage lives in the `documentation` adapter's tests —
//! the five source adapters share one template — so this binary asserts only
//! what is intent-specific.

#[path = "operations.rs"]
mod operations;
#[path = "registry.rs"]
mod registry;
