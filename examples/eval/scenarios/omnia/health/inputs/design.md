# health — Design

## Domain model

- `HealthStatus` — response value object with fields `status: String` (`"ok"` on success) and `service: String` (configured name).
- No path/query input DTO: `HealthCheck` takes unit input (`()`).

## Provider trait dependencies

| Operation     | Provider traits |
| ------------- | --------------- |
| `HealthCheck` | `Config`        |

No `HttpRequest`, `Publish`, `StateStore`, `Identity`, `TableStore`, `Broadcast`, `BlobStore`, or `DocumentStore`.

## APIs / Integrations

| Surface       | Transport                         | Operation     |
| ------------- | --------------------------------- | ------------- |
| Liveness ping | HTTP `GET /health` (typed router) | `HealthCheck` |

Guest projector maps `HealthStatus` → JSON 200; maps `ServerError { code: config_missing }` → HTTP 500 with the Omnia error envelope.

## Configuration

| Key            | Used by       | Purpose                                      |
| -------------- | ------------- | -------------------------------------------- |
| `SERVICE_NAME` | `HealthCheck` | Value returned as `HealthStatus.service`     |

Documented in workspace `.env.example`.

## Technical logic

### `HealthCheck`

- Zero-sized `HealthCheck` implementing `Operation<P>` where `P: omnia_guest::api::Provider + Config`.
- `Input = ()`, `Output = HealthStatus`, `Error = omnia_guest::Error`.
- `call`:
  1. Structural validation: none (unit input).
  2. `Config::get(provider, "SERVICE_NAME").await` with `.context(...)`; on missing/error map to `ServerError { code: "config_missing", ... }` via domain error `HealthError::ConfigMissing` and `From<HealthError> for omnia_guest::Error`.
  3. Return `HealthStatus { status: "ok".into(), service }`.
- Serde on `HealthStatus`: `#[serde(rename_all = "camelCase")]`.
- Metric: `tracing::info!(monotonic_counter.health_check = 1)` on success.

### Crate layout

```text
crates/health/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── types.rs
│   └── operations/health.rs
└── tests/
    ├── provider.rs
    └── health.rs
```

Guest root (`src/lib.rs` + workspace `Cargo.toml`) registers the typed HTTP route for `HealthCheck`.

## Error mapping

| Condition                 | Domain variant                 | `omnia_guest::Error`                          |
| ------------------------- | ------------------------------ | --------------------------------------------- |
| `SERVICE_NAME` unavailable | `HealthError::ConfigMissing`   | `ServerError` / code `config_missing`         |

No `BadRequest` / `NotFound` / `BadGateway` paths in this slice.

## Implementation constraints

- Create mode: no pre-existing `crates/health/Cargo.toml`.
- WASM Preview 2 guardrails: no `std::env::var`, no forbidden crates; Config is the only host I/O.
- No `unwrap()` / `expect()` in production paths.
