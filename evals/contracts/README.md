# Contracts Guest Evals — RFC-61 Step 2 Decomposition Proof

This directory is the live-backend eval harness for the contracts adapter guest: the session-less decomposition proof the RFC's Step 2 exit criteria require before Step 4 starts. Each scenario drives the guest's `build` operation — the three format sub-flows as single-shot `create` calls against the real cursor backend, a bounded verify-repair loop, then the schema-gated report — with all state in the scratch working tree and references fetched over the guest's own MCP route.

## Anatomy

| Piece | Where |
| ----- | ----- |
| Host binary | [`evals/runtime.rs`](../runtime.rs) (the `eval-driver` example of the flattened `evals` package) — `omnia::runtime!({ mode: command, hosts: { WasiHttp: HttpDefault, WasiModel: EvalModel } })` — the cursor backend behind the `SPECIFY_EVAL_MODEL` decorator |
| Driver guest | [`evals/guest.rs`](../guest.rs) (the `eval-guest` cdylib example) — the deployment's `wasi:cli/run` exporter; drives one seam operation per invocation (`survey` / `extract` / `guidance` / `build` / `merge`, selected by the first argument — these scenarios drive `build`), reads its inputs from the mount, prints the typed answer as one JSON line |
| Adapter guest | [`targets/contracts`](../../targets/contracts) — the component under test |
| Scenario seeds | `scenarios/<name>/inputs/*.md` (typed slice inputs by file stem) and `scenarios/<name>/seed/**` (files copied into the scratch project root) |
| Runner | [`evals/live.rs`](../live.rs) (the `live` `[[test]]` target) — one `#[ignore]`d test per scenario under `contracts::`, plus the non-ignored `contracts::wiring` smoke CI runs model-free; `cargo make eval-contracts` runs every scenario |
| Results | `runs/` — per-run raw output; the committed run summary lives at `runs/SUMMARY.md` |

The scenarios mirror the operator-driven documents under [`targets/contracts/tests/`](../../targets/contracts/tests/), reduced to the build leg this harness exercises: the `inputs/` docs stand in for what `/spec:refine` would have produced, and the report + validator gate stand in for the build phase's verifier. `update-boundary` contributes only its regression path (the contracts slice); its negative path asserts the behavior of an *implementation* adapter and cannot run through the contracts guest.

## Running

Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`, authenticated via `CURSOR_API_KEY` or a prior `cursor-agent login`. Set `SPECIFY_EVAL_MODEL=<model-id>` to override the model driver-side for a run (fills `Request.model` only when the guest left it unset; the id never enters a guest).

```bash
# every scenario
cargo make eval-contracts

# one scenario
cargo test -p evals --test live -- --ignored --nocapture contracts::describe
```

Each test builds the guests, seeds a scratch tree under a temp dir, writes the deployment manifest, and spawns the prebuilt `eval-driver` example for one command-mode run. The report JSON line and full log land under `runs/<scenario>/`; the test fails on a failing report `status`.

The non-ignored `contracts::wiring` test is the model-free smoke: it seeds every scenario and renders the manifest without building guests or spawning anything, so ordinary CI catches scenario-tree drift.
