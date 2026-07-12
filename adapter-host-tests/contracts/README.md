# Contracts guest evals

Live-backend eval harness for the contracts adapter guest. Each scenario drives the guest's `build` operation — the three format sub-flows as single-shot `create` calls against the real cursor backend, a bounded verify-repair loop, then the schema-gated report — with all state in the scratch working tree and references fetched over the guest's own MCP route.

## Anatomy

| Piece          | Where                                                                                                                                                                                                                                                                                                                                                     |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Host binary    | [`evals/runtime.rs`](../runtime.rs) (the `eval-driver` example of the flattened `evals` package) — `omnia::runtime!({ mode: command, hosts: { WasiHttp: HttpDefault, WasiModel: EvalModel } })` — the cursor backend behind the `SPECIFY_EVAL_MODEL` decorator                                                                                            |
| Driver guest   | [`evals/guest.rs`](../guest.rs) (the `eval-guest` cdylib example) — the deployment's `wasi:cli/run` exporter; drives one seam operation per invocation (`survey` / `extract` / `guidance` / `build` / `merge`, selected by the first argument — these scenarios drive `build`), reads its inputs from the mount, prints the typed answer as one JSON line |
| Adapter guest  | [`targets/contracts`](../../targets/contracts) — the component under test                                                                                                                                                                                                                                                                                 |
| Scenario seeds | `scenarios/<name>/inputs/*.md` (typed slice inputs by file stem) and `scenarios/<name>/seed/**` (files copied into the scratch project root)                                                                                                                                                                                                              |
| Runner         | [`evals/live.rs`](../live.rs) (the `live` `[[test]]` target) — one `#[ignore]`d test per scenario under `contracts::`, plus the non-ignored `contracts::wiring` smoke CI runs model-free                                                                                                                                                                  |
| Results        | `runs/` — per-run raw output; the committed run summary lives at `runs/SUMMARY.md`                                                                                                                                                                                                                                                                        |

The scenarios mirror the operator-driven documents beside them under [`scenarios/`](scenarios/README.md), reduced to the build leg this harness exercises.

## Running

Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`, authenticated via `CURSOR_API_KEY` or a prior `cursor-agent login`. Set `SPECIFY_EVAL_MODEL=<model-id>` to override the model driver-side for a run.

```bash
cargo test -p evals --test live -- --ignored --nocapture contracts::
cargo test -p evals --test live -- --ignored --nocapture contracts::metadata
```

Each test builds the guests, seeds a scratch tree under a temp dir, writes the deployment manifest, and spawns the prebuilt `eval-driver` example for one command-mode run. The report JSON line and full log land under `runs/<scenario>/`; the test fails on a failing report `status`.

The non-ignored `contracts::wiring` test is the model-free smoke: it seeds every scenario and renders the manifest without building guests or spawning anything.
