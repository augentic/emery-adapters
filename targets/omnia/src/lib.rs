//! Omnia target adapter.
//!
//! No compiled-in validator: cargo / clippy / wasm32 stay agent-side in
//! the lent workspace; the deterministic tail only checks the mounted tree.

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Adapter);
}

mod operations;
mod registry {
    adapter::registry!();
}
mod review;
pub mod scaffold;

pub use operations::Adapter;
