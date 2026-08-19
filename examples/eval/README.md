# Live eval — the graded spec-generation rung

A client of the public contract only (architecture-review T6): the runner spawns the sibling shipped `emery` binary over the built first-party components, drives `init` + `specify` across the component seam, and grades what the contract exposes — typed exit codes, the JSON envelope, and the committed spec set. It never links engine crates, and grading never reads telemetry (ADR-0001).

Operator-invoked, never CI. CI compiles this package and runs its grading-kernel tests; the live rung needs a model backend and runs from a workstation.

## Prerequisites

1. Built components: `cargo make release` (or `cargo build -p <name> --target wasm32-wasip2 --release` per adapter).
2. The sibling shipped binary: `cargo build --release --bin emery` in the `../emery` checkout. Override the checkout with `EMERY_REPO` or the binary with `EMERY_BIN`.
3. A live model backend for the binary's `wasi:model` host (the Cursor client).

## Running

```bash
cargo make eval             # every case
cargo make eval orders-docs # one case
```

Each case stages its fixture into a fresh retained sandbox (`sandbox/<case>/`), runs `emery init` with the built components (workspace-backed sources plus the case's inline `intent` value), then `emery --format json specify`, and grades the committed generation. The `omnia-r9k` case shallow-clones its UNLICENSED upstream into the gitignored `cases/omnia-r9k/fixture/` cache on first run.

## What is measured

Against [product.md](https://github.com/augentic/emery/blob/main/rfcs/product.md)'s measured qualities:

- **Time to first reviewable specification** — wall clock from `init` to the committed generation pointer, per case; the scorecard reports the worst case against the ≤30-minute target.
- **Per-operation success rate** — one extract per source binding plus one synthesis per case, from typed outcomes only; target ≥95%. A typed nonzero exit is recorded as the outcome, never bypassed.
- **CC-05 / CC-06 mechanical properties** — disagreement and gaps inline (`[unknown]` / `[conflict]` / `[divergence]` heading tags coherent with `Status:` lines), provenance one gesture away (`ID:` / `Sources:` / `Status:` on every block), and the spec covering the bound estate.
- **Reviewability beyond the mechanical checks** — model-graded territory, recorded `unconfirmed` until wired; unmeasured never silently passes.

## The scorecard

Every run writes `sandbox/scorecard.md`: dated, naming the `emery` and `emery-adapters` commit shas and the product.md numbers, with `status: green` only when every case passed and both measured numbers meet their targets. The release gate in the `emery` repository verifies a committed green scorecard names the release-tip sha (CONSTITUTION invariant 6) — CI verifies the record, it never runs the live eval. To publish a run, copy the scorecard into `emery/rfcs/scorecards/<date>.md`.
