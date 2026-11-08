# Logical parts and the build schedule

A programme of the eval, run and read under [the protocol](README.md#protocol); [the README](README.md#rounds) indexes every round.

## Logical parts at specify, budgeted slices at build

Across the three evaluation cards of 2026-11-02 through 2026-11-04 (`2026-11-02T22-43-58Z`, `2026-11-04T00-48-18Z`, `2026-11-04T23-53-01Z`), `emery specify` ran 204 `slicing` turns: 181 answered `ok` on the first attempt, 18 required retry rounds (`corrected`), and 5 failed completely (`exhausted`), discarding every extraction, grouping, spec-draft, and design-draft completion before them. Four of the five exhaustions were caused by `SLICE_CAP = 12` and its `verify_cut` pair-merge rule (`express-orders` run 1 at 12 completions and 428K input tokens; `fastapi-routers` runs 1 and 2 at 11 and 12 completions and 771K and 974K input tokens; `api-routes` run 3), either because a cross-stem route (`start.health-ready-http` on `GET /health/ready`) trapped `health-ready` into being paired with a `start` slice or because pair-by-pair merging exhausted the retry budget when the model started with one slice per route. Even when accepted (`express-orders` run 3), asking the model to split `start` and `orders` below 12 requirements forced it to invent intra-stem `depends-on` edges, chaining 7 slices across 6 waves (`1/1/1/1/1/2`).

The arm separates logical domain architecture at `specify` from turn-budget slicing at `build` across the engine (`emery` 0.39.0, revision grammar `9`) and this repository (`0.12.0`):

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

Seven cards across 2026-11-06 and 2026-11-08, all `composer-2.5`:

- **Arm A (`specify`, `EVAL_RUNS=3`):** `2026-11-06T04-25-50Z` over all four cases (`api-routes`, `express-orders`, `fastapi-routers`, `nestjs-pair`, 12 runs) and `2026-11-06T05-11-53Z` re-running `express-orders` and `fastapi-routers` 46 minutes later (6 runs, 18 `slicing` turns in all), from the working tree that became `#397` (`emery`) and `#197` (`emery-adapters`).
- **Arm B seed (`--build --target omnia --budget 12`):** `2026-11-06T05-28-13Z` over `express-orders` (`EVAL_RUNS=1`). `specify` sealed revision `44ac6b81d04c7ffe6d0a1cb635dc01e06d3095553e10c09096f51522d446ecde` (66 requirements, 26 criteria, 4 parts in 3 waves `1/2/1`), and `build` stopped at `0/4 verify-failed` in wave 1 (`65s`, exit `1`) because the seed's `common` crate required `omnia-sdk = "^0.38.0"` before `0.38.0` was published to crates.io. Its sealed revision `44ac6b81` is the base every Arm B rebuild runs over.
- **Arm B `node` reading (`--rebuild 2026-11-06T05-28-13Z --target node`):** `2026-11-06T05-41-17Z` at `--budget 12` and `2026-11-06T05-50-46Z` at `--budget 6`, both on 2026-11-06 over sealed revision `44ac6b81`.
- **Arm B `omnia` reading (`--rebuild 2026-11-06T05-28-13Z --target omnia`):** `2026-11-08T21-05-11Z` at `--budget 12` and `2026-11-08T21-37-41Z` at `--budget 6`, both on 2026-11-08 over sealed revision `44ac6b81`.

#### Arm A — `## Plan shape` across 18 `specify` runs

| card | case | run | slicing | cap exhausted | parts | waves | widest | merged stems | type-backed edges | unbacked edges | unbacked onto start |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `04-25-50Z` | `api-routes` | 1 | ok (24s) | 0 | 3 | 3 (`1/1/1`) | 1 | 0 | 0 | 2 (`health` → `orders`, `orders` → `customers`) | 0 |
| `04-25-50Z` | `api-routes` | 2 | ok (18s) | 0 | 3 | 3 (`1/1/1`) | 1 | 0 | 0 | 2 (`health` → `orders`, `orders` → `customers`) | 0 |
| `04-25-50Z` | `api-routes` | 3 | ok (19s) | 0 | 3 | 3 (`1/1/1`) | 1 | 0 | 0 | 2 (`health` → `orders`, `orders` → `customers`) | 0 |
| `04-25-50Z` | `express-orders` | 1 | ok (20s) | 0 | 3 | 2 (`2/1`) | 2 | 1 (`start`: `health` + `start`) | 0 | 1 (`orders` → `customers`) | 0 |
| `04-25-50Z` | `express-orders` | 2 | corrected (37s) | 0 | 3 | 3 (`1/1/1`) | 1 | 1 (`startup`: `health` + `start`) | 0 | 3 (`customers` → `startup`, `orders` → `startup`, `orders` → `customers`) | 0 |
| `04-25-50Z` | `express-orders` | 3 | ok (26s) | 0 | 3 | 3 (`1/1/1`) | 1 | 1 (`start`: `health` + `start`) | 1 (`orders` → `customers` (`Address`)) | 2 (`customers` → `start`, `orders` → `start`) | 2 |
| `04-25-50Z` | `fastapi-routers` | 1 | ok (17s) | 0 | 4 | 3 (`1/2/1`) | 2 | 0 | 0 | 4 (`customers` → `start`, `health` → `start`, `orders` → `start`, `orders` → `customers`) | 3 |
| `04-25-50Z` | `fastapi-routers` | 2 | ok (21s) | 0 | 4 | 3 (`1/2/1`) | 2 | 0 | 0 | 4 (`customers` → `start`, `health` → `start`, `orders` → `start`, `orders` → `customers`) | 3 |
| `04-25-50Z` | `fastapi-routers` | 3 | ok (33s) | 0 | 3 | 3 (`1/1/1`) | 1 | 1 (`server-foundation`: `health` + `start`) | 0 | 3 (`customers` → `server-foundation`, `orders` → `server-foundation`, `orders` → `customers`) | 0 |
| `04-25-50Z` | `nestjs-pair` | 1 | ok (47s) | 0 | 10 | 5 (`2/4/1/2/1`) | 4 | 0 | 3 (`files` → `start` (`AppConfig`), `users` → `authentication` (`User`), `users` → `files` (`FileDto`)) | 12 (`authentication` → `database`, `authentication` → `serialization`, `authentication` → `translations`, `authentication` → `start`, `database` → `start`, `files` → `authentication`, `serialization` → `start`, `translations` → `start`, `run-seed` → `authentication`, `run-seed` → `database`, `home` → `start`, `users` → `serialization`) | 5 |
| `04-25-50Z` | `nestjs-pair` | 2 | ok (64s) | 0 | 9 | 5 (`4/2/1/1/1`) | 4 | 1 (`authentication-and-users`: `auth` + `users`) | 2 (`authentication-and-users` → `files` (`FileDto`, `FileEntity`, `FileSchemaClass`, `FileType`), `files` → `application-bootstrap` (`AppConfig`, `FileConfig`)) | 9 (`authentication-and-users` → `database`, `authentication-and-users` → `serialization`, `authentication-and-users` → `translations`, `authentication-and-users` → `application-bootstrap`, `database` → `application-bootstrap`, `files` → `database`, `home` → `application-bootstrap`, `run-seed` → `authentication-and-users`, `run-seed` → `database`) | 3 |
| `04-25-50Z` | `nestjs-pair` | 3 | ok (45s) | 0 | 11 | 4 (`6/2/1/2`) | 6 | 0 | 2 (`auth` → `users` (`User`, `UserEntity, UserSchemaClass`), `users` → `files` (`FileDto`, `FileEntity`, `FileSchemaClass`, `FileType`)) | 9 (`auth` → `database`, `auth` → `start`, `files` → `database`, `files` → `start`, `home` → `start`, `users` → `database`, `users` → `serialization`, `run-seed` → `database`, `run-seed` → `users`) | 3 |
| `05-11-53Z` | `express-orders` | 1 | ok (20s) | 0 | 4 | 3 (`1/2/1`) | 2 | 0 | 1 (`orders` → `customers` (`Address`)) | 3 (`health` → `start`, `customers` → `start`, `orders` → `start`) | 3 |
| `05-11-53Z` | `express-orders` | 2 | ok (20s) | 0 | 3 | 3 (`1/1/1`) | 1 | 1 (`start`: `health` + `start`) | 1 (`orders` → `customers` (`Address`)) | 2 (`customers` → `start`, `orders` → `start`) | 2 |
| `05-11-53Z` | `express-orders` | 3 | ok (33s) | 0 | 4 | 3 (`2/1/1`) | 2 | 0 | 1 (`orders` → `customers` (`Address`)) | 2 (`customers` → `start`, `orders` → `start`) | 2 |
| `05-11-53Z` | `fastapi-routers` | 1 | ok (27s) | 0 | 4 | 2 (`3/1`) | 3 | 0 | 0 | 1 (`orders` → `customers`) | 0 |
| `05-11-53Z` | `fastapi-routers` | 2 | ok (21s) | 0 | 4 | 3 (`2/1/1`) | 2 | 0 | 0 | 3 (`customers` → `start`, `orders` → `start`, `orders` → `customers`) | 2 |
| `05-11-53Z` | `fastapi-routers` | 3 | ok (19s) | 0 | 4 | 2 (`3/1`) | 3 | 0 | 0 | 1 (`orders` → `customers`) | 0 |

Totals across the 18 runs: 17 `ok` first try, 1 `corrected` (`04-25-50Z` `express-orders` run 2), 0 `exhausted` (`cap exhausted` `0` on every run); 7 plans merged a stem pair (6 of them `health` + `start`, 1 `auth` + `users` on `nestjs-pair`); 10 type-backed edges against 71 unbacked edges, 33 of the unbacked pointing onto `start` (or its merged bootstrap part when named `start`).

#### Arm B — `node` reading over sealed revision `44ac6b81` (`express-orders`)

The sealed plan (`2026-11-06T05-28-13Z` run 1) has 4 parts (`PART-001` `start` REQ-001–REQ-012, `PART-002` `customers` REQ-013–REQ-020 owning 4 types, `PART-003` `health` REQ-021–REQ-025, `PART-004` `orders` REQ-026–REQ-066 owning 14 types) in 3 plan waves (`1/2/1`: `customers` and `health` depend on `start`; `orders` depends on `start` and `customers`).

| card | target | budget | slices | waves | widest | over budget | built | verified | build wall | covered | host checks | scenarios named | median turn | max turn | median input | total input |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `05-41-17Z` | `node` | 12 (flag) | 8 | 7 (`1/2/1/1/1/1/1`) | 2 | 0 | 8/8 | 7/7 | 566s | 66/66 | ok 90/90 | 90/90 (100%) · 0 outside · 0 unreached · 0 probes | 58s | 90s | 410k | 3.40M |
| `05-50-46Z` | `node` | 6 (flag) | 12 | 11 (`1/1/2/1/1/1/1/1/1/1/1`) | 2 | 2 (`SLICE-008`, `SLICE-009`) | 12/12 | 11/11 | 1059s | 66/66 | ok 90/90 | 90/90 (100%) · 1 outside (`.verify-before`) · 0 unreached · 0 probes | 71.5s | 107s | 439k | 5.41M |

`## Schedule` at `--budget 12` (`2026-11-06T05-41-17Z`):

| slice | part | sub-stems | types laid | wave | req | scenarios | over budget | duration | input | cached | output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SLICE-001` `start` | `PART-001` | `start.require-database-url` .. `start.map-internal-error` (`REQ-001`–`REQ-012`, 12 untriggered) | 0 | 1 | 12 | 12 | no | 75s | 339494 | 277664 | 13942 |
| `SLICE-002` `customers` | `PART-002` | `customers.get-id`, `customers.post`, `customers.put-id-tier` (`REQ-013`–`REQ-020`) | 4 | 2 | 8 | 8 | no | 53s | 401947 | 366496 | 11923 |
| `SLICE-003` `health` | `PART-003` | `health.get-live`, `health.get-ready` (`REQ-021`–`REQ-025`) | 0 | 2 | 5 | 5 | no | 37s | 173454 | 120096 | 5685 |
| `SLICE-004` `orders` (1/5) | `PART-004` | `orders.get`, `orders.get-id` (`REQ-026`–`REQ-033`) | 14 | 3 | 8 | 14 | no | 71s | 496656 | 440032 | 14945 |
| `SLICE-005` `orders` (2/5) | `PART-004` | `orders.post` (`REQ-034`–`REQ-043`) | 0 | 4 | 10 | 21 | no | 90s | 734714 | 698784 | 22841 |
| `SLICE-006` `orders` (3/5) | `PART-004` | `orders.put-id-lines` (`REQ-044`–`REQ-053`) | 0 | 5 | 10 | 12 | no | 63s | 586532 | 521632 | 13213 |
| `SLICE-007` `orders` (4/5) | `PART-004` | `orders.post-id-pay`, `orders.post-id-ship` (`REQ-054`–`REQ-061`) | 0 | 6 | 8 | 10 | no | 44s | 418269 | 354176 | 7460 |
| `SLICE-008` `orders` (5/5) | `PART-004` | `orders.post-id-cancel` (`REQ-062`–`REQ-066`) | 0 | 7 | 5 | 8 | no | 37s | 249732 | 212800 | 5135 |

`## Schedule` at `--budget 6` (`2026-11-06T05-50-46Z`):

| slice | part | sub-stems | types laid | wave | req | scenarios | over budget | duration | input | cached | output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SLICE-001` `start` (1/2) | `PART-001` | `start.require-database-url` .. `start.unmatched-route` (`REQ-001`–`REQ-006`) | 0 | 1 | 6 | 6 | no | 89s | 482106 | 416544 | 15558 |
| `SLICE-002` `start` (2/2) | `PART-001` | `start.map-schema-validation` .. `start.map-internal-error` (`REQ-007`–`REQ-012`) | 0 | 2 | 6 | 6 | no | 57s | 260738 | 207424 | 7782 |
| `SLICE-003` `customers` (1/2) | `PART-002` | `customers.get-id`, `customers.post` (`REQ-013`–`REQ-017`) | 4 | 3 | 5 | 5 | no | 82s | 460868 | 410112 | 12295 |
| `SLICE-004` `customers` (2/2) | `PART-002` | `customers.put-id-tier` (`REQ-018`–`REQ-020`) | 0 | 4 | 3 | 3 | no | 42s | 203785 | 148096 | 6937 |
| `SLICE-005` `health` | `PART-003` | `health.get-live`, `health.get-ready` (`REQ-021`–`REQ-025`) | 0 | 3 | 5 | 5 | no | 66s | 457881 | 400128 | 8499 |
| `SLICE-006` `orders` (1/7) | `PART-004` | `orders.get` (`REQ-026`–`REQ-030`) | 14 | 5 | 5 | 11 | no | 107s | 1040005 | 988320 | 20195 |
| `SLICE-007` `orders` (2/7) | `PART-004` | `orders.get-id` (`REQ-031`–`REQ-033`) | 0 | 6 | 3 | 3 | no | 74s | 419622 | 308992 | 13894 |
| `SLICE-008` `orders` (3/7) | `PART-004` | `orders.post` (`REQ-034`–`REQ-043`) | 0 | 7 | 10 | 21 | yes | 92s | 616123 | 562240 | 17539 |
| `SLICE-009` `orders` (4/7) | `PART-004` | `orders.put-id-lines` (`REQ-044`–`REQ-053`) | 0 | 8 | 10 | 12 | yes | 89s | 558296 | 431968 | 14730 |
| `SLICE-010` `orders` (5/7) | `PART-004` | `orders.post-id-pay` (`REQ-054`–`REQ-057`) | 0 | 9 | 4 | 5 | no | 69s | 380127 | 336480 | 9159 |
| `SLICE-011` `orders` (6/7) | `PART-004` | `orders.post-id-ship` (`REQ-058`–`REQ-061`) | 0 | 10 | 4 | 5 | no | 50s | 200069 | 132320 | 7371 |
| `SLICE-012` `orders` (7/7) | `PART-004` | `orders.post-id-cancel` (`REQ-062`–`REQ-066`) | 0 | 11 | 5 | 8 | no | 63s | 333105 | 274016 | 9994 |

#### Omnia arm

Over the same sealed revision `44ac6b81` of `2026-11-06T05-28-13Z` (`express-orders`), rebuilt through `omnia` at `--budget 12` (`2026-11-08T21-05-11Z`) and `--budget 6` (`2026-11-08T21-37-41Z`). `start`'s slices (`SLICE-001` at budget 12; `SLICE-001` and `SLICE-002` at budget 6, 12 untriggered requirements) lay no stem and are answered by `Plan::is_empty` with no turn (`0s`, `0` tokens), so `median turn` and `median input` list the active slice build turns first (`7` at budget 12, `10` at budget 6) and all scheduled slices in parentheses; `total input` lists the slice build turns first and all build and verify completions (`14` at budget 12, `21` at budget 6) in parentheses.

| card | target | budget | slices | waves | widest | over budget | built | verified | build wall | covered | host checks | scenarios named | oracle | median turn | max turn | median input | total input |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `21-05-11Z` | `omnia` | 12 (flag) | 8 | 7 (`1/2/1/1/1/1/1`) | 2 | 0 | 8/8 | 7/7 | 1278s | 54/66 | ok 82/82 | 78/78 (100%) · 0 outside · 0 unreached · 0 probes | 29/57 (10 · 3 · 12 · 4) | 81s (80s) | 160s | 1.14M (1.04M) | 10.24M (16.93M) |
| `21-37-41Z` | `omnia` | 6 (flag) | 12 | 11 (`1/1/2/1/1/1/1/1/1/1/1`) | 2 | 2 (`SLICE-008`, `SLICE-009`) | 12/12 | 11/11 | 1826s | 42/66 | ok 82/82 | 65/65 (100%) · 1 outside (`.tmp/before`) · 0 unreached · 0 probes | 28/57 (9 · 3 · 12 · 4) | 69s (61s) | 163s | 782k (713k) | 12.60M (23.10M) |

`## Schedule` at `--budget 12` (`2026-11-08T21-05-11Z`):

| slice | part | sub-stems | types laid | wave | req | scenarios | over budget | duration | input | cached | output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SLICE-001` `start` | `PART-001` | `start.require-database-url` .. `start.map-internal-error` (`REQ-001`–`REQ-012`, 12 untriggered) | 0 | 1 | 12 | 12 | no | 0s | 0 | 0 | 0 |
| `SLICE-002` `customers` | `PART-002` | `customers.get-id`, `customers.post`, `customers.put-id-tier` (`REQ-013`–`REQ-020`) | 4 | 2 | 8 | 8 | no | 89s | 1385922 | 1302976 | 15243 |
| `SLICE-003` `health` | `PART-003` | `health.get-live`, `health.get-ready` (`REQ-021`–`REQ-025`) | 0 | 2 | 5 | 5 | no | 64s | 687896 | 638336 | 8565 |
| `SLICE-004` `orders` (1/5) | `PART-004` | `orders.get`, `orders.get-id` (`REQ-026`–`REQ-033`) | 14 | 3 | 8 | 14 | no | 111s | 1249083 | 1178816 | 22515 |
| `SLICE-005` `orders` (2/5) | `PART-004` | `orders.post` (`REQ-034`–`REQ-043`) | 0 | 4 | 10 | 21 | no | 160s | 4427110 | 4324032 | 31777 |
| `SLICE-006` `orders` (3/5) | `PART-004` | `orders.put-id-lines` (`REQ-044`–`REQ-053`) | 0 | 5 | 10 | 12 | no | 81s | 1144230 | 1071232 | 16006 |
| `SLICE-007` `orders` (4/5) | `PART-004` | `orders.post-id-pay`, `orders.post-id-ship` (`REQ-054`–`REQ-061`) | 0 | 6 | 8 | 10 | no | 79s | 940716 | 861760 | 13554 |
| `SLICE-008` `orders` (5/5) | `PART-004` | `orders.post-id-cancel` (`REQ-062`–`REQ-066`) | 0 | 7 | 5 | 8 | no | 59s | 409123 | 345216 | 8806 |

`## Schedule` at `--budget 6` (`2026-11-08T21-37-41Z`):

| slice | part | sub-stems | types laid | wave | req | scenarios | over budget | duration | input | cached | output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `SLICE-001` `start` (1/2) | `PART-001` | `start.require-database-url` .. `start.unmatched-route` (`REQ-001`–`REQ-006`) | 0 | 1 | 6 | 6 | no | 0s | 0 | 0 | 0 |
| `SLICE-002` `start` (2/2) | `PART-001` | `start.map-schema-validation` .. `start.map-internal-error` (`REQ-007`–`REQ-012`) | 0 | 2 | 6 | 6 | no | 0s | 0 | 0 | 0 |
| `SLICE-003` `customers` (1/2) | `PART-002` | `customers.get-id`, `customers.post` (`REQ-013`–`REQ-017`) | 4 | 3 | 5 | 5 | no | 70s | 837564 | 777280 | 12860 |
| `SLICE-004` `customers` (2/2) | `PART-002` | `customers.put-id-tier` (`REQ-018`–`REQ-020`) | 0 | 4 | 3 | 3 | no | 53s | 367027 | 318848 | 7581 |
| `SLICE-005` `health` | `PART-003` | `health.get-live`, `health.get-ready` (`REQ-021`–`REQ-025`) | 0 | 3 | 5 | 5 | no | 54s | 473615 | 426112 | 9260 |
| `SLICE-006` `orders` (1/7) | `PART-004` | `orders.get` (`REQ-026`–`REQ-030`) | 14 | 5 | 5 | 11 | no | 112s | 1738145 | 1656576 | 19849 |
| `SLICE-007` `orders` (2/7) | `PART-004` | `orders.get-id` (`REQ-031`–`REQ-033`) | 0 | 6 | 3 | 3 | no | 68s | 725733 | 671424 | 8618 |
| `SLICE-008` `orders` (3/7) | `PART-004` | `orders.post` (`REQ-034`–`REQ-043`) | 0 | 7 | 10 | 21 | yes | 163s | 4549851 | 4458112 | 33687 |
| `SLICE-009` `orders` (4/7) | `PART-004` | `orders.put-id-lines` (`REQ-044`–`REQ-053`) | 0 | 8 | 10 | 12 | yes | 107s | 1267807 | 1187776 | 16930 |
| `SLICE-010` `orders` (5/7) | `PART-004` | `orders.post-id-pay` (`REQ-054`–`REQ-057`) | 0 | 9 | 4 | 5 | no | 40s | 413217 | 351616 | 6610 |
| `SLICE-011` `orders` (6/7) | `PART-004` | `orders.post-id-ship` (`REQ-058`–`REQ-061`) | 0 | 10 | 4 | 5 | no | 49s | 699556 | 623424 | 8785 |
| `SLICE-012` `orders` (7/7) | `PART-004` | `orders.post-id-cancel` (`REQ-062`–`REQ-066`) | 0 | 11 | 5 | 8 | no | 103s | 1525895 | 1456256 | 13102 |

Both `omnia` builds merged every serial slice into one logical module per part (`crates/customers/src/customers.rs`, `crates/health/src/health.rs`, `crates/orders/src/orders.rs`, `0` numbered modules, `0` unreached, `0` probes, 12 routes across the 3 modules). As a side reading on the 57-scenario reference oracle ([scenarios.md](scenarios.md#the-express-orders-scenario-oracle)), `21-05-11Z` (`--budget 12`) passed `29/57` (`10/10` customers, `3/3` health, `12/40` orders, `4/4` start) and `21-37-41Z` (`--budget 6`) passed `28/57` (`9/10` customers, `3/3` health, `12/40` orders, `4/4` start, `customers/set-tier-invalid` answering `422` over a Rust enum); because sealed revision `44ac6b81` predates `TABLE_BYTES`, its design carries no `Type: ZONES` fence and the 26 zone-dependent `orders` scenarios fail on the invented `AKL` zone on both builds, while `amend-unknown` and `get-unknown` fail on an invented `data` (`21-05-11Z`) or `payload` (`21-37-41Z`) column.

#### Reading against C1–C5

- **C1 holds.** Across 18 `slicing` turns on the two specify cards, `cap exhausted` is `0`, no run exited in `slicing`, 17 finished `ok` on the first attempt, and 1 was `corrected` (`04-25-50Z` `express-orders` run 2). The correction rate is `1/18` (`5.6%`), below the `8.8%` (`18/204`) baseline, and `slicing` finished `ok` first try on `>= 2/3` runs of every case (`api-routes` 3/3, `express-orders` 2/3 then 3/3, `fastapi-routers` 3/3 then 3/3, `nestjs-pair` 3/3).
- **C2 holds with its justification clause.** `merged stems` is `0` on `>= 2/3` runs of `api-routes` (3/3), `nestjs-pair` (2/3), and `fastapi-routers` (2/3 on `04-25-50Z`, 3/3 on `05-11-53Z`). On `express-orders`, `04-25-50Z` merged `health` into `start` on 3/3 runs and `05-11-53Z` on 1/3 (`6/18` plans across the round merged `health` + `start`, plus `1/18` merging `auth` + `users` on `nestjs-pair`), and every merged part explicitly justified the merge in its brief as C2's exception clause allows.
- **C3 holds.** Under both `omnia` (`21-05-11Z`) and `node` (`05-41-17Z`) at `--budget 12`, `Schedule::new` cuts sealed revision `44ac6b81`'s 4 parts into `8` slices (within `8 ± 1`) across `7` waves (`1/2/1/1/1/1/1`) with `0` slices `over budget`.
- **C4 holds on turn duration, over-budget route handling, single-module layout, and `omnia` median input; partial on `outside` (`1` stray file at `--budget 6`) and `node` median input.** At `--budget 6`, every slice build turn finished well under the `600 s` first-rung cap on both targets: under `omnia` (`21-37-41Z`), max `163 s` (`SLICE-008` `orders.post`, corrected once on `stem_manifest_drift` and `empty_misses`) and median `69 s` across the 10 active slice turns (`61 s` across all 12 slices), against max `160 s` and median `81 s` (`80 s` across 8) at `--budget 12`; under `node` (`05-50-46Z`), max `107 s` (`SLICE-006` `orders` 1/7, laying 14 types) and median `71.5 s`, against max `90 s` and median `58 s` at `--budget 12`. On median input tokens per slice turn, `omnia` dropped from `1.14M` (7 active turns; `1.04M` across 8 slices) at `--budget 12` to `782k` (10 active turns; `713k` across 12 slices) at `--budget 6`, while total slice input rose from `10.24M` to `12.60M` (`16.93M` to `23.10M` across all build and verify completions); under `node`, median input rose from `410k` to `439k` and total input from `3.40M` to `5.41M`. On both targets, the two single-route groups exceeding 6 requirements (`SLICE-008` `orders.post` at 10 requirements / 21 scenarios and `SLICE-009` `orders.put-id-lines` at 10 requirements / 12 scenarios) were flagged `over budget: yes` and built, merged, and verified cleanly into one logical module per part (`crates/orders/src/orders.rs`, `0` numbered modules, `unreached` `0`, `probes` `0`, `host checks` `ok 82/82` on `omnia` and `ok 90/90` on `node`). `outside` was `0` at `--budget 12` on both targets and `1` at `--budget 6` on both (`.tmp/before` left by wave 5's verify turn on `omnia`, `.verify-before` on `node`). Each slice's `scenarios` count is recorded beside its `duration` in `## Schedule` above.
- **C5 holds on the schedule, wave verification, host checks, and `build wall` at `--budget 12`, and triggers Decision 5 on the Arm A edge baseline.** At `--budget 12`, total schedule waves are `7` (`<= 7`, `+1` over the 6-wave `2026-11-04` plan); under `omnia` (`21-05-11Z`), `8/8` slices merged, `7/7` waves verified, `host checks` passed (`ok 82/82`, `0` outside, `0` unreached), and `build wall` was `1278 s` — `286 s` below the `1564–1792 s` `composer-2.5` band (`566 s` under `node`). Across the 18 Arm A plans, `## Plan shape` records `10` type-backed edges against `71` unbacked edges, `33` of them `unbacked onto start`, meeting C5's decision rule trigger for Decision 5.

**Reading under the rule.** The arm holds across Arm A and both targets of Arm B: Option C (`Part` at `specify` with no requirement cap, deterministic `Schedule::new` cutting budget-sized `SLICE-NNN` slices at `build`) and `omnia`'s serial held-module extension stand as the baseline. On C2, all seven stem merges (`6/18` `health` + `start`, `1/18` `auth` + `users`) justified the merge in the part's brief, so `crates/engine/prose/slicing.md`'s merge rule is not tightened under the rule, and the `health` + `start` tendency is recorded. On Decision 1, halving `--budget` from `12` to `6` increased scheduled slices from `8` to `12` (`7` to `10` active turns on `omnia`), verify waves from `7` to `11`, total build input (`16.93M` → `23.10M` on `omnia`, `3.40M` → `5.41M` on `node`), and build wall (`1278 s` → `1826 s` on `omnia`, `566 s` → `1059 s` on `node`), while the heaviest single-route slice (`orders.post`, 10 requirements / 21 scenarios) stayed a single turn at both budgets (`160 s` / `4.43M` and `163 s` / `4.55M` on `omnia`, `90 s` / `735k` and `92 s` / `616k` on `node`); the adapter default budget stays at `12`. On C5, unbacked inter-part edges dominated Arm A (`71` unbacked vs `10` type-backed, `33` onto `start`, serialising `customers` and `health` behind a typeless `start` in wave 1 of `44ac6b81`), firing Decision 5's trigger: inter-part edges move from the model to the engine in the next round below.

## Edges by code

In the round above, the model answered `depends-on` by part name and `Floors::check` verified only that each named part existed and that the graph was acyclic. Across the 18 Arm A plans, `71` of `81` inter-part edges were unbacked by any shared data type (`10` type-backed), and `33` pointed onto `start` (or its bootstrap part), which owns no data types at all and whose slices `omnia` answers with no turn — adding a serial wave before `customers` and `health` can build.

The arm moves inter-part edge construction from the model to the engine (`emery`, revision grammar `10`) and updates the runner's `## Plan shape` columns in this repository:

- **Derived from ownership (`Types`).** `Types::of(&Design)` reads the design's `Role::Data` keys and their `signature` fences. Type `T` names type `U` when `U`'s declared identifier (the identifier after `interface`, `type`, `class`, `struct`, `enum`, `const`, or `def` in the signature, else the key's last segment) appears as a whole identifier token in `T`'s signature (never self, and never when two keys share one declared identifier). Part `P` depends on part `Q` when a type `P` owns names a type `Q` owns. On `express-orders`, `Order` names `Address`, so `orders` → `customers` derives by code; `start` owns no type, so no edge onto it can be derived.
- **Named by the model (`uses`).** `Draft` drops `depends-on` and gains `uses: Vec<String>` (tightened to an `enum` over the design's `Role::Data` keys, `maxItems: 0` when none): design type keys this part's code compiles against without owning. The engine maps each `uses` key to the part that owns it for one more edge. A `uses` key not in the design, owned by the part itself, or owned by no part is a finding.
- **Cycles are findings naming the types.** When derived or `uses`-backed edges form a cycle across parts, `Floors::check` refuses the candidate and quotes the type keys behind each edge in the cycle so the model places mutually referencing types under one owner.
- **Stored and projected.** `Part` stores `uses: Vec<String>` in design order beside `types`; `depends_on: Vec<PartId>` stays on `Part` and is written by the engine from the derived and `uses` edges (`revision::EMERY = 10`, `Uses:` projected after `Types:`). On `amend`, `Pruned::assemble` and `PartAmend` prune `uses` to the design's keys and re-derive `depends_on` through `Types::owner_edges`, removing `spliced`.
- **Known consequence for `omnia`.** Two `/api` stems sharing no type now build in the same wave, so each may lay its own `authenticate` gate; the build still verifies, and the duplication is recorded for the `omnia` programme rather than this round.
- **The runner (`evals/`).** `## Plan shape` reads `uses` from the stored plan and reports `derived edges` (explained by an owned type's signature naming another part's type), `named edges` (explained by `uses` alone), and `onto typeless` (edges whose target owns no type, expected `0`).

The round runs as Arm A (`specify`, `EVAL_RUNS=3`) under `CURSOR_MODEL` pinned to `composer-2.5` over the rebuilt `emery` binary:

```bash
CURSOR_MODEL=composer-2.5 cargo run -p evals -- express-orders fastapi-routers api-routes nestjs-pair
```

Followed by `emery show schedule --budget 12` over the `express-orders` `run-1` staged project to check the scheduled slice and wave count. The predictions and the decision rule are written before the engine or runner changes are built.

### Predictions

- **E1, no edge onto a typeless part.** `onto typeless` is `0` on every run across all four cases (code makes an edge onto a part that owns no type impossible; the clause guards the derivation and the runner's check).
- **E2, the serialisation goes.** On `express-orders`, `health` and `customers` share wave 1 with `start` on `3/3` runs: plan `waves` becomes `3/1` (4 parts) or `2/1` (when `health` + `start` merge) rather than `1/2/1` or `1/1/1`, and `emery show schedule --budget 12` cuts `8 ± 1` slices in `<= 6` waves (against `7` on `44ac6b81`).
- **E3, the type-backed edges stay.** `orders` → `customers` (`Address`) derives on `3/3` `express-orders` runs; across the four cases, the derived edges are a superset of the 10 type-backed edges the 2026-11-06 plans drew, read case by case.
- **E4, the gate does not regress.** `slicing` finishes `ok` on the first attempt on `>= 2/3` runs per case (correction rate at or below `8.8%`), `cap exhausted` is `0`, and `uses` names at most `2` keys per part on `>= 2/3` runs (the model does not rebuild the old hub edges through `uses`).

### Decision rule

- **The arm holds** when E1–E4 hold across the 12 runs; engine-derived edges and `uses` then stand as grammar `10`.
- **E1 or E3 fails:** the derivation rule in `Types` (`declared_name` or token matching over `signature`) missed or mis-matched a type; fix `crates/engine/src/specify/plan/types.rs` and re-run.
- **E4 fails on the correction rate:** the `uses` contract or cycle finding in `crates/engine/prose/slicing.md` is ambiguous; inspect the `candidate rejected` findings in `run-N.stderr`, tighten the `uses` paragraph, and re-run.
- **E4 fails on `uses` volume (`> 2` keys per part on `>= 2/3` runs):** inspect whether the named keys are genuine cross-part references or reconstructed hub edges, and reconsider whether `uses` stays alongside signature-derived edges.
- **E2 fails with E1 and E3 holding:** read against the `merged stems` column and `uses` before changing code or prose.

### Results

Not yet run.
