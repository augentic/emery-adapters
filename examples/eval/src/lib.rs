//! Grading kernels for the live eval runner — a client of the public
//! contract only: argv, typed exit codes, JSON envelopes, and the
//! committed spec set. Never engine internals, never telemetry.

pub mod envelope;
pub mod grade;
pub mod scorecard;
