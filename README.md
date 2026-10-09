# Emery Adapters

[![CI](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml/badge.svg)](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

First-party Wasm components for [Emery](https://github.com/augentic/emery): the **source** adapters its specification generator reads — `documentation`, `intent`, `typescript`, and `python` — and the **target** adapter `emery build` builds through, `typescript-target`.

**Using Emery in a project?** You do not need this repository. Name a published adapter by package reference (`emery:<name>@<version>`) in the project's `emery.toml`; the `emery` binary fetches it on the first run that names it and reads it from `~/.emery/adapters` after. A build of your own goes there by `cp`, under a reference of its own (`~/.emery/adapters/emery_<name>@<version>-dev.wasm`); follow the [Emery README](https://github.com/augentic/emery#readme).

**Authoring or debugging an adapter?** This repo is your home. Edit prose or Rust, run the crate and component tests, then walk the adapter live with its example or the eval's build arm.

The version operators pin (`emery:documentation@0.13.0`) is this workspace's shared SemVer (`[workspace.package].version`); published components live on GHCR as `ghcr.io/augentic/emery/<name>:<version>`.

## What an adapter is

A source adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world (`extract` + `metadata`). The engine names the sources on each `emery specify` invocation, dispatches one `extract` per source, and reconciles the returned Evidence documents into the typed specification and design that `emery show` projects as `spec.md` / `design.md`. A target adapter exports the `target-adapter` world (`build` + `verify` + `metadata`): `emery build` hands it each slice of the plan with a working copy to build into, verifies each merged wave through it, and merges slices under the rules its metadata declares. Adapters never orchestrate lifecycle.

| Adapter                 | Kind          | Extracts                                                                                                                                |
| ----------------------- | ------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `sources/documentation` | documentation | written specs, guides, ADRs — requirement / criterion / decision claims                                                                 |
| `sources/intent`        | intent        | the operator's brief, verbatim plus its directives as requirement claims                                                                |
| `sources/typescript`    | behaviour     | TS/JS estates — requirement claims backed by excerpt / type / call detail                                                               |
| `sources/python`        | behaviour     | Python estates — FastAPI, Flask, Django, Click, Celery, and plain libraries — requirement claims backed by excerpt / type / call detail |

| Target                      | Builds                                                                                                                                                          |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `targets/typescript-target` | strict TypeScript on Node (22.18 or later): one module per slice under `src/<stem>/`, `node:test` tests per scenario, checked with `tsc --noEmit` and `npm test` |

## Rust-only loop

The suites need no model credentials. Every test is the root package's: the native suites call every adapter's `Adapter` over a scratch tree and a scripted model and assert what it decides, the component suite runs every built component under the omnia runtime once (the components are built by `crates/test-programs` on the first `make test`), the fixture probes prove the boundary, and every adapter's corpus is checked natively:

```bash
make ci                                               # exactly the CI gate: fmt-check, lint (host + wasm32), tests, doctests, docs, vet, deny
cargo nextest run -p emery-adapters                   # the root suites: every adapter, every component, the probes, the corpora
cargo nextest run -p emery-adapters --test source     # what the shipped source adapters decide, natively
cargo nextest run -p emery-adapters --test target     # what the shipped target adapters decide, natively
cargo nextest run -p emery-adapters --test component  # the shipped components alone, under the runtime
```

## Live examples

`examples/<name>/` is an `emery.toml` that names the shipped component as the package `emery:<name>@<version>-dev` — `cargo build --workspace --target wasm32-wasip2 --release` builds every one, and a `cp` into `~/.emery/adapters` under that reference stands it beside the published release — and the fixture it reads. `emery specify` extracts through the Cursor model backend and commits the revision; `emery show spec` reviews it — the same journey an operator's project takes. Needs an `emery` binary, `cursor-sdk-bridge`, and `CURSOR_API_KEY`; see [examples/README.md](examples/README.md).

```bash
cargo build --workspace --target wasm32-wasip2 --release   # every component
mkdir -p ~/.emery/adapters
cp target/wasm32-wasip2/release/documentation.wasm ~/.emery/adapters/emery_documentation@0.13.0.wasm   # one per adapter the example names
emery specify --config examples/documentation/emery.toml  # or intent, typescript, python; examples/emery.toml runs all four
emery show spec
```

## Graded live eval

The live rung is a **public-contract client**: `evals/` spawns the sibling shipped `emery` binary over the built `typescript` or `python` component a case names, drives one `specify` per case across the adapter contract, grades the accepted claims and the surveyed surfaces against the case's `expected.toml` through `emery show`, and writes the dated scorecard. `--build` follows each graded `specify` with one `emery build` through the built `typescript-target` into a bare repository the harness lays, and the card reads what was built, verified, and merged. Operator-invoked, never CI; see [evals/README.md](evals/README.md).

```bash
cargo run -p evals -- orders express-orders   # the named cases; none names every case whose fixture the checkout holds
cargo run -p evals -- python                  # every case under one adapter
cargo run -p evals -- --build express-orders  # specify, then build the plan through typescript-target
```

## Repair loop

1. Edit `sources/<name>/prose/**` (the extract prompt, references, rules) or `targets/<name>/prose/**` (the build and verify prompts, the layout).
2. `cargo nextest run -p emery-adapters` to re-run its suites; `cargo build -p <name> --target wasm32-wasip2 --release` to rebuild the shipped component and `cp` it over the `-dev` file in `~/.emery/adapters`; `emery specify --config examples/<name>/emery.toml` to watch it become a spec.

The native suites are the Rust inner loop and prove every adapter's own decisions, the component suite proves every built component once; the live examples show one adapter's claims becoming a specification; live eval is for prompt quality.

## Stuck?

| Symptom                                                 | What to check                                                                                                                               |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `make` says mise is not on `PATH`                       | Install [mise](https://mise.jdx.dev/getting-started.html); the `Makefile` forwards to it and never installs it                              |
| `make fmt` fails                                        | Install nightly rustfmt: `rustup toolchain install nightly --component rustfmt`                                                             |
| The first `make test` is slow                           | `crates/test-programs/build.rs` nested-builds every component for `wasm32-wasip2`; later builds are incremental                             |
| Patch-resolution errors after editing root `Cargo.toml` | The committed `[patch.crates-io]` git patches fetch `augentic/emery`; uncomment the path patches only when co-developing against `../emery` |

Bugs and questions: [GitHub Issues](https://github.com/augentic/emery-adapters/issues).

## Further reading

- Contributor setup, engine pin, publishing: [CONTRIBUTING.md](CONTRIBUTING.md)
- The adapter shape, invariants, and test ownership: [AGENTS.md](AGENTS.md)

## License

MIT OR Apache-2.0.
