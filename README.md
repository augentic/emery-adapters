# Emery Adapters

[![CI](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml/badge.svg)](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

This repository houses the first-party WebAssembly (Wasm) source and target adapters for [Emery](https://github.com/augentic/emery). It is intended for contributors authoring, debugging, or evaluating Emery adapters.

## Contents

- [Do you need this repository?](#do-you-need-this-repository)
- [What an adapter is](#what-an-adapter-is)
  - [Source adapters](#source-adapters)
  - [Target adapters](#target-adapters)
- [Rust test loop](#rust-test-loop)
- [Live examples](#live-examples)
- [Graded live eval](#graded-live-eval)
- [Adapter development loop](#adapter-development-loop)
- [Troubleshooting](#troubleshooting)
- [Further reading](#further-reading)
- [License](#license)

## Do you need this repository?

- **Using Emery in a project?** You do not need to clone or build this repository. Reference any published adapter in your `emery.toml` (for example, `emery:documentation@0.12.0`). The `emery` CLI automatically downloads published releases from GitHub Container Registry (`ghcr.io/augentic/emery/<name>:<version>`) and caches them in `~/.emery/adapters`. See the [Emery documentation](https://emery.augentic.io/) to get started.
- **Authoring or debugging an adapter?** Use this workspace to edit extraction or build prompts and Rust survey/scaffolding code, run the deterministic test suites, test live runs against `examples/`, and benchmark prompt changes with `evals/`. To test a local build in an external project, copy the compiled `.wasm` artifact to `~/.emery/adapters/emery_<name>@<version>-dev.wasm`.

All adapters in this workspace share a single semantic version defined in `[workspace.package].version` in the root `Cargo.toml`.

## What an adapter is

Every adapter is a Rust crate under `adapters/<kind>` that compiles to a standalone `wasm32-wasip2` WebAssembly System Interface (WASI) component implementing the `emery:adapter` WebAssembly Interface Types (WIT) contract. Adapters are stateless and never manage revision storage or Git history directly.

### Source adapters

A source adapter implements the `source-adapter` world (`metadata` and `extract`). During `emery specify` or `emery amend`, the engine invokes `extract` for each configured source and reconciles the returned `Evidence` claims into a unified specification, design, and build plan.

| Crate | Source kind | Default rank | What it extracts |
| --- | --- | --- | --- |
| `adapters/intent` | `intent` | `1` (highest) | Operator briefs and change directives as `requirement`, `criterion`, `decision`, and `retire` claims. |
| `adapters/documentation` | `documentation` | `2` | Written specifications, guides, and Architecture Decision Records (ADRs) as `requirement`, `criterion`, `decision`, and `retire` claims. |
| `adapters/typescript` | `behaviour` | `3` | TypeScript and JavaScript codebases as `requirement` and `criterion` claims, plus `type`, `contract`, `call`, and `excerpt` claims. |
| `adapters/python` | `behaviour` | `3` | Python codebases (FastAPI, Flask, Django, Click, Celery, and libraries) as `requirement` and `criterion` claims, plus `type`, `contract`, `call`, and `excerpt` claims. |

### Target adapters

A target adapter implements the `target-adapter` world (`metadata` and `run` over `init`, `build`, and `verify`). During `emery build`, the engine initializes the target layout once, builds each slice in an isolated Git worktree, merges slices using the adapter's merge rules, and verifies each integrated wave.

| Crate | Target output | Layout and verification |
| --- | --- | --- |
| `adapters/node` | Strict TypeScript on Node.js (`>= 22.18`) | Initializes root `package.json`, `tsconfig.json`, and `.gitignore`. Builds one workspace package per slice under `src/<stem>/` with `node:test` suites under `test/`. Verifies waves with `npm install`, `npx tsc --noEmit`, and `npm test`. |
| `adapters/omnia` | [Omnia](https://github.com/augentic/omnia) WASI HTTP & messaging guest in Rust | Initializes the [`omnia-template`](https://github.com/augentic/omnia-exemplar) workspace. Scaffolds one member crate per stem under `crates/<stem>/` with `axum` routes, messaging consumers, and `#[tokio::test]` suites. Verifies waves with `cargo clippy`, `cargo test`, and `cargo build --target wasm32-wasip2`. |

## Rust test loop

The automated test suites run offline and require no model API keys. All integration tests live in the root `emery-adapters` package:

- `tests/source.rs` and `tests/target.rs` test each adapter's native Rust implementation against a scripted model double.
- `tests/component.rs` runs every compiled `.wasm` component inside the Omnia Wasmtime runtime (`crates/test-programs` compiles the components automatically on the first test run).
- `tests/probe.rs` tests the WIT boundary and SDK error handling using fixture probe components.
- `tests/prose.rs` validates every adapter's embedded Markdown prompt corpus, relative links, and JSON worked examples.

```bash
make ci                                               # full CI gate: fmt-check, lint (host + wasm32), test, test-docs, docs, vet, deny
cargo nextest run -p emery-adapters                   # all root suites: source, target, component, probe, and prose
cargo nextest run -p emery-adapters --test source     # native source adapter tests
cargo nextest run -p emery-adapters --test target     # native target adapter tests
cargo nextest run -p emery-adapters --test component  # compiled Wasm component tests under the runtime
```

## Live examples

Each directory under `examples/` contains an `emery.toml` configuration and sample inputs for running compiled adapters live through the `emery` CLI and Cursor backend. Running live examples requires a sibling `../emery` checkout (or installed `emery` binary), `cursor-sdk-bridge` on `PATH`, and `CURSOR_API_KEY`. See [examples/README.md](examples/README.md) for the complete walkthrough.

```bash
# 1. Build all Wasm adapter components in release mode
cargo build --workspace --target wasm32-wasip2 --release

# 2. Copy the built component into the local adapter cache
mkdir -p ~/.emery/adapters
cp target/wasm32-wasip2/release/emery_documentation.wasm ~/.emery/adapters/emery_documentation@0.12.0.wasm

# 3. Extract and stage a specification, then inspect it
emery specify --config examples/documentation/emery.toml
emery show spec --staged
```

## Graded live eval

The `evals/` crate is an operator-run benchmark harness that spawns the `emery` binary against real-world and synthetic fixtures in `evals/cases/`. It runs `emery specify` multiple times per case, grades recall, surface discovery, authority status, and technology leaks against `expected.toml`, and optionally runs `emery apply` and `emery build` (`--build`) to verify generated code. See [evals/README.md](evals/README.md) for details.

```bash
cargo run -p evals -- orders express-orders                  # grade specific cases
cargo run -p evals -- python                                 # grade all cases for one source adapter
cargo run -p evals -- --build express-orders                 # specify, apply, and build through emery:node
cargo run -p evals -- --build --target omnia express-orders  # specify, apply, and build through emery:omnia
```

## Adapter development loop

1. Edit the adapter's prompts under `adapters/<kind>/prose/` or its Rust survey/scaffold logic under `adapters/<kind>/src/`.
2. Run `cargo nextest run -p emery-adapters` to verify prompt links, worked examples, and native/component behavior.
3. Rebuild the Wasm component and copy it to `~/.emery/adapters` to test live:

```bash
cargo build -p emery-<kind> --target wasm32-wasip2 --release
cp target/wasm32-wasip2/release/emery_<kind>.wasm ~/.emery/adapters/emery_<kind>@0.12.0.wasm
emery specify --config examples/<kind>/emery.toml
```

## Troubleshooting

| Symptom | Resolution |
| --- | --- |
| `make` reports `mise` is not on `PATH` | Install [mise](https://mise.jdx.dev/getting-started.html). The root `Makefile` delegates tasks to `mise.toml` and does not install `mise` automatically. |
| `make fmt` fails | Install the nightly Rust formatter: `rustup toolchain install nightly --component rustfmt`. |
| The first `make test` takes a long time | `crates/test-programs/build.rs` compiles all Wasm components for `wasm32-wasip2` on first run; subsequent runs are incremental. |
| Cargo patch resolution errors | Committed `[patch.crates-io]` entries fetch `augentic/emery` from Git. Uncomment the local `path = "../emery/..."` lines only when co-developing with a sibling `../emery` checkout. |

Report bugs or questions in [GitHub Issues](https://github.com/augentic/emery-adapters/issues).

## Further reading

- Contributor setup, prompt rules, engine pinning, and release publishing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Live walkthrough (`specify`, `apply`, `build`, `amend`): [examples/README.md](examples/README.md)
- Live evaluation harness and scorecard metrics: [evals/README.md](evals/README.md)
- Workspace architecture and coding invariants: [AGENTS.md](AGENTS.md)

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option.
