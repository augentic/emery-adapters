# Vectis Guest Evals — RFC-61 Step 3 Build-Leg Proof

This directory is the live-backend eval harness for the vectis adapter guest. The scenario drives the guest's `build` operation end-to-end: the deterministic in-guest prepare prelude (scope resolution + conditional `materialize assets`), the prompt-driven judgment legs as single-shot `create` calls against the real cursor backend (composition, Crux core, per-shell writes, review), the in-core composition validator gate with its bounded repair, then the schema-gated report and the deterministic postlude — with all state in the scratch working tree and references fetched over the guest's own MCP route.

## Anatomy

| Piece | Where |
| ----- | ----- |
| Host binary | [`evals/runtime.rs`](../runtime.rs) (the `eval-driver` example of the flattened `evals` package) — `omnia::runtime!({ mode: command, hosts: { WasiHttp: HttpDefault, WasiModel: EvalModel } })` — the cursor backend behind the `SPECIFY_EVAL_MODEL` decorator |
| Driver guest | [`evals/guest.rs`](../guest.rs) (the `eval-guest` cdylib example) — the deployment's `wasi:cli/run` exporter; drives one seam operation per invocation (`survey` / `extract` / `guidance` / `build` / `merge`, selected by the first argument — this scenario drives `build`), reads its inputs from the mount, prints the typed answer as one JSON line |
| Adapter guest | [`targets/vectis`](../../targets/vectis) — the component under test (`vectis.wasm`) |
| Scenario seeds | `scenarios/<name>/inputs/*.md` (typed slice inputs by file stem) and `scenarios/<name>/seed/**` (files copied into the scratch project root: `.specify/project.yaml` platform set, operator-curated `design-system/` manifests) |
| Runner | [`evals/live.rs`](../live.rs) (the `live` `[[test]]` target) — one `#[ignore]`d test per scenario under `vectis::`, plus the non-ignored `vectis::wiring` smoke CI runs model-free; `cargo make eval-vectis` runs the scenario |
| Results | `runs/` — per-run raw output |

## Scenarios

| Scenario | Slice | Shape |
| -------- | ----- | ----- |
| `single-screen` | `daily-quote` | A tiny single-screen feature (one read-only quote screen with a refresh action) on a `core + ios` platform set — one composition leg, one Crux core leg, one iOS shell leg, review, report. The Android leg is skipped by the declared platform set; the seed's `assets.yaml` is symbol-only so the materialize prelude reports `skipped: true`. |

The scenario is deliberately minimal: it proves the session-less leg decomposition and the deterministic prelude / validator-gate / postlude bracket against a live model, not the full fixture depth of `targets/vectis/tests/`. The spawned agent's host-command loops (cargo / xcodebuild / make) may degrade gracefully inside the sandboxed scratch tree; the report and the composition gate are the assertions that matter.

## Running

Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`, authenticated via `CURSOR_API_KEY` or a prior `cursor-agent login`. Set `SPECIFY_EVAL_MODEL=<model-id>` to override the model driver-side for a run (fills `Request.model` only when the guest left it unset; the id never enters a guest).

```bash
# the scenario through cargo-make
cargo make eval-vectis

# directly
cargo test -p evals --test live -- --ignored --nocapture vectis::single_screen
```

Each test builds the guests, seeds a scratch tree under a temp dir, writes the deployment manifest, and spawns the prebuilt `eval-driver` example for one command-mode run. The report JSON line and full log land under `runs/<scenario>/`; the test fails on a failing report `status`.

## Smoke-checking without a model

```bash
cargo test -p evals --test live vectis::wiring
```

The non-ignored `vectis::wiring` test exercises everything below the model seam that needs no build — the scratch tree seeding and the deployment-manifest generation for every scenario — so ordinary CI runs it without `cursor-agent` or the wasm32 guests.
