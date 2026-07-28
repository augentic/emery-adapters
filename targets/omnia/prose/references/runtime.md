# Runtime Setup

Local development runtime configuration via the `omnia::runtime!` macro in `examples/runner.rs` (or `examples/<guest-name>.rs`).

Compiling reference: the exemplar checkout's `guests/typed/examples/runner.rs` — a `cfg_if` split (`wasm32` gets an empty `main`; native gets the `omnia::runtime!({ hosts: { … } })` block) enumerating every WASI host the guest consumes. Navigation: [`exemplar.md`](exemplar.md). Provider configuration and trait composition: [providers/README.md](providers/README.md).

## WASI host options

Enumerate exactly the hosts backing the capability traits the guest's provider implements:

| Host | Default | Backs |
| --- | --- | --- |
| `WasiConfig` | `ConfigDefault` | `Config` — environment variable access |
| `WasiHttp` | `HttpDefault` | `HttpRequest` — HTTP client requests |
| `WasiIdentity` | `IdentityDefault` | `Identity` — authentication tokens |
| `WasiKeyValue` | `KeyValueDefault` | `StateStore` — cache/KV storage |
| `WasiMessaging` | `MessagingDefault` | `Publish` — message pub/sub |
| `WasiOtel` | `OtelDefault` | OpenTelemetry tracing (build-time, no trait) |
| `WasiSql` | `SqlDefault` | `TableStore` — database connections |
| `WasiWebSocket` | `WebSocketDefault` | `Broadcast` — WebSocket event handling |
| `WasiBlobstore` | `BlobstoreDefault` | `BlobStore` — binary blob storage |
| `WasiJsonDb` | `JsonDbDefault` | `DocumentStore` — JSON document storage |

## Environment variables

Create `examples/.env.example` documenting every required `Config` key. Use module-level `RUST_LOG` filtering for debugging:

```bash
# Logging -- use module-level filtering for debugging
RUST_LOG="info,omnia_wasi_http=debug,omnia_wasi_messaging=debug,<guest-name>=debug"

# Service config
API_URL=https://api.example.com
SERVICE_NAME=my-service
```

### Identity environment variables

When `WasiIdentity` is enabled, the runtime requires OAuth2 credentials; include them in `.env.example` whenever the guest's Provider implements `Identity`:

```bash
IDENTITY_CLIENT_ID="<client_id>"
IDENTITY_CLIENT_SECRET="<client_secret>"
IDENTITY_TOKEN_URL="<token endpoint>"
```

## Running

```bash
source examples/.env.example && cargo run --example <guest-name>
```

## References

- [providers/README.md](providers/README.md) -- provider composition and per-trait notes
- [capabilities.md](capabilities.md) -- trait definitions and method signatures
- [guest-patterns.md](guest-patterns.md) -- guest export patterns (HTTP, Messaging, WebSocket)
