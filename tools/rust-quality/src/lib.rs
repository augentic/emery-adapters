//! Dev-only Rust-quality gate for the `specify-adapters` workspace.
//!
//! This crate has no runtime surface; it exists solely to host the
//! workspace-wide unit-test ratchet in `tests/rust_quality.rs`. The gate
//! mirrors the engine's `tests/rust_quality.rs` and enforces the
//! integration-first posture in the workspace `TESTING.md`.
