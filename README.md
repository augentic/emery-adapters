# Specify Adapters

Specify **source** and **target**  WebAssembly adapters — plugins for use by the [Specify](https://github.com/augentic/specify) framework.

This repository is primarily for adapter authors and, aside from the code/prose, supports running live evals for debugging purposes. The dev-loop allows authors to rapidly inspect failures, repair and re-run. 

## Start here


| I want to…                                            | Go to                                                              |
| ----------------------------------------------------- | ------------------------------------------------------------------ |
| Fix or tune adapter prompts / references              | The repair loop below                                              |
| Fix Rust adapter logic                                | [docs/testing.md](docs/testing.md)`cargo nextest run -p <adapter>` |
| Create a new adapter (source or target)               | [docs/authoring.md](docs/authoring.md)                             |
| Set up the toolchain, publish, or bump the engine pin | [CONTRIBUTING.md](CONTRIBUTING.md)                                 |


The root `Makefile` forwards every goal to [cargo-make](Makefile.toml), so `make eval …` and `cargo make eval …` are interchangeable; this README uses the shorter form.

## Prerequisites

1. Authenticated `[cursor-agent](https://cursor.com/docs/cli)` on `PATH` — `cursor-agent login`, or `CURSOR_API_KEY` in a repo-root `.env` (the `eval` task loads it).
2. Optional: `EVAL_MODEL=<model-id>`, `EVAL_TIMEOUT_SECS=<secs>` (defaults to `300`).

The eval binary links every first-party adapter into a native catalog and drives production verbs through the shared cursor backend. Grading is **deterministic** — not a model.

## Quick start

From the repository root, run one target-adapter scenario:

```bash
make eval scenario contracts/design
```

The command prints the retained scratch directory and report path. Open `report.json`; a successful run has `outcome: "pass"`, and the generated contract delta is under `.specify/slices/returns-api/contracts/`.

To start developing, edit the contracts adapter's prompts or references under `targets/contracts/prose/`, then run the same command again and compare the new scratch directory with the previous run. Native scenarios pick up prose changes automatically — no Wasm build is required.

Use `make eval scenario` to list the other scenarios. Below: when to escalate to a full trial, how to read a failure, and the repair loop.

## Scenario vs full trial

Prefer a **scenario** when iterating on one adapter operation (minutes). Use a **full trial** only when you need `plan → execute → finalize` or real source trees (tens of minutes).

| | Scenario | Full trial |
| --- | --- | --- |
| Command | `make eval scenario <adapter>/<name>` | `make eval` (contracts) |
| Output | `sandbox/<adapter>/<name>/run-…/` + `report.json` | `sandbox/` project tree |
| Retention | Always kept | Kept on failure; removed on full pass |
| Depth | [scenarios.md](examples/eval/scenarios.md) | [trial.md](examples/eval/trial.md) |

Stock `make eval` is the contracts trial only. Omnia and Vectis need a custom trial — see [trial.md § Custom trials](examples/eval/trial.md#custom-trials).

| Target | Smoke scenario |
| --- | --- |
| contracts | `make eval scenario contracts/design` |
| omnia | `make eval scenario omnia/health` |
| vectis | `make eval scenario vectis/single-screen` |

## After a run

**Scenario.** Open `report.json` first. `outcome: pass` means the adapter report *and* every `expect` path in `scenario.toml` succeeded — a success report that wrote nothing still fails the expect gate. Then inspect the scratch tree for those artifacts and any review/verify output under the slice or generated tree.

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

Lab only (same native catalog as eval; not on the shipped CLI): `make specify -- --project-dir <dir> slice list`.


