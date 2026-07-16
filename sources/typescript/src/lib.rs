//! TypeScript source adapter.

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::source!(crate::Typescript);
}

mod operations;
mod registry {
    adapter::registry!();
}

pub use operations::Typescript;
