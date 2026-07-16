//! Vectis target adapter.
//!
//! [`Vectis`] owns the judgment legs; the other modules are deterministic
//! prelude / postlude helpers (validate, materialize, scaffold, verify).

pub mod android;
pub mod android_scaffold;
mod error;
pub mod infer;
pub mod ios_scaffold;
pub mod materialize;
mod operations;
pub mod prepare;
mod registry;
pub mod scaffold;
pub mod schema_source;
pub mod shell;
pub mod sync;
pub mod validate;
pub mod verify;

pub use error::{EXIT_FAILURE, VectisError};
pub use operations::Vectis;

#[cfg(target_arch = "wasm32")]
mod guest {
    adapter::target!(crate::Vectis);
}
