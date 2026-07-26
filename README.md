# Specify Adapters

[![CI](https://github.com/augentic/specify-adapters/actions/workflows/ci.yaml/badge.svg)](https://github.com/augentic/specify-adapters/actions/workflows/ci.yaml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

First-party **source** and **target** Wasm components for [Specify](https://github.com/augentic/specify).

**Using Specify in a project?** You do not need this repository. Install adapters with a pin from the engine CLI — for example `specify init contracts@0.5.0` — and follow the [Specify README](https://github.com/augentic/specify#readme).

**Authoring or debugging an adapter?** This repo is your home. Edit prose or Rust, then re-run a live eval case until it passes.

The pin you pass to `specify` (`contracts@0.5.0`) is this workspace’s shared SemVer (`[workspace.package].version`). Operators consume published GHCR artifacts; authors iterate here natively without a Wasm rebuild for prose changes.

## Choose your path

| I want to… | Go to |
| --- | --- |
| Fix or tune adapter prompts / references | [Quick start](#quick-start) → [Repair loop](#repair-loop) |
| Fix Rust adapter logic | [Rust-only loop](#rust-only-loop) · [docs/testing.md](docs/testing.md) |
| Create a new adapter (source or target) | [docs/authoring.md](docs/authoring.md) |
| Set up the toolchain, publish, or bump the engine pin | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Use Specify as an operator | [Specify README](https://github.com/augentic/specify#readme) — leave this repo |

The root `Makefile` forwards every goal to [cargo-make](Makefile.toml), so `make eval …` and `cargo make eval …` are interchangeable; this README uses the shorter form.

## What an adapter is

An adapter is one Rust crate that ships as one Wasm component. The engine calls its operations; you never hand-edit lifecycle or `plan.yaml` from adapter code.

| Role | Operations | Examples |
| --- | --- | --- |
| **Source** | `survey`, `extract` | intent, documentation, typescript, screenshots, captures |
| **Target** | `guidance`, `build`, `merge` | contracts, omnia, vectis |

How adapters show up for operators: pinned package (`contracts@0.5.0` / `specify:contracts@0.5.0`) pulls from GHCR on first use; bare names resolve only a project component cache seeded by `specify adapter add` or a local `.wasm` at init. Details: [Specify adapter install notes](https://github.com/augentic/specify/blob/main/docs/reference/cli/init.md) and [CONTRIBUTING.md § Publishing](CONTRIBUTING.md#publishing).

## Rust-only loop

Native crate tests do **not** need `cursor-agent` or model credentials:

```bash
cargo make check
cargo nextest run -p contracts    # or any adapter crate name
```

Use this path for Rust logic, validators, and deterministic behavior. Live eval (below) is for prompt quality.

## Prerequisites (live eval)

Needed only for `make eval …`:

1. Authenticated [`cursor-agent`](https://cursor.com/docs/cli) on `PATH` — `cursor-agent login`, or `CURSOR_API_KEY` in a repo-root `.env` (the `eval` task loads it).
2. Optional: `EVAL_MODEL=<model-id>`, `EVAL_TIMEOUT_SECS=<secs>` (defaults to `900`).

If eval hangs or fails authenticating, check `cursor-agent` login / `.env` — see [CONTRIBUTING.md § Troubleshooting](CONTRIBUTING.md#troubleshooting-first-runs). Grading is **deterministic** (not a model): the eval binary links every first-party adapter into a native catalog and drives production verbs through the shared cursor backend.

## Quick start

From the repository root, run one build case (~2–5 minutes; needs cursor auth):

```bash
make eval contracts-design --restart
```

A passing run prints the retained sandbox and the authoritative report path — `.specify/slices/returns-api/contracts/` in that sandbox carries the generated contract delta. A success report that wrote nothing still fails the case's `expect` gate.

To start developing, edit the contracts adapter’s prompts or references under `targets/contracts/prose/`, then run the same command again and compare the sandbox with the previous run (`--restart` replaces it). Native runs pick up prose changes automatically — no Wasm build is required.

```bash
make eval    # list the cases
```

| Target | Smoke case |
| --- | --- |
| contracts | `make eval contracts-design --restart` |
| omnia | `make eval omnia-health --restart` |
| vectis | `make eval vectis-single-screen --restart` |

## Build vs workflow cases

Prefer a **build case** when iterating on one target adapter's build (minutes). Use a **workflow case** only when you need `plan → execute → finalize` or real source trees (tens of minutes). Catalog: [examples/eval/README.md](examples/eval/README.md).

| | Build case | Workflow case |
| --- | --- | --- |
| Command | `make eval <id> --restart` | `make eval <id> --restart [--until plan]` |
| Fixture | committed refined slice | source trees + intent, plan authored live |
| Gates | `built` metadata, `build/report.yaml`, `expect` paths | pending plan at Gate 1, drained plan, provenance |

Omnia has a stock migration workflow case; the `UNLICENSED` Propellerhead upstream is shallow-cloned into the case's gitignored `fixture/` cache on first run and reused offline after that:

```bash
make eval omnia-r9k --restart      # typescript at_r9k_position_adapter → omnia
```

Depth: [eval README § Omnia legacy migration](examples/eval/README.md#omnia-legacy-migration-r9k).

## After a run

Every case keeps one stable sandbox at `examples/eval/sandbox/<id>/`, on success and failure alike:

```text
examples/eval/sandbox/<id>/
  plan.yaml / change.md / discovery.md   # workflow cases
  .specify/slices/<slice>/               # proposal, specs, design, tasks, evidence
  .specify/slices/<slice>/build/report.yaml   # the authoritative build report
  …target outputs (contracts, crates/, shells, …)
```

Grading checks lifecycle, the report, `expect` paths, and (workflow) provenance; target quality is still a human look — see [eval README § Grading](examples/eval/README.md#grading). Per-leg repair counts are reported, not asserted; drift toward the repair budget is an early signal that prose or answer-schema changes degraded the first answer.

An existing sandbox refuses to rerun without `--restart`. Continue or debug it explicitly through the native verbs instead:

```bash
make specify -- --project-dir examples/eval/sandbox/orders-contracts plan approve
```

## Repair loop

1. Edit `{targets,sources}/<name>/prose/**` (prompts, references, rules). Cases load prose from the linked crates — no Wasm rebuild.
2. Re-run the same case (e.g. `make eval contracts-design --restart`).
3. Compare the sandbox tree and report to the previous run.
4. Repeat until the case passes and the artifacts look right.

Do not burn a workflow case for a prompt typo — use a build case, or [add one](examples/eval/README.md#case-shapes). Native crate tests (`cargo nextest run -p <adapter>`) stay the Rust inner loop; live eval is for prompt quality. See [docs/testing.md](docs/testing.md).

## Stuck?

| Symptom | What to check |
| --- | --- |
| Eval hangs / auth errors | `cursor-agent login` or `CURSOR_API_KEY` in repo-root `.env` |
| `cargo make fmt` fails | Install nightly rustfmt: `rustup toolchain install nightly --component rustfmt` |
| `cargo make wasm-run` fails immediately | Needs sibling [`augentic/specify`](https://github.com/augentic/specify) at `../specify` |
| Patch-resolution errors after editing root `Cargo.toml` | `[patch."https://github.com/augentic/specify.git"]` needs `../specify`; re-comment if not co-developing |
| Case fails with a green-looking report | Check `expect` paths in `case.toml` — missing files fail the gate |
| `sandbox … already exists` | Rerun with `--restart`, or continue it via `make specify -- --project-dir <sandbox> …` |

More first-run tips: [CONTRIBUTING.md](CONTRIBUTING.md#troubleshooting-first-runs). Bugs and questions: [GitHub Issues](https://github.com/augentic/specify-adapters/issues).

## Further reading

| Topic | Doc |
| --- | --- |
| Creating an adapter | [docs/authoring.md](docs/authoring.md) |
| Eval case catalog (build + workflow) | [examples/eval/README.md](examples/eval/README.md) |
| Toolchain, layout, publishing | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Test rungs | [docs/testing.md](docs/testing.md) |
| Wasm / WIT seam | [examples/wasm/README.md](examples/wasm/README.md) |
| Agent / contract rules | [AGENTS.md](AGENTS.md) |
| Operator docs (engine) | [Specify README](https://github.com/augentic/specify#readme) · [hosted guide](https://specify.augentic.io/) |
| Lab CLI (native catalog; not the shipped CLI) | `make specify -- --project-dir <dir> slice list` |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option. Contribution norms (including DCO) match the engine repo — see [specify CONTRIBUTING](https://github.com/augentic/specify/blob/main/CONTRIBUTING.md). [Code of Conduct](CODE-OF-CONDUCT.md).
