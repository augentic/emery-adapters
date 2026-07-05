//! Consolidated integration binary for `specify-contracts-core`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once. See
//! [docs/standards/testing.md](../../../../../docs/standards/testing.md).

#[path = "operations.rs"]
mod operations;
#[path = "registry.rs"]
mod registry;
#[path = "validate.rs"]
mod validate;
