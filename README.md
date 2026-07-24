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

Use `make eval scenario` to list the other scenarios. Continue below for debugging, the repair loop, and full end-to-end trials.

## 1. Run an eval

Two loops. Prefer a **scenario** when iterating on prompts; use a **full trial** when you need the whole `plan → execute → finalize` rhythm.


| Loop           | Use when                                              | Command                | Depth                                      |
| -------------- | ----------------------------------------------------- | ---------------------- | ------------------------------------------ |
| **Scenario**   | One adapter operation (`build` / merge gate); minutes | `make eval scenario …` | [scenarios.md](examples/eval/scenarios.md) |
| **Full trial** | Real sources → working-tree outputs; tens of minutes  | `make eval`            | [trial.md](examples/eval/trial.md)         |


```text
scenario   one seam op over inputs/*.md (+ optional fixture)
               → sandbox/<adapter>/<name>/run-…/ + report.json

trial      init → plan author → approved → plan execute → archive
               → sandbox/ project (cleaned on full pass)
```



### Start here by target

Stock `make eval` is the **contracts** trial only.


| Target        | Scenario smoke                            | Full trial                                                          |
| ------------- | ----------------------------------------- | ------------------------------------------------------------------- |
| **contracts** | `make eval scenario contracts/design`     | `make eval`                                                         |
| **omnia**     | `make eval scenario omnia/health`         | Custom trial — see [trial.md](examples/eval/trial.md#custom-trials) |
| **vectis**    | `make eval scenario vectis/single-screen` | Custom trial (`--target vectis`, fixture + platforms as needed)     |


List scenarios:

```bash
make eval scenario
```

Any specify verb through the same native catalog (lab flag — not on the shipped CLI):

```bash
make specify -- --project-dir <dir> slice list
```



## 2. Debug after a run



### Scenario

Each run keeps a scratch tree (passing trials clean `sandbox/`; scenarios always retain theirs):

```text
sandbox/<adapter>/<name>/run-<stamp>-<pid>/
  report.json          # outcome: pass | fail
  …slice tree and generated outputs…
```

Open `report.json` first — `outcome` is `pass` only when the adapter report **and** every `expect` artifact in `scenario.toml` succeed. A success report that wrote nothing still fails the expect gate.

Then inspect the scratch delta for missing or wrong artifacts named in `expect`, and the adapter's own review / verify output under the slice or generated tree.

### Full trial

A **failing** phase (or a phased stop) retains `sandbox/` for review. A **full** passing unphased run removes it.

```text
sandbox/
  plan.yaml / change.md / discovery.md
  .specify/slices/<slice>/     # proposal, specs, design, tasks, evidence
  …target outputs (contracts, crates/, shells, …)
```

What grading asserts (lifecycle + provenance) vs what you must eyeball (target quality) is in [trial.md § Grading](examples/eval/trial.md#grading). Per-leg **repair counts** are reported, not asserted — a drift toward the repair budget is the early signal that prose or answer-schema changes degraded the first answer.

Resume a failed trial **without** wiping the tree — re-run only the failed phase (never `init` if you mean to resume):

```bash
make eval execute    # stock contracts; same for plan | finalize
```

For custom trials, pass the full argv again with the phase name; see [trial.md § Phases](examples/eval/trial.md#phases).

## 3. Repair loop — edit prose, re-run

The fast path for prompt work:

1. Edit `{targets,sources}/<name>/prose/**` (prompts, references, rules). Scenarios load prose from the linked crates — no wasm rebuild.
2. Re-run the same scenario:

```bash
make eval scenario contracts/design
make eval scenario omnia/health
make eval scenario vectis/single-screen
```

1. Compare the new `sandbox/<adapter>/<name>/run-…/` + `report.json` against the previous run.
2. Repeat until `outcome: pass` and the artifacts look right.

Reach for a [full trial](examples/eval/trial.md) only when the change needs survey → extract → synthesis → build → merge, or real source trees. Do not burn a full trial for a prompt typo — use a scenario (or [add one](examples/eval/scenarios.md#anatomy)).

Native crate tests (`cargo nextest run -p <adapter>`) stay the inner loop for Rust / scripted harness checks; live eval is for prompt quality. See [docs/testing.md](docs/testing.md).

## Further reading


| Topic                         | Doc                                                      |
| ----------------------------- | -------------------------------------------------------- |
| Creating an adapter           | [docs/authoring.md](docs/authoring.md)                   |
| Scenario index and anatomy    | [examples/eval/scenarios.md](examples/eval/scenarios.md) |
| Stock / custom trials         | [examples/eval/trial.md](examples/eval/trial.md)         |
| Eval package notes            | [examples/eval/README.md](examples/eval/README.md)       |
| Toolchain, layout, publishing | [CONTRIBUTING.md](CONTRIBUTING.md)                       |
| Test rungs                    | [docs/testing.md](docs/testing.md)                       |
| Wasm / WIT seam               | [examples/wasm/README.md](examples/wasm/README.md)       |
| Agent / contract rules        | [AGENTS.md](AGENTS.md)                                   |


