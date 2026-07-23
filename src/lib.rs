//! Root package for the adapters workspace composition surface.
//!
//! On the native host this crate exposes the first-party catalog
//! declaration consumed by the `eval` example. The wasm32 build is a
//! stub so the package's Omnia/wasm examples remain target-clean.

#![cfg_attr(target_arch = "wasm32", allow(missing_docs))]

#[cfg(not(target_arch = "wasm32"))]
mod catalog;

#[cfg(not(target_arch = "wasm32"))]
pub use catalog::catalog;
