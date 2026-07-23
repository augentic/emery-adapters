# Prompt evaluation

A live-model harness for testing adapter prompts and references used in judgement steps. Outputs are graded by deterministic validators — not a model.

Ownership, hermeticism, and how this example sits among the five test rungs: [TESTING.md](../../TESTING.md). Catalog declaration: [`src/catalog.rs`](../../src/catalog.rs). Shared fixture with the wasm example: [`examples/wasm/fixture/`](../wasm/fixture/).

## Quick start

Login to the Cursor agent:

```bash
[cursor-]agent login
```

or set `CURSOR_API_KEY` in `.env` at the repository root.

```bash
make eval
```

This runs the full operator rhythm in `sandbox/` over a `contracts`-bound project with `documentation` + `intent` as sources. Expect tens of minutes of live model time. A passing run removes the project; a failing run retains it for in-place review or per-phase re-runs (below).

`SPECIFY_EVAL_MODEL=<model-id>` overrides the model; unset means the cursor backend's default.

Any other specify verb goes through the native first-party catalog:

```bash
cargo make specify -- --project-dir <dir> slice list
```



### Manual workflow

```bash
make eval init
make eval plan
make eval execute
make eval finalize
make eval clean     # or re-run init to reinitialize
```



### Prompt scenarios

One adapter operation over a fixture scratch tree — minutes, not a full change:

```bash
make eval scenario                     # list
make eval scenario contracts/design    # one
```

Anatomy, indexing, and third-party joining: `[scenarios/README.md](scenarios/README.md)`.

## Model judgment

A trial exercises engine legs (`proposal`, `synthesis`) and this repo's adapter legs (`leads` / `evidence` on sources; contracts build sub-flows + `report` on the target). Rung details and grading posture: [TESTING.md](../../TESTING.md) (§ eval composition example).

## Workflow

```text
init        specify init contracts + fixture docs/
plan        specify plan author (documentation + intent) → Gate 1 approved
execute     specify plan execute  (refine → build → merge per slice, until drained)
finalize    specify plan archive
```

Every step runs the production operation — `execute` is the real drained loop. Completed phases are echoed as the loop runs.

## Grading

Hard assertions only (shared `probe` runner):


| Stage   | Check      | Pass condition                                               |
| ------- | ---------- | ------------------------------------------------------------ |
| plan    | Entries    | `plan author` produces at least one entry                    |
| execute | Lifecycle  | Every plan entry is `done`                                   |
| execute | Provenance | Every evidenced requirement carries sources; ids are present |


Per-leg request / repair counts are **reported, not asserted**. A leg drifting from zero repairs toward the budget is the early signal that a prompt or answer-schema change degraded the model's first answer. In manual mode, counts cover only that operation's requests.