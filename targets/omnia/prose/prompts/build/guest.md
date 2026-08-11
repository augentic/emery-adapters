# Omnia build — guest writer

Loaded by [../build.md](../build.md) phase 4 on **first build only** (when `src/lib.rs` at the workspace root is absent). Subsequent builds skip this step; route / topic / WebSocket wiring updates are folded into [crate writer](crate.md)'s four-category cadence.

The guest is the **workspace root package** — thin WASI/wasm32 wiring in `src/lib.rs` over domain crates under `crates/`. It owns HTTP routing, topic dispatch, WebSocket exports, provider setup, and config validation; ALL business logic lives in `crates/`. Match the exemplar's root-package Axum guest (`src/lib.rs`); do not create a `guests/<service>/` directory.

Typed `omnia_guest::api` routers are a fallback only — see [`guest-patterns.md`](../../references/guest-patterns.md). Generate that style only when `design.md` explicitly requires it.

## Hard rules

- **Never put business logic in the guest.** All domain logic lives in the project's domain crates; the guest is wiring only.
- **Gate the guest with `#![cfg(target_arch = "wasm32")]`** — wasm32 is the only supported target.
- **Forbid `std::env`, `std::fs`, `std::net`, `std::thread::spawn`** in guest code. All I/O routes through provider traits; configuration via `omnia_guest::Config`. Async only — no blocking operations.
- **Prefer Axum + exact-topic messaging.** Register HTTP with an `axum::Router` served through `omnia_wasi_http::serve`; match messaging topics exactly. Missing or unhandled topics return errors.
- **Export every transport explicitly.** Use the HTTP, messaging, WebSocket, and command export macros only for interfaces this component exposes.
- **Use one shared invoker.** Construct `Invoker::new("owner", provider)` and invoke operations through it from each handler.
- **Keep projection at the boundary.** HTTP status/body/error and messaging acknowledgement/retry policy belong in the guest handlers (or typed projectors when using the typed-router fallback).
- **Axum 0.8 route params use `{param}` brace syntax**, never `:param`.

The full constraint list lives at [`guardrails.md`](../../references/guardrails.md) and [`wasm-constraints.md`](../../references/wasm-constraints.md).

## Process

The adapter's deterministic scaffold prelude has already written every missing tooling file the exemplar checkout's template manifest declares (toolchain, fmt/clippy/taplo config, Makefiles, deny/vet scaffold, `.gitignore`, editor settings, and the GitHub workflows) before this leg runs. Do not re-author or overwrite them; the generation user prompt's `### scaffold prelude` block lists exactly what was written and any still-unfilled `<TOKEN>` placeholders. [`configuration.md`](../../references/configuration.md) describes each file.

1. **Lay down the root `Cargo.toml`** per [`configuration.md`](../../references/configuration.md) — a root `[package]` for the guest, `members = ["crates/*", …]` (never `guests/*`), `[workspace.package]`, `[workspace.dependencies]`, lints, and the release profile. Adopt the Omnia pin from the exemplar checkout's `exemplar.yaml`.
2. **Generate the root guest** (`src/lib.rs`, `examples/runtime.rs`, `.env.example`) with explicit WIT exports, Axum HTTP routes, and exact-topic messaging over the shared `Invoker`. Pattern catalogue: [`guest-patterns.md`](../../references/guest-patterns.md); the exemplar checkout's `src/lib.rs` is the compiling reference — see [`exemplar.md`](../../references/exemplar.md). Layout: [`project-layout.md`](../../references/project-layout.md).
3. **Implement the `Provider` struct** that satisfies the consumed `omnia-wasi-*` adapter traits. Validate every required `Config::get` key in `Provider::new()` and document each in `.env.example`. Injection contract: [`guest-wiring.md`](../../references/guest-wiring.md).
4. **Author `examples/runtime.rs`** with the `omnia::runtime!({ hosts: { … } });` block enumerating every WASI host the guest consumes. See [`runtime.md`](../../references/runtime.md).
5. **Finish the scaffolded supply-chain and publish config**: fill every `<UPPER_SNAKE>` placeholder still listed in the scaffold prelude (typically `<PACKAGE_NAME>` / `<STORAGE_ACCOUNT>` / `<RESOURCE_GROUP>` in `.github/workflows/publish.yaml`), and after the workspace builds for the first time and produces `Cargo.lock`, run `cargo vet regenerate {imports,exemptions,unpublished}` to populate `supply-chain/imports.lock` and the exemptions in `supply-chain/config.toml`.
6. **Stop after writing** — do not run `cargo check` or fix-and-recheck: the engine dispatches the verification operation after the build and routes any missing route / provider impl / wasm32-incompatible usage through the repair operation.

## When `WasiIdentity` is consumed

Identity needs OAuth2 credentials wired through `Config`. Add `IDENTITY_CLIENT_ID`, `IDENTITY_CLIENT_SECRET`, `IDENTITY_TOKEN_URL` to `.env.example` and assert their presence in `Provider::new()`. Env contract: [`runtime.md`](../../references/runtime.md); compiling integration: the exemplar's `crates/gtfs-adapter/` and `src/lib.rs`.
