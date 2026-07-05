//! Wasm-free core of the omnia target adapter (RFC-61 Step 3).
//!
//! Everything the omnia guest does that is not platform glue lives here,
//! natively testable against a mock [`specify_guest_kit::Model`]. The
//! generic machinery — the seam DTO vocabulary, the judgment-answer
//! schema pins and deserializers, the judgment-call helper, and the
//! prose-registry codegen — lives in `specify-guest-kit` /
//! `specify-prose-registry`; this crate keeps only what is omnia:
//!
//! - [`registry`] — the embedded prose registry (`briefs/` +
//!   `references/` + `rules/`, symlinks resolved at build time) the guest
//!   serves over MCP and the operations read for prompt assembly.
//! - [`operations`] — the omnia flow logic over the shared judgment
//!   template (`guidance`, `build`, `merge`): the build brief's phase
//!   legs and the deterministic report-coherence gate.
//!
//! Omnia's verification is cargo / clippy / wasm32 runs a wasm guest
//! cannot spawn, so — unlike contracts — there is no in-core validator
//! pass: the briefs instruct the spawned agent to run those commands in
//! the lent workspace, and the core's deterministic tail is limited to
//! what pure Rust over the mounted tree can check.
//!
//! No `cfg(target_arch)` appears anywhere in this crate; the wasm32-only
//! shim (`specify-omnia`, the adapter-root package) owns bindings and
//! export glue.

pub mod operations;
pub mod registry;
