//! Contracts target adapter.

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Adapter);
}

mod operations;
mod registry {
    adapter::registry!();
}
pub mod validate;

pub use operations::Adapter;
