//! Omnia target adapter.
//!
//! No compiled-in validator: cargo / clippy / wasm32 stay agent-side in
//! the lent workspace; the deterministic tail only checks the mounted tree.

mod operations;
mod registry;

pub use operations::Omnia;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Omnia);
}
