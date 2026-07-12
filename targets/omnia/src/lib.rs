//! The omnia target adapter: [`operations`] (build/merge legs and the
//! report-coherence gate) and [`registry`] (embedded prose). The
//! wasm32-only `guest` module owns bindings and export glue.
//!
//! Unlike contracts, there is no compiled-in validator pass: omnia's
//! verification is cargo / clippy / wasm32 runs a wasm guest cannot
//! spawn, so the prompts have the agent run them in the lent workspace
//! and the deterministic tail only checks the mounted tree.

pub mod operations;
pub mod registry;

#[cfg(target_arch = "wasm32")]
mod guest;
