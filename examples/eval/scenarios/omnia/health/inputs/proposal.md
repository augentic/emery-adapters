# health — Proposal

## Why

Operators want the smallest possible Omnia create-mode build to prove the target's generation → verify-repair → review → report loop: one HTTP GET operation, one Config key, one domain crate, and guest wiring.

## What Changes

- New `health` domain crate under `crates/health/` with a single `HealthCheck` operation.
- Guest HTTP route `GET /health` projecting the operation's typed response.
- No outbound HTTP, messaging, or persistence — Config only.

## Domains

### New Domains

- **health** — returns service liveness and the configured service name.

### Modified Domains

None — this is a greenfield slice.

## Impact

- Greenfield Cargo workspace authored by the build (no pre-existing `crates/` tree).
- One Config key (`SERVICE_NAME`) documented in `.env.example`.
