//! Wasm-free core of the omnia target adapter: [`operations`] (the
//! build operation's phase legs and the deterministic report-coherence
//! gate over the shared `adapter::phase` template) and
//! [`registry`] (the embedded prose), natively testable against a mock
//! [`adapter::Model`]. The wasm32 shim (`omnia`) owns
//! bindings and export glue.
//!
//! Omnia's verification is cargo / clippy / wasm32 runs a wasm guest
//! cannot spawn, so — unlike contracts — there is no in-core validator
//! pass: the prompts instruct the spawned agent to run those commands in
//! the lent workspace, and the core's deterministic tail is limited to
//! what pure Rust over the mounted tree can check.

pub mod operations;
pub mod registry;
