# Emery Adapters

[![CI](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml/badge.svg)](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

First-party **source** Wasm components for [Emery](https://github.com/augentic/emery)'s specification generator: `documentation`, `intent`, and `typescript`.

**Using Emery in a project?** You do not need this repository. Name a published adapter by package reference (`emery:<name>@<version>`) or a built `.wasm` by path in the project's `emery.toml`; follow the [Emery README](https://github.com/augentic/emery#readme).

**Authoring or debugging an adapter?** This repo is your home. Edit prose or Rust, run the crate and seam tests, then walk the adapter live with its example.

The version operators pin (`documentation@0.13.0`) is this workspace's shared SemVer (`[workspace.package].version`); published components live on GHCR.

## What an adapter is

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world (`extract` + `metadata`). The engine names the sources on each `emery specify` invocation, dispatches one `extract` per source, and reconciles the returned Evidence documents into the typed specification and design that `emery show` projects as `spec.md` / `design.md`. Adapters never orchestrate lifecycle.

| Adapter | Authority | Extracts |
| --- | --- | --- |
| `sources/documentation` | documentation | written specs, guides, ADRs — requirement / criterion / decision claims |
| `sources/intent` | intent | the operator's brief, verbatim plus its directives as requirement claims |
| `sources/typescript` | behaviour | TS/JS estates — requirement claims backed by excerpt / type / call detail |

## Rust-only loop

Native tests need no model credentials. Each adapter carries its own extract suite; the root package carries the seam suites that run every built component under the omnia runtime (the components are built by `crates/test-programs` on the first `make test`), the seam's probes, and every adapter's corpus:

```bash
make check
cargo nextest run -p documentation   # or intent, typescript: the native extract suite
cargo nextest run -p emery-adapters  # the root seam suites: every component, the probes, the corpora
```

## Live examples

`examples/<name>/` is a root-package example cdylib plus an `emery.toml` that binds the built component by path and the fixture it reads. `cargo build --example <name> --target wasm32-wasip2 --release` produces the wasm; `emery specify` extracts through the Cursor model backend and commits the revision; `emery show spec` reviews it — the same journey an operator's project takes. Needs an `emery` binary, `cursor-sdk-bridge`, and `CURSOR_API_KEY`; see [examples/README.md](examples/README.md).

```bash
make release                                              # every component → target/wasm32-wasip2/release/
emery specify --config examples/documentation/emery.toml  # or intent, typescript; examples/emery.toml runs all three
emery show spec
```

## Graded live eval

The live rung is a **public-contract client**: it spawns the sibling shipped `emery` binary over the built components, drives one `specify` per case across the adapter contract, grades the committed spec via `emery show spec`, and writes the dated scorecard. Operator-invoked, never CI. It is being recreated as a root example beside the live examples, whose `emery.toml` files are its cases.

## Repair loop

1. Edit `sources/<name>/prose/**` (the extract prompt, references, rules).
2. `cargo nextest run -p <name>` to re-run its extract suite and `cargo nextest run -p emery-adapters` for its seam and corpus; `cargo build -p <name> --target wasm32-wasip2 --release` to rebuild the shipped component; `emery specify --config examples/<name>/emery.toml` to watch it become a spec.

Native crate tests stay the Rust inner loop; the seam suites prove the component boundary; the live examples show one adapter's claims becoming a specification; live eval is for prompt quality. See [docs/testing.md](docs/testing.md).

## Stuck?

| Symptom | What to check |
| --- | --- |
| `make fmt` fails | Install nightly rustfmt: `rustup toolchain install nightly --component rustfmt` |
| The first `make test` is slow | `crates/test-programs/build.rs` nested-builds every component for `wasm32-wasip2`; later builds are incremental |
| Patch-resolution errors after editing root `Cargo.toml` | The committed `[patch.crates-io]` git patches fetch `augentic/emery`; uncomment the path patches only when co-developing against `../emery` |

Bugs and questions: [GitHub Issues](https://github.com/augentic/emery-adapters/issues).

## Further reading

- Contributor setup, engine pin, publishing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Creating an adapter: [docs/authoring.md](docs/authoring.md)
- Test ownership: [docs/testing.md](docs/testing.md)
- Agent instructions: [AGENTS.md](AGENTS.md)

## License

MIT OR Apache-2.0.
