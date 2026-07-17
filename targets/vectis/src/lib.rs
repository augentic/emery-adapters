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

pub mod android;
pub mod android_scaffold;
mod error;
pub mod infer;
pub mod ios_scaffold;
pub mod materialize;
pub mod prepare;
pub mod scaffold;
pub mod schema_source;
pub mod shell;
pub mod sync;
pub mod validate;
pub mod verify;

pub use error::{EXIT_FAILURE, VectisError};
pub use operations::Adapter;
