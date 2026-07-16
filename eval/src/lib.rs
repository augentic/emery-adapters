//! Rust-native execution shim for `specify-dev` and the eval harness.
//! Binds adapter capabilities in-process so the workflow runs without a wasm runtime.

pub mod catalog;
pub mod env;
pub mod fs;
pub mod inputs;
pub mod mcp;
pub mod model;
pub mod native;
pub mod provider;
pub mod scenario;
