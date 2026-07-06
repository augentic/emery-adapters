//! Wasm-free core of the contracts target adapter: [`operations`] (the
//! format sub-flows, bounded verify-repair loop, and
//! validate-before-visible enforcement over the shared
//! `adapter::phase` template), [`validate`] (the
//! baseline-contract validators), and [`registry`] (the embedded
//! prose), natively testable against a mock
//! [`adapter::Model`]. The wasm32 shim (`contracts`)
//! owns bindings and export glue.

pub mod operations;
pub mod registry;
pub mod validate;
