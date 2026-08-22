# Emery Adapters

[![CI](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml/badge.svg)](https://github.com/augentic/emery-adapters/actions/workflows/ci.yaml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

First-party **source** Wasm components for [Emery](https://github.com/augentic/emery)'s specification generator: `documentation`, `intent`, and `typescript`.

**Using Emery in a project?** You do not need this repository. Bind a built `.wasm` or declare it as a static guest in the host runtime; follow the [Emery README](https://github.com/augentic/emery#readme).

**Authoring or debugging an adapter?** This repo is your home. Edit prose or Rust, run the crate tests, then run a graded live eval case.

The version operators pin (`documentation@0.13.0`) is this workspace's shared SemVer (`[workspace.package].version`); published components live on GHCR.

## What an adapter is

An adapter is one Rust crate that ships as one Wasm component exporting the `source-adapter` world (`extract` + `metadata`). The engine binds the sources named on each `emery specify` invocation, dispatches one `extract` per binding, and reconciles the returned Evidence documents into `spec.md` / `design.md`. Adapters never orchestrate lifecycle.

| Adapter | Authority | Extracts |
| --- | --- | --- |
| `sources/documentation` | documentation | written specs, guides, ADRs — requirement / criterion / decision claims |
| `sources/intent` | intent | the operator's brief, verbatim plus its directives as requirement claims |
| `sources/typescript` | behaviour | TS/JS estates — requirement claims backed by excerpt / type / call detail |

## Rust-only loop

Native crate tests need no model credentials:

```bash
cargo make check
cargo nextest run -p documentation   # or intent, typescript
```

## Graded live eval

The live rung is a **public-contract client**: it spawns the sibling shipped `emery` binary over the built components, drives one `specify` per case across the component seam, grades the committed spec via `emery show spec`, and writes the dated scorecard. Operator-invoked, never CI.

```bash
cargo make release                   # build the components
# build the sibling binary: cargo build --release --bin emery (in ../emery)
cargo make eval                      # every case
cargo make eval orders-docs          # one case
```

Prerequisites, cases, measurements, and the scorecard schema: [`examples/eval/README.md`](examples/eval/README.md). The `omnia-r9k` case shallow-clones its `UNLICENSED` upstream into a gitignored fixture cache on first run.

## Repair loop

1. Edit `sources/<name>/prose/**` (the extract prompt, references, rules).
2. `cargo make adapter <name>` to rebuild the component, then re-run the eval case.
3. Compare the retained sandbox (`sandbox/<case>/`) and scorecard with the previous run.

Native crate tests stay the Rust inner loop; live eval is for prompt quality. See [docs/testing.md](docs/testing.md).

## Stuck?

| Symptom | What to check |
| --- | --- |
| `cargo make fmt` fails | Install nightly rustfmt: `rustup toolchain install nightly --component rustfmt` |
| Eval refuses: binary missing | Build the sibling [`augentic/emery`](https://github.com/augentic/emery) release binary, or set `EMERY_BIN` |
| Eval refuses: component missing | `cargo make release` |
| Patch-resolution errors after editing root `Cargo.toml` | The committed `[patch.crates-io]` git patches fetch `augentic/emery`; uncomment the path patches only when co-developing against `../emery` |

Bugs and questions: [GitHub Issues](https://github.com/augentic/emery-adapters/issues).

## Further reading

- Contributor setup, engine pin, publishing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Creating an adapter: [docs/authoring.md](docs/authoring.md)
- Test ownership: [docs/testing.md](docs/testing.md)
- Agent instructions: [AGENTS.md](AGENTS.md)

## License

MIT OR Apache-2.0.
