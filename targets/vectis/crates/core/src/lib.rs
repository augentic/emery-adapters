//! Wasm-free core of the vectis target adapter, natively testable
//! against a mock [`specify_guest_kit::Model`]; the wasm32 shim
//! (`specify-vectis`) owns bindings and export glue.
//!
//! - [`operations`] — the build brief's phase legs, the in-core
//!   composition validator gate with its bounded repair, and the
//!   deterministic report-coherence tail, over the shared
//!   `specify_guest_kit::phase` template.
//! - [`validate`] / [`materialize`] / [`prepare`] — the deterministic
//!   libraries absorbed from the legacy `specify-vectis-extension` WASI
//!   tool (RFC-61 Step 3): schema + cross-artifact validation for
//!   tokens / assets / layout / composition, canonical-to-export asset
//!   conversion (SVG rasterisation included), and the slice-build
//!   prepare scope resolution. The extension consumes these modules for
//!   its CLI surface until Step 5 deletes it; the guest calls them
//!   directly as the deterministic prelude and postlude around the
//!   judgment legs.
//! - [`registry`] — the embedded prose (`briefs/` + `references/` +
//!   `rules/`).

mod error;
pub mod materialize;
pub mod operations;
pub mod prepare;
pub mod registry;
pub mod schema_source;
pub mod validate;

pub use error::{EXIT_FAILURE, VectisError};
