//! The vectis target adapter, natively testable against a mock
//! [`adapter::JudgmentModel`]; the wasm32-only `guest` module owns
//! bindings and export glue.
//!
//! [`operations`] carries the build prompt's phase legs and validator
//! gate. The remaining modules are deterministic libraries the guest
//! calls as prelude / postlude around the judgment legs: validation,
//! asset materialization, prepare orchestration, shell verification,
//! Crux scaffolding, scaffold sync, and the Android Gradle-wrapper
//! bootstrap. [`registry`] holds the embedded prose.

pub mod android;
pub mod android_scaffold;
mod error;
pub mod infer;
pub mod ios_scaffold;
pub mod materialize;
pub mod operations;
pub mod prepare;
pub mod registry;
pub mod scaffold;
pub mod schema_source;
pub mod shell;
pub mod sync;
pub mod validate;
pub mod verify;

pub use error::{EXIT_FAILURE, VectisError};

#[cfg(target_arch = "wasm32")]
mod guest;
