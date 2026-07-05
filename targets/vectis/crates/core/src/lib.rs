//! Wasm-free core of the vectis target adapter (RFC-61 Step 3).
//!
//! Everything the vectis guest does that is not platform glue lives
//! here, natively testable against a mock [`specify_guest_kit::Model`].
//! The generic machinery — the seam DTO vocabulary, the judgment-answer
//! schema pins and deserializers, the judgment-call helper, and the
//! prose-registry codegen — lives in `specify-guest-kit` /
//! `specify-prose-registry`; this crate keeps what is vectis:
//!
//! - [`registry`] — the embedded prose registry (`briefs/` +
//!   `references/` + `rules/`, symlinks resolved at build time) the
//!   guest serves over MCP and the operations read for prompt assembly.
//! - [`validate`] / [`materialize`] / [`prepare`] — the deterministic
//!   libraries absorbed from the legacy `specify-vectis-extension` WASI
//!   tool (RFC-61 Step 3): schema + cross-artifact validation for
//!   tokens / assets / layout / composition, canonical-to-export asset
//!   conversion (SVG rasterisation included), and the slice-build
//!   prepare scope resolution. The extension consumes these modules for
//!   its CLI surface until Step 5 deletes it; the guest calls them
//!   directly as the deterministic prelude and postlude around the
//!   judgment legs.
//! - [`operations`] — the vectis flow logic over the shared judgment
//!   template (`guidance`, `build`, `merge`): the build brief's phase
//!   legs, the in-core composition validator gate with its bounded
//!   repair, and the deterministic report-coherence tail.
//!
//! Unlike omnia (whose verification is cargo runs a wasm guest cannot
//! spawn), vectis carries real in-core validators — the composition /
//! tokens / assets cross-checks — so the operations bracket the model
//! legs with deterministic guest code the way the contracts adapter
//! does. Host-command verification (cargo, xcodebuild, Gradle, the
//! host-prereq and finalize-verify scripts) stays agent-side: the
//! briefs instruct the spawned agent to run those in the lent
//! workspace.
//!
//! No `cfg(target_arch)` appears anywhere in this crate; the
//! wasm32-only shim (`specify-vectis`, the adapter-root package) owns
//! bindings and export glue.

mod error;
pub mod materialize;
pub mod operations;
pub mod prepare;
pub mod registry;
pub mod schema_source;
pub mod validate;

pub use error::{EXIT_FAILURE, VectisError};
