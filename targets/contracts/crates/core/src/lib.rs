//! Wasm-free core of the contracts target adapter: [`operations`] (the
//! format sub-flows, bounded verify-repair loop, and
//! validate-before-visible enforcement over the shared
//! `specify_guest_kit::phase` template), [`validate`] (the
//! baseline-contract validators absorbed from the `specify-contract`
//! extension, which now wraps this crate), and [`registry`] (the
//! embedded prose), natively testable against a mock
//! [`specify_guest_kit::Model`]. The wasm32 shim (`specify-contracts`)
//! owns bindings and export glue.

pub mod operations;
pub mod registry;
pub mod validate;
