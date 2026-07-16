//! Captures source adapter.

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::source!(crate::Captures);
}

mod operations;
mod registry {
    adapter::registry!();
}

pub use operations::Captures;
