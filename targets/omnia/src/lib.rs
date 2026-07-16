//! The omnia target adapter: [`Omnia`] (the `adapter::Target`
//! implementor carrying the build/merge legs and the report-coherence
//! gate) and `registry` (embedded prose). The wasm32-only `guest`
//! module is one `adapter::target!` invocation.
//!
//! Unlike contracts, there is no compiled-in validator pass: omnia's
//! verification is cargo / clippy / wasm32 runs a wasm guest cannot
//! spawn, so the prompts have the agent run them in the lent workspace
//! and the deterministic tail only checks the mounted tree.

mod operations;
mod registry;

pub use operations::Omnia;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Omnia);
}
