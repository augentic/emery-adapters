//! Screenshots source adapter.

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::source!(crate::Screenshots);
}

mod operations;
mod registry {
    adapter::registry!();
}

pub use operations::Screenshots;
