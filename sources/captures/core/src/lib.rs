//! Wasm-free core of the captures source adapter: [`operations`] (the
//! survey / extract judgment legs) and [`registry`] (the embedded
//! prose), natively testable against a mock
//! [`adapter::Model`]. The wasm32 shim (`captures`)
//! owns bindings and export glue.

pub mod operations;
pub mod registry;
