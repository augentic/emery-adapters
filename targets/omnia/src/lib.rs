//! Omnia target adapter.
//!
//! No compiled-in validator: cargo / clippy / wasm32 stay agent-side in
//! the lent workspace; the deterministic tail only checks the mounted tree.

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Omnia);
}

mod operations;
mod registry {
    adapter::registry!();
}

pub use operations::Omnia;
