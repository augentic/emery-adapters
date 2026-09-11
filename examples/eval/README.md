# Live eval — the graded spec-generation rung

A client of the public contract only (architecture-review T6): the runner spawns the sibling shipped `emery` binary over the built first-party components, drives one `specify` per case across the adapter contract, and grades what the contract exposes — typed exit codes, the JSON envelope, and the committed spec rendered by `emery show spec`. It never links engine crates, never reads engine storage directly, and grading never reads telemetry (ADR-0001).

Operator-invoked, never CI. CI compiles this package and runs its grading-kernel tests; the live rung needs a model backend and runs from a workstation.

## Prerequisites

1. Built components: `make release` (or `cargo build -p <name> --target wasm32-wasip2 --release` per adapter).
2. The sibling shipped binary: `cargo build --release --bin emery` in the `../emery` checkout. Override the checkout with `EMERY_REPO` or the binary with `EMERY_BIN`.
3. A live Cursor model backend for the shipped binary (`omnia-cursor`): `cursor-sdk-bridge` on `PATH` (or `CURSOR_SDK_BRIDGE_BIN`) and `CURSOR_API_KEY`. Install the bridge from the [sdk-bridge releases](https://github.com/cursor/sdk-bridge/releases). A prior `cursor-agent login` is not enough.

## Running

```bash
make eval             # every case
make eval orders-docs # one case
```

Each case stages its fixture into a fresh retained sandbox (`sandbox/<case>/`), runs one `emery --format json specify` naming the built components (workspace-backed sources plus the case's inline `intent` value — the source list never persists), and grades the committed revision through `emery --format json show spec` — the typed specification master the envelope carries as `document`, checked against the Markdown projection it carries as `body`. The `omnia-r9k` case shallow-clones its UNLICENSED upstream into the gitignored `cases/omnia-r9k/fixture/` cache on first run.

## What is measured

Against the measured qualities (time to first reviewable specification; per-operation success):

- **Time to first reviewable specification** — wall clock over the one `specify` invocation to the committed revision, per case; the scorecard reports the worst case against the ≤30-minute target.
- **Per-operation success rate** — one extract per source plus one synthesis per case, from typed outcomes only; target ≥95%. A typed nonzero exit is recorded as the outcome, never bypassed.
- **CC-05 / CC-06 mechanical properties** — read from the typed master and its projection: disagreement and gaps inline (a `[unknown]` / `[conflict]` / `[divergence]` heading tag in the projection coherent with each requirement's `status`), identity stored (every requirement carries a unique `REQ-NNN` id), provenance one gesture away (every requirement cites at least one complete `(source, claim)` pair), and the spec covering the bound estate (a requirement subject naming it).
- **Reviewability beyond the mechanical checks** — model-graded territory, recorded `unconfirmed` until wired; unmeasured never silently passes.

## The scorecard

Every run writes `sandbox/scorecard.md`: dated, naming the `emery` and `emery-adapters` commit shas (plus each cloned fixture's head sha) and the measured numbers, with `status: green` only when the **complete** case catalog ran, every case passed, and both measured numbers meet their targets — a filtered single-case run is an iteration aid and is never green.
