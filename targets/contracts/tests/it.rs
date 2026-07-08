//! Consolidated integration binary for the `contracts` adapter's
//! wasm-free modules.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once. See
//! `TESTING.md` at the workspace root.

#[path = "operations.rs"]
mod operations;
#[path = "registry.rs"]
mod registry;
#[path = "validate.rs"]
mod validate;
