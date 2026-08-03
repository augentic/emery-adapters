//! Vectis target adapter.
//!
//! [`Adapter`] owns the judgment legs; the other modules are deterministic
//! prelude / postlude helpers (validate, materialize, scaffold, verify).

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Adapter);
}

mod operations;
mod registry {
    adapter::registry!();
}

pub mod android_scaffold;
mod composition_manifests;
mod error;
pub mod infer;
pub mod ios_scaffold;
pub mod materialize;
pub mod prepare;
pub(crate) mod projections;
pub mod scaffold;
pub mod schema_source;
pub mod shell;
pub mod validate;
pub mod verify;

pub use error::VectisError;
pub use operations::Adapter;
