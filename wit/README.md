# Vendored `specify:adapter` WIT

[augentic/specify](https://github.com/augentic/specify) owns and publishes the adapter contract as the wasm-pkg package `specify:adapter` (from its `wit/specify.wit`, on engine release tags). This repo only consumes it: `deps/specify/specify.wit` is a vendored copy of the pinned published version, and no contract change ever lands here first.

## Layout and pin

- **Vendored copy**: `wit/deps/specify/specify.wit` (conventional wasm-tools deps layout). [`adapter`](../crates/adapter/) generates the `source-adapter` and `target-adapter` world bindings from it once (`crates/adapter/src/{source,target}.rs`); each adapter-root crate hand-writes a thin shim that implements the generated `Guest` trait and wires it in with `export!(Adapter with_types_in adapter::{source,target})`. The eval guest (`evals/guest.rs`) generates the import-only `workflow` world from the same directory.
- **Pin**: the version lives in exactly one place — the `WIT_PIN` env var at the top of [`Makefile.toml`](../Makefile.toml). Every task (`wit-vendor`, `check-pins`) reads it; nothing else declares a version.
- **Registry routing**: [`.wkg-config.toml`](./.wkg-config.toml) maps the `specify:` namespace to `augentic.io`, whose `/.well-known/wasm-pkg/registry.json` resolves to the backing OCI registry. Pulls are anonymous.

## Refreshing the vendored copy

Requires [wkg](https://github.com/bytecodealliance/wasm-pkg-tools) (`cargo install wkg --locked`).

```bash
cargo make wit-vendor
```

Fetches `specify:adapter@<WIT_PIN>` to a temp file and moves it into place on success — a failed fetch (unpublished pin, unreachable registry) leaves the vendored copy untouched. To adopt a new contract version: bump `WIT_PIN` in `Makefile.toml`, run `wit-vendor`, commit both.

`cargo make check-pins` verifies the vendored bytes match the pinned published version. Until the first `specify:adapter` publish lands (it rides the next engine release tag), the fetch cannot succeed and the check skips with a notice naming the pin — that is the expected pre-first-publish posture, and it keeps CI green on runners with no registry access.

## Dev loop: sibling override

While iterating on a contract change in the engine repo *before* the new version is published, point the build at the sibling checkout's file:

```bash
cargo make wit-vendor-sibling   # copies ../specify/wit/specify.wit into wit/deps/specify/
```

This is a dev-loop convenience only. The published pin is the release posture: once the engine publishes the new `specify:adapter` version, bump `WIT_PIN` and run `cargo make wit-vendor` so the vendored copy is pinned to published bytes again.
