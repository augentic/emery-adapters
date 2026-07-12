# Vectis guest quality tests

Live-backend eval harness for the vectis adapter guest. The scenario drives the guest's `build` operation end-to-end: the deterministic in-guest prepare prelude, prompt-driven judgment legs against the real cursor backend, the in-guest composition validator gate with its bounded repair, then the schema-gated report and deterministic postlude.

## Anatomy

| Piece          | Where                                                                                            |
| -------------- | ------------------------------------------------------------------------------------------------ |
| Host binary    | [`harness/runtime.rs`](../runtime.rs) (the `eval-driver` example of the flattened `harness` package) |
| Driver guest   | [`harness/guest.rs`](../guest.rs) (the `eval-guest` cdylib example)                                |
| Adapter guest  | [`targets/vectis`](../../targets/vectis) — the component under test (`vectis.wasm`)              |
| Scenario seeds | `scenarios/<name>/inputs/*.md` and `scenarios/<name>/seed/**`                                    |
| Runner         | [`harness/live.rs`](../live.rs) — the ignored `vectis::single_screen` test runs the scenario       |
| Results        | `runs/` — per-run raw output                                                                     |

## Scenarios

| Scenario        | Slice         | Shape                                                                                                            |
| --------------- | ------------- | ---------------------------------------------------------------------------------------------------------------- |
| `single-screen` | `daily-quote` | A tiny single-screen feature on a `core + ios` platform set — composition, Crux core, iOS shell, review, report. |

## Running

Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`. Set `SPECIFY_EVAL_MODEL=<model-id>` to override the model driver-side.

```bash
cargo make dev -- live vectis single_screen
```

## Smoke-checking without a model

```bash
cargo test -p harness --test live vectis::wiring
```
