# Omnia build — guest writer

Loaded by [../build.md](../build.md) phase 4 on **first build only** (when `$GUEST_PATH/src/lib.rs` is absent). Subsequent builds skip this step; route / topic / WebSocket wiring updates are folded into [crate writer](crate.md)'s four-category cadence.

The guest is a thin WASI/wasm32 package under `guests/<service>/` — it owns HTTP routing, topic dispatch, WebSocket exports, provider setup, and config validation; ALL business logic lives in domain crates under `crates/`. Prefer the typed-router style; the exemplar's `guests/axum/` is escape-hatch only.

## Hard rules

- **Never put business logic in the guest.** All domain logic lives in the project's domain crates; the guest is wiring only.
- **Gate the guest with `#![cfg(target_arch = "wasm32")]`** — wasm32 is the only supported target.
- **Forbid `std::env`, `std::fs`, `std::net`, `std::thread::spawn`** in guest code. All I/O routes through provider traits; configuration via `omnia_guest::Config`. Async only — no blocking operations.
- **Use typed transport routers.** Register HTTP operations with `api::http::Router` and exact messaging topics with `api::messaging::Router`; missing or unhandled topics return errors.
- **Export every transport explicitly.** Use the HTTP, messaging, WebSocket, and command export macros only for interfaces this component exposes.
- **Use one shared invoker per router.** Construct `Invoker::new("owner", provider)` and let it supply `CallContext`.
- **Keep projection at the boundary.** HTTP status/body/error and messaging acknowledgement/retry policy belong in transport projectors.
- **Axum 0.8 route params use `{param}` brace syntax**, never `:param`.

The full constraint list lives at [`guardrails.md`](../../references/guardrails.md) and [`wasm-constraints.md`](../../references/wasm-constraints.md).

## Process

The adapter's deterministic scaffold prelude has already written every missing tooling file the exemplar checkout's template manifest declares (toolchain, fmt/clippy/taplo config, Makefiles, deny/vet scaffold, `.gitignore`, editor settings, and the GitHub workflows) before this leg runs. Do not re-author or overwrite them; the generation user prompt's `### scaffold prelude` block lists exactly what was written. [`configuration.md`](../../references/configuration.md) describes each file.

1. **Lay down the workspace `Cargo.toml`** per [`configuration.md`](../../references/configuration.md) — `members = ["crates/*", "guests/*"]`, `[workspace.package]`, `[workspace.dependencies]`, lints, and the release profile. Adopt the Omnia pin from the exemplar checkout's `exemplar.yaml`.
2. **Generate `$GUEST_PATH/`** (`Cargo.toml`, `src/lib.rs`, `examples/runner.rs`, `examples/.env.example`) with explicit WIT exports and typed HTTP / messaging / command router assembly over the shared operation kernel. Pattern catalogue: [`guest-patterns.md`](../../references/guest-patterns.md); the exemplar checkout's `guests/typed/` is the compiling reference — see [`exemplar.md`](../../references/exemplar.md). Layout: [`project-layout.md`](../../references/project-layout.md).
3. **Implement the `Provider` struct** that satisfies the consumed `omnia-wasi-*` adapter traits. Validate every required `Config::get` key in `Provider::new()` and document each in `examples/.env.example`. Injection contract: [`guest-wiring.md`](../../references/guest-wiring.md).
4. **Author `examples/runner.rs`** with the `omnia::runtime!({ main: true, hosts: { … } });` block enumerating every WASI host the guest consumes. See [`runtime.md`](../../references/runtime.md).
5. **Finish the scaffolded supply-chain and publish config**: fill the `<PACKAGE_NAME>` / `<STORAGE_ACCOUNT>` / `<RESOURCE_GROUP>` placeholders in `.github/workflows/publish.yaml`, and after the workspace builds for the first time and produces `Cargo.lock`, run `cargo vet regenerate {imports,exemptions,unpublished}` to populate `supply-chain/imports.lock` and the exemptions in `supply-chain/config.toml`.
6. **Verify with `cargo check`** — fix any missing route / provider impl / wasm32-incompatible usage and re-check until clean. The build prompt's verify-repair loop runs after this step.

## When `WasiIdentity` is consumed

Identity needs OAuth2 credentials wired through `Config`. Add `IDENTITY_CLIENT_ID`, `IDENTITY_CLIENT_SECRET`, `IDENTITY_TOKEN_URL` to `.env.example` and assert their presence in `Provider::new()`. Env contract: [`runtime.md`](../../references/runtime.md); compiling integration: the exemplar's `crates/gtfs-adapter/` and `guests/typed/`.
