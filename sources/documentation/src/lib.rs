//! Documentation source adapter.

#[cfg(target_arch = "wasm32")]
mod guest {
    emery_adapter::source!(crate::Adapter);
}

mod operations;
mod registry {
    emery_prose::registry!();
}

pub use operations::Adapter;
