//! Contracts target adapter.

mod operations;
mod registry;
pub mod validate;

pub use operations::Contracts;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Contracts);
}
