# Vectis Guest Evals — RFC-61 Step 3 Build-Leg Proof

This directory is the live-backend eval harness for the vectis adapter guest. The scenario drives the guest's `build` operation end-to-end: the deterministic in-guest prepare prelude (scope resolution + conditional `materialize assets`), the prompt-driven judgment legs as single-shot `create` calls against the real cursor backend (composition, Crux core, per-shell writes, review), the in-core composition validator gate with its bounded repair, then the schema-gated report and the deterministic postlude — with all state in the scratch working tree and references fetched over the guest's own MCP route.

## Anatomy

| Piece | Where |
| ----- | ----- |
| Host binary | [`crates/eval-driver`](../../crates/eval-driver) — `omnia::runtime!({ mode: command, hosts: { WasiHttp: HttpDefault, WasiModel: Cursor } })` |
| Driver guest | [`crates/eval-guest`](../../crates/eval-guest) — the deployment's `wasi:cli/run` exporter; reads the slice inputs from the mount, dispatches `target.build` by adapter id, prints the report as one JSON line |
| Adapter guest | [`targets/vectis`](../../targets/vectis) — the component under test (`specify_vectis.wasm`) |
| Scenario seeds | `scenarios/<name>/inputs/*.md` (typed slice inputs by file stem) and `scenarios/<name>/seed/**` (files copied into the scratch project root: `.specify/project.yaml` platform set, operator-curated `design-system/` manifests) |
| Runner | `run.sh` (one scenario per invocation) — `cargo make eval-vectis` runs it |
| Results | `runs/` — per-run raw output |

## Scenarios

| Scenario | Slice | Shape |
| -------- | ----- | ----- |
| `single-screen` | `daily-quote` | A tiny single-screen feature (one read-only quote screen with a refresh action) on a `core + ios` platform set — one composition leg, one Crux core leg, one iOS shell leg, review, report. The Android leg is skipped by the declared platform set; the seed's `assets.yaml` is symbol-only so the materialize prelude reports `skipped: true`. |

The scenario is deliberately minimal: it proves the session-less leg decomposition and the deterministic prelude / validator-gate / postlude bracket against a live model, not the full fixture depth of `targets/vectis/tests/`. The spawned agent's host-command loops (cargo / xcodebuild / make) may degrade gracefully inside the sandboxed scratch tree; the report and the composition gate are the assertions that matter.

## Running

Requires [`cursor-agent`](https://cursor.com/docs/cli) on `PATH`, authenticated via `CURSOR_API_KEY` or a prior `cursor-agent login`.

```bash
# the scenario through cargo-make
cargo make eval-vectis

# directly
evals/vectis/run.sh single-screen
```

The runner builds the guests, seeds a scratch tree under a temp dir, writes the deployment manifest, and drives one command-mode run. The report JSON line and full log land under `runs/<scenario>/`; exit status carries the report's `status`.

## Smoke-checking without a model

```bash
DRY_RUN=1 evals/vectis/run.sh single-screen
```

`DRY_RUN=1` exercises everything up to the model seam — the wasm32 guest builds, the scratch tree seeding, and the deployment-manifest generation — then exits before spawning the driver, so no `cursor-agent` is needed.
