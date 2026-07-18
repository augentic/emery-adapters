# Prompt evaluation

A live-model harness for testing adapter prompts and references used in judgement steps. Outputs are graded by deterministic validators — not a model.

## Quick start

Login to the Cursor agent:

```bash
[cursor-]agent login
```

or set `CURSOR_API_KEY` in `.env` at the repository root.

```bash
cargo make eval
```

This runs the entire workflow in `sandbox/` — the operator rhythm over a contracts-bound project with `documentation` + `intent` as sources. Expect a full trial to take tens of minutes of live model time; a single phase or prompt scenario takes minutes. A passing run will remove the project, while a failing run will retain it for in-place review, or to re-run individual operations (using the manual workflow below).

Runs are hermetic: every phase and scenario isolates its execution paths, so the project cache lands inside its own sandbox — the operator's normal project cache is never read or written, and a run's result never depends on prior local state. The trial's operator inputs (change name, intent, source binding) come from the shared [`examples/change/trial.env`](../../examples/change/trial.env) — the same definition the wasm change example `source`s — and both rungs seed the same [`examples/change/seed/`](../../examples/change/seed/) tree. Project name defaults from the sandbox directory basename.

`SPECIFY_EVAL_MODEL=<model-id>` overrides the model for a run: the driver fills `Request.model` only when the guest left it `None`, so a guest-supplied id always wins; unset or blank means the cursor backend's default. The cursor connection is lazy — it happens on the first judgment leg, so deterministic phases never require `cursor-agent` on `PATH`. The trial driver, scenario runner, telemetry, and grading live in Specify's lab-only `eval` library over its `linked` host; this lab owns the Tokio runtime, the Cursor backend construction, the first-party catalog declaration ([`src/lib.rs`](src/lib.rs)), and its scenario root, with the `cargo make eval` task passing the trial inputs explicitly.

The binary runs the trial behind the `eval` subcommand and sends any other argv through the linked command path over the first-party catalog (`cargo make dev -- --project-dir <dir> slice list`).

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

Scenario anatomy and the index live in [`scenarios/README.md`](scenarios/README.md). Scenarios run **natively** over the linked adapter crates — they prove prompt quality, not WASM/WIT conformance (that stays with `composed/` and the change example). For a first-party adapter a new scenario is just the data directory; a **third-party adapter** additionally needs a Cargo dependency in [`Cargo.toml`](Cargo.toml) and a catalog entry in [`src/lib.rs`](src/lib.rs), because configuration alone cannot link a Rust crate into the shim.

## Model judgment

A trial run exercises both the engine's own judgment legs and this repo's adapter legs, all through the live cursor backend:


| Leg                            | Owner             | Purpose                                                              |
| ------------------------------ | ----------------- | -------------------------------------------------------------------- |
| `proposal`                     | engine (`change`) | Reconcile *surveyed* leads across sources into plan slices           |
| `synthesis`                    | engine (`slice`)  | Reconcile *extracted* evidence into `proposal.md`, `spec.md`, …      |
| `leads` / `evidence`           | source adapters   | `documentation` + `intent` survey and extract                        |
| contracts sub-flows + `report` | target adapter    | The `contracts` build's author / import / verify legs and its report |


The execution and repair loop lives in the engine's `project` crate and is infrastructure, not judgment.

## Workflow

The driver mirrors the operator rhythm:

```text
init        specify init contracts + seed docs/
plan        specify plan author (documentation + intent) → Gate 1 approved
execute     specify plan execute  (refine → build → merge per slice, until drained)
finalize    specify plan archive
```

Every step runs the production operation — `execute` is the real drained loop, not a hand-driven breakout sequence. Completed phases are echoed as the loop runs.

## Grading

Hard assertions only — the shared runner applies the same set to every adapter binding:


| Stage   | Check      | Pass condition                                               |
| ------- | ---------- | ------------------------------------------------------------ |
| plan    | Entries    | `plan author` produces at least one entry                    |
| execute | Lifecycle  | Every plan entry is `done`                                   |
| execute | Provenance | Every evidenced requirement carries sources; ids are present |


Per-leg request / repair counts are **reported, not asserted**. After grading, the trial prints one line per judgment leg (keyed by answer-schema name) with its request count and derived repairs — requests beyond one per leg invocation (one propose per trial, one synthesis per plan entry), e.g. `leg synthesis: 4 request(s) over 3 slice(s), 1 repair(s)`. A leg drifting from zero repairs toward the budget is the early signal that a prompt or answer-schema change degraded the model's first answer.

In manual mode, repair counts cover only model requests made by that operation.