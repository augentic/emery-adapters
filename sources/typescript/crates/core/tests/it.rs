//! Consolidated integration binary for `specify-typescript-core`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once. See
//! `TESTING.md` at the workspace root.
//!
//! Full operation coverage lives in `specify-documentation-core`'s tests —
//! the five source cores share one template — so this binary asserts only
//! what is typescript-specific.

#[path = "operations.rs"]
mod operations;
#[path = "registry.rs"]
mod registry;
