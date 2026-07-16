# Prompt evaluation

The adapters mirror of the engine's [`crates/eval`](https://github.com/augentic/specify/tree/main/crates/eval): a live-model harness for testing this repo's adapter prompts, with real adapters in place of the engine's fixture. Outputs are graded by deterministic validators — not a model.

The same package is the **native dev shim**: the `specify-dev` binary runs any specify verb over the linked adapter crates without building WebAssembly (`cargo make dev -- --project-dir <dir> plan status`), serves the per-adapter MCP reference shelves (`specify-dev serve`), and carries the deterministic seam/CLI/MCP test suites (`cargo make eval-test`). This workspace is standalone — its manifest pins the Specify engine revision it is verified against; see the [root README](../README.md#publishing) and [TESTING.md](../TESTING.md).

## Quick start

Login to the Cursor agent:

```bash
cursor-agent login
```

or set `CURSOR_API_KEY` in `.env` at the repository root.

```bash
cargo make eval
```

This runs the entire workflow in `sandbox/eval/` — the operator rhythm over a contracts-bound project with `documentation` + `intent` as sources. A passing run will remove the project, while a failing run will retain it for in-place review, or to re-run individual operations (using the manual workflow below).

### Manual workflow

Run one operation at a time to inspect its artifacts:

```bash
cargo make eval init
cargo make eval plan
cargo make eval execute
cargo make eval finalize
```

While `cargo make eval init` will reinitialize a project, a project can also be removed using:

```bash
cargo make eval clean
```

### Prompt scenarios

For fast prompt iteration, one adapter operation over a seeded scratch tree — minutes, not a full change:

```bash
cargo make eval scenario                     # list scenarios
cargo make eval scenario contracts/design    # run one
```

Scenario anatomy and the index live in [`scenarios/README.md`](scenarios/README.md).

## Model judgment

A trial run exercises both the engine's own judgment legs and this repo's adapter legs, all through the live cursor backend:

| Leg                            | Owner            | Purpose                                                                 |
| ------------------------------ | ---------------- | ------------------------------------------------------------------------ |
| `proposal`                     | engine (`change`)| Reconcile surveyed leads across sources into plan slices                 |
| `synthesis`                    | engine (`slice`) | Reconcile extracted evidence into `proposal.md`, `spec.md`, …            |
| `leads` / `evidence`           | source adapters  | `documentation` + `intent` survey and extract                            |
| contracts sub-flows + `report` | target adapter   | The `contracts` build's author / import / verify legs and its report     |

## Workflow

The driver mirrors the operator rhythm:

```text
init        specify init contracts + seed docs/
plan        specify plan author (documentation + intent) → Gate 1 approved
execute     specify plan execute  (refine → build → merge per slice, until drained)
finalize    specify plan archive
```

Every step runs the production operation through the shared typed command router — `execute` is the real drained loop, not a hand-driven breakout sequence.

## Grading

Hard assertions only (`src/grade.rs`):

| Stage   | Check              | Pass condition                                                        |
| ------- | ------------------ | ---------------------------------------------------------------------- |
| plan    | Authored entries   | `plan author` produced at least one entry                              |
| execute | Lifecycle          | Every plan entry is `done`                                             |
| execute | Provenance         | Every evidenced requirement carries sources; ids are present           |
| execute | Contracts baseline | The merged `contracts/` baseline is non-empty and validator-clean      |

Per-leg request / repair counts are **reported, not asserted**. After grading, the trial prints one line per judgment leg (keyed by answer-schema name) with its request count; for the engine legs it derives repairs from the invocation baseline (one propose per trial, one synthesis per plan entry). A leg drifting from zero repairs toward the budget is the early signal that a prompt or answer-schema change degraded the model's first answer.

In manual mode, repair counts cover only model requests made by that operation.
