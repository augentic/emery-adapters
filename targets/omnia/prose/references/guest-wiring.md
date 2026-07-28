# Guest Wiring

How domain crates are wired into the WASM guest at the workspace root (`src/lib.rs`). Applies when that package exists.

Compiling reference: the exemplar checkout's `src/lib.rs` (preferred Axum guest). Typed routers are a documented fallback only — see [`guest-patterns.md`](guest-patterns.md). Navigation: [`exemplar.md`](exemplar.md). Layout: [`project-layout.md`](project-layout.md).

## Architecture

- Domain crates under `crates/*` hold `Operation<P>` implementations and depend only on `omnia-guest` capability traits.
- The root guest package constructs a unit `Provider` with default WASI capability impls (`Config`, `HttpRequest`, `Identity`, `Publish`, `StateStore`, … as needed), registers Axum routes and exact messaging topics, and exports HTTP / messaging / WebSocket via the WASI export macros.
- Routes and topics should come from a single catalog (the exemplar uses `acme_common::routes`) so producers, consumers, and the guest stay aligned.

## Injecting a new crate

1. Add a path dependency on the crate in the root `Cargo.toml` (the guest package).
2. Import the operation types in `src/lib.rs`.
3. Register each surface: Axum `.route(…)` for HTTP and an exact-topic arm in the messaging export (no substring match).
4. Extend `Provider` bounds only when the new operations need additional capabilities.
5. Document new `Config::get` keys in `.env.example` (and keep `examples/runtime.rs` hosts in sync).

In update mode the crate writer performs these steps; the dedicated guest-writer phase runs only in create mode. If the consumer already uses a legacy `guests/<service>/` package, inject there instead of relocating the guest.
