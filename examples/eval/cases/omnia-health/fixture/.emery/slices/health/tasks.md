# health — Tasks

Tasks follow the omnia build legs: crate, tests, guest, verify-repair, then standards review. Every task is agent-completable in the lent workspace.

## Crate

- [ ] Author the workspace `Cargo.toml` and `crates/health/Cargo.toml` (workspace deps for `omnia-guest`, `serde`, `thiserror`, `anyhow`, `tracing`).
- [ ] Implement `HealthError`, `HealthStatus`, and `HealthCheck` per `design.md` under `crates/health/src/`.
- [ ] Compose `AppProvider: Config` and wire module exports from `crates/health/src/lib.rs`.

## Tests

- [ ] Author `MockProvider` implementing `Config` under `crates/health/tests/provider.rs`.
- [ ] Generate one integration test per REQ-001 scenario with `/// Spec: health > REQ-001 > Scenario: ...` traceability comments.
- [ ] Cover the missing-`SERVICE_NAME` path asserting `ServerError` / `config_missing`.

## Guest

- [ ] Author root `src/lib.rs` guest wiring with a typed HTTP router registering `GET /health` → `HealthCheck`.
- [ ] Emit `.env.example` documenting `SERVICE_NAME`.

## Verify-repair

- [ ] Run `cargo fmt --check`, `cargo check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` under `crates/health/` (max 3 repair iterations).

## Review

- [ ] Run the standards-review leg; leave `crates/health/REVIEW.md` beside the crate.
