# Logical parts and the build schedule

A programme of the eval, run and read under [the protocol](README.md#protocol); [the README](README.md#rounds) indexes every round.

## Logical parts at specify, budgeted slices at build

Across the three evaluation cards of 2026-11-02 through 2026-11-04 (`2026-11-02T22-43-58Z`, `2026-11-04T00-48-18Z`, `2026-11-04T23-53-01Z`), `emery specify` ran 204 `slicing` turns: 181 answered `ok` on the first attempt, 18 required retry rounds (`corrected`), and 5 failed completely (`exhausted`), discarding every extraction, grouping, spec-draft, and design-draft completion before them. Four of the five exhaustions were caused by `SLICE_CAP = 12` and its `verify_cut` pair-merge rule (`express-orders` run 1 at 12 completions and 428K input tokens; `fastapi-routers` runs 1 and 2 at 11 and 12 completions and 771K and 974K input tokens; `api-routes` run 3), either because a cross-stem route (`start.health-ready-http` on `GET /health/ready`) trapped `health-ready` into being paired with a `start` slice or because pair-by-pair merging exhausted the retry budget when the model started with one slice per route. Even when accepted (`express-orders` run 3), asking the model to split `start` and `orders` below 12 requirements forced it to invent intra-stem `depends-on` edges, chaining 7 slices across 6 waves (`1/1/1/1/1/2`).

The arm separates logical domain architecture at `specify` from turn-budget slicing at `build` across the engine (`emery` 0.39.0, revision grammar `9`) and this repository (`0.14.0`):

- **Stage 1 — Logical parts at `specify` and `amend`:** `SLICE_CAP`, sub-stem floors (`Stem::substems`), the anti-fragmentation gate (`verify_cut`), and plan recutting are removed from `specify` and `amend`. The stored plan unit is the `Part` (`PART-NNN`, `## Part:`), floored by stem and trigger alone with no requirement cap; the model merges stems only when neither can be built and verified apart from the other, assigns each `Role::Data` type key to one part, draws inter-part `depends-on` edges, and writes each part's brief.
- **Stage 2 — Deterministic schedule at `build` and `show schedule`:** `Schedule::new` cuts each part into budget-sized `SLICE-NNN` build slices under `--budget` / `EMERY_BUDGET`, `[target] budget` in `emery.toml`, the target adapter's `metadata().budget`, or `DEFAULT_BUDGET` (`12`), with no model turn — grouping a part's remaining requirements by sub-stem, merging groups that share a trigger, ordering untriggered groups (stages and helpers) before triggered ones (routes and consumers), next-fit packing to the budget, flagging a single group that exceeds the budget `over_budget` without refusing the run, assigning the part's owned data types to its first slice alone, building a part's slices serially while independent parts build in parallel waves, and resuming from `Requirements:` trailers (`Part:` and `Budget:` recorded beside `Slice:`).
- **Stage 3 — Target adapters (`omnia` and `node`):** Both targets declare `budget: 12` in `TargetAdapter::metadata()`. Modules stay named for the logical part (`crates/<stem>/src/<part>.rs`, never numbered); on a serial slice (`index > 1`) of a part whose module already stands in the working copy, `omnia`'s scaffold appends the slice's new handlers, consumers, functions, path constants, and `#[ignore]` scenario tests at the `syn`-located end of the held module and test file, registers each new route or topic in the held `routes()` or `topics(..)` chain, lists the module as `held, extended` in the brief, and holds every handler in a part module to its router through `unregistered_handlers`.
- **The runner (`evals/`):** `--budget <N>` (and `EVAL_BUDGET`) passes `--budget <N>` to `emery build`, records `budget`, `budget_source`, and `schedule` from the build envelope, and renders `## Plan shape` (the `slicing` turn outcome, `cap exhausted`, `parts`, `waves`, `widest`, `merged stems`, `type-backed edges`, `unbacked edges`, and `unbacked onto start`) and `## Schedule` (budget, slices, waves, widest, `over budget` count, and per-slice requirements, `scenarios`, `over budget`, `duration`, and tokens).

The round runs in two arms under `CURSOR_MODEL` pinned to `composer-2.5` over the release `emery` and the Wasm components built from the two working trees:

1. **Arm A (`specify`, `EVAL_RUNS=3`):**
   ```bash
   cargo run -p evals -- express-orders fastapi-routers api-routes nestjs-pair
   ```
2. **Arm B (`build` at budgets `12` and `6`, `EVAL_RUNS=1`):**
   ```bash
   cargo run -p evals -- --build --target omnia --budget 12 express-orders
   cargo run -p evals -- --rebuild <arm-b-12-card> --target omnia --budget 6 express-orders
   ```

The predictions and the decision rule were written before the live runs; the runs are the operator's.

### Predictions

- **C1, no `specify` runs fail due to slice sizing.** Across all 12 runs of `express-orders`, `fastapi-routers`, `api-routes`, and `nestjs-pair`, `cap exhausted` is `0`, no run exits in `slicing`, and `slicing` finishes `ok` on the first attempt on `>= 2/3` runs per case (correction rate at or below the `18/204` = `8.8%` baseline).
- **C2, logical parts match domain stems.** On `>= 2/3` runs of each case, each domain stem becomes one part (`merged stems` is `0` in `## Plan shape`: `4` parts on `express-orders` and `fastapi-routers`, `3` on `api-routes`, `9` on `nestjs-pair`), unless two stems are explicitly merged with a justification in the part's brief.
- **C3, budget 12 matches current slice counts.** On `express-orders` at `--budget 12`, `Schedule::new` cuts the plan's parts into `8 ± 1` (`7–9`) slices in `## Schedule` with `0` slices `over budget`.
- **C4, budget 6 keeps every build turn under the timeout and over-budget routes build cleanly.** On `express-orders` rebuilt at `--budget 6`, every slice build turn finishes under the `600 s` first-rung cap with lower median token usage per turn than at `--budget 12`; single-route groups exceeding 6 requirements (`orders.post` ~10, `orders.put-id-lines` ~7) are flagged `over budget: yes` in `## Schedule` and still build, merge, and verify cleanly into one logical module per part (`crates/orders/src/orders.rs`, `0` numbered modules, `outside` `0`, `unreached` `0`); each slice's `scenarios` count is recorded beside its `duration` for Decision 1's scenario-volume follow-up.
- **C5, serial intra-part waves do not slow down total build time.** On `express-orders` at `--budget 12`, total schedule waves are at most `+1` over the `2026-11-04` plan (`<= 7` waves vs `6`), every wave verifies, `host checks` pass (`ok`), and total `build wall` does not exceed the `1564–1792 s` `composer-2.5` band; `## Plan shape` records `type-backed edges`, `unbacked edges`, and `unbacked onto start` across all four cases as the baseline for Decisions 2 and 5.

### Decision rule

- **The arm holds** when C1–C5 hold across Arm A and Arm B; Option C and the held-module extension then stand as the baseline.
- **C1 or C2 fails (slicing prompt ambiguity or eager stem merging):** inspect the `candidate rejected` findings in `run-N.stderr` and the `merged stems` column in `## Plan shape`; if the model merges unrelated domain stems without justification, tighten the merge rule in `crates/engine/prose/slicing.md` and re-run the affected cases.
- **C3 fails (slice count outside `8 ± 1` at `--budget 12`):** inspect `## Schedule`'s sub-stem and trigger groupings on `express-orders` and adjust the sub-stem grouping in `crates/engine/src/build/schedule.rs`.
- **C4 fails (slices within `--budget 6` still approach or exceed `600 s`):** lower the adapter default budget rather than changing the unit, and use the recorded `scenarios` vs `duration` column in `## Schedule` to scope Decision 1's scenario-volume follow-up.
- **C5 fails (wave count or wall-clock regression from unbacked edges):** read `unbacked edges` and `unbacked onto start` from `## Plan shape`; if unbacked inter-part edges dominate, trigger Decision 5's hybrid follow-up (derive type-backed edges in engine code, refuse edges onto parts that own no types such as `start`, and require any remaining edge to name the file or type it needs in the brief).

### Results

Not yet run. Arm A's specify scorecard and Arm B's two `omnia` build scorecards at `--budget 12` and `--budget 6` are read into this section against C1–C5 when Step 10 runs.
