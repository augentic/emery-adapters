//! Consolidated integration binary for `vectis-core`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once. See
//! `TESTING.md` at the workspace root.

#[path = "absorbed.rs"]
mod absorbed;
#[path = "appendices.rs"]
mod appendices;
#[path = "catalog.rs"]
mod catalog;
#[path = "operations.rs"]
mod operations;
#[path = "registry.rs"]
mod registry;
#[path = "scaffold.rs"]
mod scaffold;
#[path = "shell.rs"]
mod shell;
