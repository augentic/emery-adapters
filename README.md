# Specify Adapters

[![CI](https://github.com/augentic/specify-adapters/actions/workflows/ci.yaml/badge.svg)](https://github.com/augentic/specify-adapters/actions/workflows/ci.yaml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

First-party **source** and **target** Wasm components for [Specify](https://github.com/augentic/specify).

**Using Specify in a project?** You do not need this repository. Install adapters with a pin from the engine CLI — for example `specify init contracts@0.5.0` — and follow the [Specify README](https://github.com/augentic/specify#readme).

**Authoring or debugging an adapter?** This repo is your home. Edit prose or Rust, then re-run a live scenario until `report.json` says `pass`.

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

Needed only for `make eval` / `make eval scenario …`:

1. Authenticated [`cursor-agent`](https://cursor.com/docs/cli) on `PATH` — `cursor-agent login`, or `CURSOR_API_KEY` in a repo-root `.env` (the `eval` task loads it).
2. Optional: `EVAL_MODEL=<model-id>`, `EVAL_TIMEOUT_SECS=<secs>` (defaults to `300`).

If eval hangs or fails authenticating, check `cursor-agent` login / `.env` — see [CONTRIBUTING.md § Troubleshooting](CONTRIBUTING.md#troubleshooting-first-runs). Grading is **deterministic** (not a model): the eval binary links every first-party adapter into a native catalog and drives production verbs through the shared cursor backend.

## Quick start

From the repository root, run one target-adapter scenario (~2–5 minutes; needs cursor auth):

```bash
make eval scenario contracts/design
```

Stock `make eval` (no `scenario`) is the **contracts** full trial only — tens of minutes. Prefer a scenario while iterating.

The command prints the retained scratch directory and report path. Open `report.json`; a successful run looks like:

```json
{ "outcome": "pass" }
```

`outcome: "pass"` means the adapter report *and* every `expect` path in `scenario.toml` succeeded — a success report that wrote nothing still fails the expect gate. The generated contract delta is under `.specify/slices/returns-api/contracts/` in the scratch tree.

To start developing, edit the contracts adapter’s prompts or references under `targets/contracts/prose/`, then run the same command again and compare the new scratch directory with the previous run. Native scenarios pick up prose changes automatically — no Wasm build is required.

```bash
make eval scenario    # list scenarios
```

| Target | Smoke scenario |
| --- | --- |
| contracts | `make eval scenario contracts/design` |
| omnia | `make eval scenario omnia/health` |
| vectis | `make eval scenario vectis/single-screen` |

## Scenario vs full trial

Prefer a **scenario** when iterating on one adapter operation (minutes). Use a **full trial** only when you need `plan → execute → finalize` or real source trees (tens of minutes).

| | Scenario | Full trial |
| --- | --- | --- |
| Command | `make eval scenario <adapter>/<name>` | `make eval` (contracts) |
| Output | `sandbox/<adapter>/<name>/run-…/` + `report.json` | `sandbox/` project tree |
| Retention | Always kept | Kept on failure; removed on full pass |
| Depth | [scenarios.md](examples/eval/scenarios.md) | [trial.md](examples/eval/trial.md) |

Omnia and Vectis need a custom trial — see [trial.md § Custom trials](examples/eval/trial.md#custom-trials).

## After a run

**Scenario.** Open `report.json` first, then inspect the scratch tree for expected artifacts and any review/verify output under the slice or generated tree.

```text
sandbox/<adapter>/<name>/run-<stamp>-<pid>/
  report.json          # outcome: pass | fail
  …slice tree and generated outputs…
```

**Full trial.** On failure (or a phased stop), `sandbox/` is kept for review:

```text
sandbox/
  plan.yaml / change.md / discovery.md
  .specify/slices/<slice>/     # proposal, specs, design, tasks, evidence
  …target outputs (contracts, crates/, shells, …)
```

Grading checks lifecycle and provenance; target quality is still a human look — see [trial.md § Grading](examples/eval/trial.md#grading). Per-leg repair counts are reported, not asserted; drift toward the repair budget is an early signal that prose or answer-schema changes degraded the first answer.

Resume without wiping the tree — re-run only the failed phase (never `init`):

```bash
make eval execute    # stock contracts; same for plan | finalize
```

For custom trials, pass the full argv again with the phase name; see [trial.md § Phases](examples/eval/trial.md#phases).

## Repair loop

1. Edit `{targets,sources}/<name>/prose/**` (prompts, references, rules). Scenarios load prose from the linked crates — no Wasm rebuild.
2. Re-run the same scenario (e.g. `make eval scenario contracts/design`).
3. Compare the new `sandbox/<adapter>/<name>/run-…/` tree and `report.json` to the previous run.
4. Repeat until `outcome: pass` and the artifacts look right.

Do not burn a full trial for a prompt typo — use a scenario, or [add one](examples/eval/scenarios.md#anatomy). Native crate tests (`cargo nextest run -p <adapter>`) stay the Rust inner loop; live eval is for prompt quality. See [docs/testing.md](docs/testing.md).

## Stuck?

| Symptom | What to check |
| --- | --- |
| Eval hangs / auth errors | `cursor-agent login` or `CURSOR_API_KEY` in repo-root `.env` |
| `cargo make fmt` fails | Install nightly rustfmt: `rustup toolchain install nightly --component rustfmt` |
| `cargo make wasm-run` fails immediately | Needs sibling [`augentic/specify`](https://github.com/augentic/specify) at `../specify` |
| Patch-resolution errors after editing root `Cargo.toml` | `[patch."https://github.com/augentic/specify.git"]` needs `../specify`; re-comment if not co-developing |
| Scenario `outcome: fail` with a green-looking report | Check `expect` paths in `scenario.toml` — missing files fail the gate |

More first-run tips: [CONTRIBUTING.md](CONTRIBUTING.md#troubleshooting-first-runs). Bugs and questions: [GitHub Issues](https://github.com/augentic/specify-adapters/issues).

## Further reading

| Topic | Doc |
| --- | --- |
| Creating an adapter | [docs/authoring.md](docs/authoring.md) |
| Scenario index and anatomy | [examples/eval/scenarios.md](examples/eval/scenarios.md) |
| Stock / custom trials | [examples/eval/trial.md](examples/eval/trial.md) |
| Eval package notes | [examples/eval/README.md](examples/eval/README.md) |
| Toolchain, layout, publishing | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Test rungs | [docs/testing.md](docs/testing.md) |
| Wasm / WIT seam | [examples/wasm/README.md](examples/wasm/README.md) |
| Agent / contract rules | [AGENTS.md](AGENTS.md) |
| Operator docs (engine) | [Specify README](https://github.com/augentic/specify#readme) · [hosted guide](https://specify.augentic.io/) |
| Lab CLI (native catalog; not the shipped CLI) | `make specify -- --project-dir <dir> slice list` |

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your option. Contribution norms (including DCO) match the engine repo — see [specify CONTRIBUTING](https://github.com/augentic/specify/blob/main/CONTRIBUTING.md). [Code of Conduct](CODE-OF-CONDUCT.md).
