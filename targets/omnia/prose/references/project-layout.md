# Project Structure

Directory layout for an Omnia guest workspace. The compiling reference is the exemplar checkout — see [`exemplar.md`](exemplar.md); prefer `guests/typed/` over inventing a different shape.

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
├── guests/
│   └── <service>/           # preferred: one typed-router guest package
│       ├── src/lib.rs       # WASI exports + routers + Provider
│       ├── examples/
│       │   ├── runner.rs    # omnia::runtime! host
│       │   └── .env.example
│       └── Cargo.toml
├── supply-chain/            # cargo-vet (scaffold prelude + cargo vet)
├── Cargo.toml               # workspace: members = ["crates/*", "guests/*"]
├── Makefile / Makefile.toml
├── deny.toml
├── rust-toolchain.toml
├── rustfmt.toml
├── clippy.toml
└── taplo.toml
```

Create mode authors **one** guest under `guests/<service>/` using the typed-router style. The exemplar's `guests/axum/` is an escape-hatch teaching surface — do not generate a second guest style unless `design.md` requires transport-level control the typed router cannot express.

## Key files

| Path | Purpose |
| ---- | ------- |
| `crates/<crate>/` | Domain operations; no WASI exports |
| `guests/<service>/src/lib.rs` | HTTP / messaging / WebSocket exports, routers, Provider |
| `guests/<service>/examples/runner.rs` | Native host via `omnia::runtime!` |
| `Cargo.toml` | Workspace members, lints, shared dependencies |

## Scaffolded tooling

Standard tooling files are written by the adapter's deterministic scaffold prelude when absent (policy table in [`configuration.md`](configuration.md)). Bodies live in the exemplar's `templates/guest/`; the exemplar root **is** the rendered set. Never re-author them from prose.

## File ownership

| Scope | Responsibility |
| ----- | -------------- |
| `crates/*` | Crate writer / test writer |
| `guests/<service>/` | Guest writer (create mode); crate writer for route/topic updates |
| Workspace `Cargo.toml` | Guest writer (create) / crate writer (members + deps) |
| Tooling + workflows | Scaffold prelude (fill-only); guest writer fills `publish.yaml` tokens |
