//! Screenshots source adapter.

mod operations;
mod registry;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::source!(crate::Screenshots);
}

pub use operations::Screenshots;
