//! Consolidated integration binary for `specify-captures-core`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once. See
//! [docs/standards/testing.md](../../../../../docs/standards/testing.md).
//!
//! Full operation coverage lives in `specify-documentation-core`'s tests —
//! the five source cores share one template — so this binary asserts only
//! what is captures-specific.

#[path = "operations.rs"]
mod operations;
#[path = "registry.rs"]
mod registry;
