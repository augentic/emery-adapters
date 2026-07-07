//! Consolidated integration binary for `testkit`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once.

#[path = "mock.rs"]
mod mock;
