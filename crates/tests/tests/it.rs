//! Consolidated integration binary for the composed-deployment tests.
//!
//! One binary per crate: each area file is pulled in as a `#[path]`
//! submodule so the harness links exactly once. See
//! `TESTING.md` at the workspace root.

#[path = "common.rs"]
mod common;
#[path = "contracts.rs"]
mod contracts;
#[path = "omnia.rs"]
mod omnia;
#[path = "sources.rs"]
mod sources;
#[path = "sources_rest.rs"]
mod sources_rest;
#[path = "vectis.rs"]
mod vectis;
