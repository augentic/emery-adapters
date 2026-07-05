# Contracts Guest Evals — RFC-61 Step 2 Decomposition Proof

This directory is the live-backend eval harness for the contracts adapter guest: the session-less decomposition proof the RFC's Step 2 exit criteria require before Step 4 starts. Each scenario drives the guest's `build` operation — the three format sub-flows as single-shot `create` calls against the real cursor backend, a bounded verify-repair loop, then the schema-gated report — with all state in the scratch working tree and references fetched over the guest's own MCP route.

## Anatomy

| Piece | Where |
| ----- | ----- |
| Host binary | [`crates/eval-driver`](../../crates/eval-driver) — `omnia::runtime!({ mode: command, hosts: { WasiHttp: HttpDefault, WasiModel: Cursor } })` |
| Driver guest | [`crates/eval-guest`](../../crates/eval-guest) — the deployment's `wasi:cli/run` exporter; reads the slice inputs from the mount, dispatches `target.build`, prints the report as one JSON line |
| Adapter guest | [`targets/contracts`](../../targets/contracts) — the component under test |
| Scenario seeds | `scenarios/<name>/inputs/*.md` (typed slice inputs by file stem) and `scenarios/<name>/seed/**` (files copied into the scratch project root) |
| Runner | `run.sh` (one scenario per invocation) — `cargo make eval-contracts` runs it for every scenario |
| Results | `runs/` — per-run raw output; the committed run summary lives at `runs/SUMMARY.md` |

The scenarios mirror the operator-driven documents under [`targets/contracts/tests/`](../../targets/contracts/tests/), reduced to the build leg this harness exercises: the `inputs/` docs stand in for what `/spec:refine` would have produced, and the report + validator gate stand in for the build phase's verifier. `update-boundary` contributes only its regression path (the contracts slice); its negative path asserts the behavior of an *implementation* adapter and cannot run through the contracts guest.

## Running

Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`, authenticated via `CURSOR_API_KEY` or a prior `cursor-agent login`.

```bash
# every scenario
cargo make eval-contracts

# one scenario
evals/contracts/run.sh describe
```

The runner builds the guests, seeds a scratch tree under a temp dir, writes the deployment manifest, and drives one command-mode run. The report JSON line and full log land under `runs/<scenario>/`; exit status carries the report's `status`.
