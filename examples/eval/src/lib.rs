//! Grading kernels for the live eval runner (remediation Phase 4
//! item 3): a client of the public contract only (architecture-review
//! T6). The runner observes the shipped `emery` binary's argv, typed
//! exit codes, JSON envelopes, and the committed spec set — never
//! engine internals, and never telemetry (ADR-0001: `wasi:otel` is
//! emit-only and must not feed grading).

pub mod envelope;
pub mod grade;
pub mod scorecard;
