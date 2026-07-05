//! Wasm-free core of the captures source adapter (RFC-61 Step 3).
//!
//! Everything the captures guest does that is not platform glue lives
//! here, natively testable against a mock [`specify_guest_kit::Model`].
//! The generic machinery — the seam DTO vocabulary, the judgment-answer
//! schema pins, deserializers, and validation tails, the judgment-call
//! helper, and the prose-registry codegen — lives in `specify-guest-kit` /
//! `specify-prose-registry`; this crate keeps only what is captures:
//!
//! - [`registry`] — the embedded prose registry (`briefs/` +
//!   `references/`, symlinks resolved at build time) the guest serves
//!   over MCP and the operations read for prompt assembly.
//! - [`operations`] — the survey / extract judgment operations over the
//!   shared judgment template.
//!
//! No `cfg(target_arch)` appears anywhere in this crate; the wasm32-only
//! shim (`specify-captures`, the adapter-root package) owns bindings
//! and export glue.

pub mod operations;
pub mod registry;
