# Project Structure

Directory layout for an Omnia guest workspace. The compiling reference is the exemplar checkout — see [`exemplar.md`](exemplar.md). Match its root-package guest shape; do not invent a `guests/<service>/` tree.

## Directory Tree

```text
<workspace>/
├── .github/workflows/       # audit, ci, patch, publish, release (scaffold prelude)
├── .vscode/settings.json    # rust-analyzer wasm32 config (scaffold prelude)
├── crates/                  # domain Operation<P> libraries
│   └── <crate>/
│       ├── src/
│       ├── tests/           # native mock-provider tests
│       └── data/            # fixtures (optional)
├── src/
│   └── lib.rs               # WASM guest: HTTP / messaging / exports + Provider
├── examples/
│   └── runtime.rs           # omnia::runtime! host
├── .env.example             # Config keys the guest validates at startup
├── supply-chain/            # cargo-vet (scaffold prelude + cargo vet)
├── Cargo.toml               # root [package] is the guest; members = ["crates/*", …]
├── Makefile / Makefile.toml
├── deny.toml
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
└── taplo.toml
```

Create mode authors **one** guest as the workspace root package (`src/lib.rs`, hand-written Axum + exact-topic messaging), matching the exemplar. Domain logic stays under `crates/`. Do not create a `guests/` directory.

Typed `omnia_guest::api` routers are a documented fallback only — see [`guest-patterns.md`](guest-patterns.md). Generate them only when `design.md` explicitly requires that style; the exemplar does not ship a compiling typed guest.

Update mode: if the consumer already has a non-root guest layout (for example a legacy `guests/<service>/` package), preserve that layout — existing crate code outranks the exemplar for packaging (see [`hard-rules.md`](hard-rules.md)).

## Key files

| Path | Purpose |
| ---- | ------- |
| `crates/<crate>/` | Domain operations; no WASI exports |
| `src/lib.rs` | HTTP / messaging / WebSocket exports, Axum routes, Provider |
| `examples/runtime.rs` | Native host via `omnia::runtime!` |
| `Cargo.toml` | Root guest package + workspace members, lints, shared dependencies |

## Scaffolded tooling

Standard tooling files are written by the adapter's deterministic scaffold prelude when absent (policy table in [`configuration.md`](configuration.md)). Bodies live in the exemplar's `templates/guest/`; the exemplar root **is** the rendered set. Never re-author them from prose.

## File ownership

| Scope | Responsibility |
| ----- | -------------- |
| `crates/*` | Crate writer / test writer |
| `src/lib.rs`, `examples/runtime.rs`, root guest `Cargo.toml` fields | Guest writer (create mode); crate writer for route/topic updates |
| Workspace tables in root `Cargo.toml` | Guest writer (create) / crate writer (members + deps) |
| Tooling + workflows | Scaffold prelude (fill-only); guest writer fills `publish.yaml` tokens |
