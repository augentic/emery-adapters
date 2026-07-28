# Guest Wiring

How domain crates are wired into the WASM guest package at `$GUEST_PATH` (`guests/<service>/`). Applies when that package exists.

Compiling reference: the exemplar checkout's `guests/typed/src/lib.rs` (preferred) and `guests/axum/src/lib.rs` (escape hatch). Navigation: [`exemplar.md`](exemplar.md). Export patterns: [`guest-patterns.md`](guest-patterns.md). Layout: [`project-layout.md`](project-layout.md).

## Architecture

- Domain crates under `crates/*` hold `Operation<P>` implementations and depend only on `omnia-guest` capability traits.
- The guest constructs a unit `Provider` with default WASI capability impls (`Config`, `HttpRequest`, `Identity`, `Publish`, `StateStore`, … as needed), builds typed routers, and exports HTTP / messaging / WebSocket via the WASI export macros.
- Routes and topics should come from a single catalog (the exemplar uses `acme_common::routes`) so producers, consumers, and the guest stay aligned.

## Injecting a new crate

1. Add a path dependency on the crate in `$GUEST_PATH/Cargo.toml`.
2. Import the operation types in `$GUEST_PATH/src/lib.rs`.
3. Register each surface on the typed HTTP and/or messaging router (exact topics; no substring match in the preferred style).
4. Extend `Provider` / `AppProvider` bounds only when the new operations need additional capabilities.
5. Document new `Config::get` keys in `examples/.env.example`.

In update mode the crate writer performs these steps; the dedicated guest-writer phase runs only in create mode.
