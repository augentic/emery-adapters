//! Consolidated integration binary for `specify-vectis-core`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once. See
//! `TESTING.md` at the workspace root.

#[path = "absorbed.rs"]
mod absorbed;
#[path = "operations.rs"]
mod operations;
#[path = "registry.rs"]
mod registry;
