//! Wasm-free core of the vectis target adapter, natively testable
//! against a mock [`specify_guest_kit::Model`]; the wasm32 shim
//! (`specify-vectis`) owns bindings and export glue.
//!
//! - [`operations`] — the build brief's phase legs, the in-core
//!   composition validator gate with its bounded repair, and the
//!   deterministic report-coherence tail, over the shared
//!   `specify_guest_kit::phase` template.
//! - [`validate`] / [`materialize`] / [`prepare`] / [`infer`] /
//!   [`verify`] / [`scaffold`] / [`sync`] / [`android`] — the
//!   deterministic libraries: schema + cross-artifact validation,
//!   canonical-to-export asset conversion (SVG rasterisation included),
//!   slice-build prepare orchestration, component-identity clustering
//!   (the catalog infer report), declared-platform shell verification,
//!   render-only Crux scaffolding, scaffold-file sync, and the Android
//!   Gradle-wrapper bootstrap. The guest calls them directly as the
//!   deterministic prelude and postlude around the judgment legs.
//! - [`registry`] — the embedded prose (`briefs/` + `references/` +
//!   `rules/`).

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
