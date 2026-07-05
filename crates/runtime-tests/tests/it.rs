//! Consolidated integration binary for the composed-deployment tests.
//!
//! One binary per crate: each area file is pulled in as a `#[path]`
//! submodule so the harness links exactly once. See
//! [docs/standards/testing.md](../../../docs/standards/testing.md).

#[path = "common.rs"]
mod common;
#[path = "contracts.rs"]
mod contracts;
#[path = "omnia.rs"]
mod omnia;
#[path = "sources.rs"]
mod sources;
