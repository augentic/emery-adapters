//! Wasm-free core of the contracts target adapter (RFC-61 Step 2).
//!
//! Everything the contracts guest does that is not platform glue lives
//! here, natively testable against a mock [`specify_guest_kit::Model`].
//! The generic machinery — the seam DTO vocabulary, the judgment-answer
//! schema pins and deserializers, the judgment-call helper, and the
//! prose-registry codegen — lives in `specify-guest-kit` /
//! `specify-prose-registry`; this crate keeps only what is contracts:
//!
//! - [`validate`] — the baseline-contract validators absorbed from the
//!   `specify-contract` extension (which now wraps this crate).
//! - [`registry`] — the embedded prose registry (`briefs/` +
//!   `references/`, symlinks resolved at build time) the guest serves
//!   over MCP and the operations read for prompt assembly.
//! - [`operations`] — the contracts flow logic over the shared judgment
//!   template (`guidance`, `build`, `merge`): the format sub-flows, the
//!   bounded verify-repair loop, and validate-before-visible enforcement.
//!
//! No `cfg(target_arch)` appears anywhere in this crate; the wasm32-only
//! shim (`specify-contracts`, the adapter-root package) owns bindings and export glue.

pub mod operations;
pub mod registry;
pub mod validate;
