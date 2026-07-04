//! Wasm-free core of the contracts target adapter (RFC-61 Step 2).
//!
//! Everything the contracts guest does that is not platform glue lives
//! here, natively testable against a mock [`specify_guest_kit::Model`]:
//!
//! - [`validate`] — the baseline-contract validators absorbed from the
//!   `specify-contract` extension (which now wraps this crate).
//! - [`registry`] — the embedded prose registry (`briefs/` +
//!   `references/`, symlinks resolved at build time) the guest serves
//!   over MCP and the operations read for prompt assembly.
//! - [`report`] — schema-gated answer deserialization and the compact
//!   seam projection (diagnostic → WIT `finding`).
//! - [`operations`] — the judgment operation template (`guidance`,
//!   `build`, `merge`): prompt assembly, single-shot `create` calls with
//!   schema-gated formats, validate-before-visible enforcement.
//!
//! No `cfg(target_arch)` appears anywhere in this crate; the wasm32-only
//! shim (`specify-contracts-guest`) owns bindings and export glue.

pub mod operations;
pub mod registry;
pub mod report;
pub mod validate;
