//! Consolidated integration binary for `specify-prose-registry`.
//!
//! One binary per crate: each former `tests/<area>.rs` is pulled in here as a
//! `#[path]` submodule so the crate-under-test links exactly once.

#[path = "emit.rs"]
mod emit;
