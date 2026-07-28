# Provider Patterns

Per-trait notes for provider composition. Compiling usage — production operations, guest provider construction, and mock providers — lives in the exemplar checkout ([`exemplar.md`](../exemplar.md)); this document keeps only the selection semantics and constraints code cannot show. Trait definitions and method signatures: [`capabilities.md`](../capabilities.md).

## Trait summary

| Trait | When required | Compiling demonstration (exemplar checkout) |
| --- | --- | --- |
| `Config` | Always — all components read configuration | Transit crates throughout; key catalog in `crates/common/` |
| `HttpRequest` | Component makes HTTP calls | `crates/gtfs-adapter/` (clients in `crates/common/`); mocks in `crates/*/tests/` |
| `Publish` | Component publishes messages/events | `crates/tally-connector/` (minimal); mocks in its `tests/` |
| `Identity` | Any HTTP call uses Bearer authentication | `crates/gtfs-adapter/`, `crates/pulse-adapter/` |
| `StateStore` | Caching or key-value storage | `crates/gtfs-adapter/` (`state_keys.rs` for key discipline) |
| `Broadcast` | Handler pushes data to WebSocket clients | `crates/capability-examples/` (operation + mock) |
| `BlobStore` | Binary blob storage | `crates/capability-examples/` (operation + mock) |
| `DocumentStore` | JSON document storage / queries | `crates/capability-examples/` (operation + mock) |
| `TableStore` | Relational (SQL) storage | `crates/capability-examples/` (operation + mock) |

## Composition semantics

- **Minimal bounds.** Each function takes `provider: &P` with only the traits it actually calls; an `Operation<P>` bound is the union of the bounds of everything it calls. Fewer bounds mean fewer mock traits and self-documenting I/O.
- **Owner.** Every transport router in a guest shares one `Invoker::new(owner, provider)`; the owner is the tenant that owns the deployment and must be consistent across routers.
- **Startup config validation.** Use `ensure_env!` in `Provider::new()` only when the guest requires environment variables at startup; otherwise omit it and make `Provider::new()` a `const fn`.
- **Bearer authentication.** When any HTTP call needs a token: read the identity name via `Config::get`, fetch the token via `Identity::access_token`, attach it as the `Authorization: Bearer` header — so the bounds become at least `P: Config + Identity + HttpRequest`.

## Rules

1. **Never construct host-side types** — no `Client::new()`, `RedisClient::connect()`, `Producer::new()`, etc.
2. **Never create I/O abstractions** — do not wrap provider traits in custom abstractions.
3. **Never call raw WASI modules from domain crates** — only `omnia_guest` traits; raw WASI calls belong in boundary/provider code.
4. **All state is explicit** — no caching, memoization, or global state; state flows through parameters and provider calls. Mock providers share state through `Arc<Mutex<…>>` fields, never `static` cells.
5. **Config, not env vars** — `Config::get(provider, "KEY")`, never `std::env::var("KEY")`.

## Selecting traits from artifacts

[`capability-mapping.md`](../capability-mapping.md) maps artifact-declared capabilities to Omnia traits; [`capabilities.md`](../capabilities.md#capability-selection-summary) carries the full selection table. Check the artifacts' "Source Capabilities Summary" and "External Service Dependencies" sections; for code-analysis artifacts, also derive traits from "Business Logic Blocks".
