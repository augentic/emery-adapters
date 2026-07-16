//! Documentation source adapter.

mod operations;
mod registry;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::source!(crate::Documentation);
}

pub use operations::Documentation;
