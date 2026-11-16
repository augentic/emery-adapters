# Change builds and the evolution chain

A programme of the eval, run and read under [the protocol](README.md#protocol); [the README](README.md#rounds) indexes every round.

Over a built ancestor in a revision lineage, `emery build` computes the minimum part scope from typed requirement, departure, type, and plan diffs (`ChangeSet`), asks the target adapter's `Task::Scope` in a read-only working copy at the built commit for design-prose widening (`omnia`'s `change::scope`), skips carried parts (`carried` seeded into `done`), cuts removed parts as retiring slices, and attaches `Slice.change` (`SliceChange`) to every rebuilt and retiring slice. In `emery-omnia`, a stem whose specification changed (`Reached { first: true }`) has `crates/<stem>/` removed before `Scaffold::plan` and is re-laid from the typed slice, a vanished stem (`Vanished`) has its crate and root or sibling list lines removed by code before the turn (answering a retiring slice with no touch points in zero model turns), and every other crate in the workspace is held to its pre-turn AST items and `[dependencies]` lines by `touched_beyond` except at items listed in `TouchPoints`.

Unlike an A/B arm over one frozen revision, an evolution chain runs `emery amend` live at each step, so two chains stage slightly different revisions. Every measure and check below reads each step's own staged amendment, build envelope, `git diff`, and cumulative scenario oracle rather than a fixed expected tree: this card is the record of the change-build flows passing end to end.

## Protocol

The chain case is [`cases/express-orders-evolve/evolution.toml`](../cases/express-orders-evolve/evolution.toml) over base [`express-orders`](../cases/express-orders/expected.toml) (the 57-scenario HTTP oracle of [scenarios.md](scenarios.md#the-express-orders-scenario-oracle)) and target `omnia` ([evals/README.md](../README.md#evolution-chains---evolve)). One chain runs step `00` (`emery specify` + `emery apply` + whole `emery build` + 57-scenario baseline oracle), then seven sequential `emery amend` → `emery apply` → `emery build` steps, driving the cumulative scenario oracle after every build:

```bash
# Dry-run step overlays and scenario counts without spending model turns
cargo run -p evals -- --facts --evolve express-orders-evolve

# Run one full chain under a pinned CURSOR_MODEL
EVAL_RUNS=1 cargo run -p evals -- --evolve --target omnia express-orders-evolve

# Run a prefix of steps or resume a card's chain at step K while debugging a defect
cargo run -p evals -- --evolve --target omnia --steps 00..04 express-orders-evolve
cargo run -p evals -- --evolve --resume <card> --at-step <K> --target omnia express-orders-evolve
```

| Step | Input | Expected scope | Expected crate actions | Oracle scenarios | What it probes |
| --- | --- | --- | --- | --- | --- |
| `00` Baseline | Base `express-orders` fixture | Whole (`no-parent`) | `orders`, `customers`, `health` laid | `57` | Baseline tree and byte-identity reference. |
| `01-bulk-threshold` | Patch: `BULK_DISCOUNT_MIN_UNITS` `24` → `20` | Minimum `[orders]` | `orders` rewrite; `customers`, `health` carried | `58` (`+1 ~2`) | Core crate rewrite, carried byte-identity, and non-regression of carried `orders` scenarios. |
| `02-customer-delete` | Patch: `DELETE /api/customers/:id` (`204`, or `409` with orders) | Minimum `[customers]` | `customers` rewrite; `orders` touch; `health` carried | `61` (`+3`) | Cross-crate touch points (the `/api` gate imported by `orders` from `customers`). |
| `03-returns-stem` | Patch: `POST /api/returns` and `GET /api/returns/:id` mounted in `server.ts` | Minimum `[returns]` | `returns` laid; `orders`, `customers`, `health` carried | `64` (`+3`, `returns` table) | New stem part laid on a change build over carried parts. |
| `04-retire-lines` | Patch: delete `PUT /api/orders/:id/lines` (`scope = "whole"`) | Minimum `[orders]` | `orders` rewrite; `customers`, `returns`, `health` carried | `62` (`+1 -3`) | In-part departure (`retired`), leaving no dead handler from the prior crate. |
| `05-retire-returns` | Patch: remove `src/**/returns.ts` and unmount `returnsRouter` | Retiring `[returns]` | `returns` removed; `orders`, `customers`, `health` carried | `60` (`+1 -3`, 3-table schema) | Whole-part retirement, zero-turn retiring slice, root list line removal, and `dangling_entries` / `dangling_mentions` at verify. |
| `06-rates-decision` | Intent: courier rates called over HTTPS with `x-api-key` header | Minimum `[]`, widened `[orders]` | `orders` rewrite; `customers`, `health` carried | `61` (`+1`, upstream `x-api-key`) | `Task::Scope` precision when only `## APIs and integrations` design prose changes. |
| `07-revert-threshold` | Patch: `BULK_DISCOUNT_MIN_UNITS` `20` → `24` | Minimum `[orders]` | `orders` rewrite; `customers`, `health` carried | `61` (`~3`) | Round-trip revert without LOC accumulation across repeated rewrites. |

### Measures

| Measure | Definition | Read from |
| --- | --- | --- |
| **Correctness** | Oracle pass rate after each build, split into carried scenarios (regressions) and step scenarios (new or updated behaviour). | `step-0K.oracle.log`, `step-0K.fidelity.md` |
| **Locality** | Carried crates byte-identical (`git ls-tree` hash unchanged); no AST item or manifest dependency changed outside rewritten crates except at listed `TouchPoints`; worker shell trace scanned for `git` invocations. | `git diff emery/<prev>..emery/<new>` in `step-0K.locality.md`, `step-0K.build.stderr` |
| **Scope** | `change.rebuilt` (`minimum` vs adapter-`widened`) and `change.retiring` against `step.toml`'s `[scope]`. | `step-0K.build.stdout`, `step-0K.locality.md` |
| **Round trip** | Non-blank Rust LOC, item counts, and test counts per crate; at `step-07`, `crates/orders` is recorded against both `step-00` (the 7-route baseline) and `step-04` (the matching 6-route specification after `PUT /orders/:id/lines` is retired at step 04 and remains retired through step 07) so shrinkage from route retirement is visible alongside rewrite accumulation. | `step-07.locality.md` |
| **Cost** | Specify/amend completions, slice build and verify turns, tool calls (`edit_files`, `write_files`, refused, shell), correction rounds, wave verify repairs, tokens, and wall-clock per step. Recorded, not gated. | Backend trace lines in `step-0K.{specify,amend,build}.stderr` |
| **Amend fidelity** | `added`, `changed`, and `retired` counts, stems, design sections, and added/removed parts against `step.toml`'s `[amend]`, separating specify/amend misses from build-side misses. | `step-0K.amendment.json`, `step-0K.locality.md` |

### Checks

| Check | Requirement |
| --- | --- |
| `C1` | Carried crates are byte-identical on `7` of `7` steps (with `0` touch-point violations outside rebuilt crates). |
| `C2` | Every carried oracle scenario passes after every build (no regression from rewriting). |
| `C3` | Every step's own oracle scenarios pass after its build. |
| `C4` | `crates/orders` non-blank LOC at `step-07` is within `±10%` of `step-00` (recorded alongside `step-07` vs `step-04`). |
| `C5` | `step-06` rebuilds exactly `{orders}` (`minimum []`, `widened [orders]`, `retiring []`). |
| `C6` | `step-05`: `crates/returns` and its root/stem list lines are gone after verify, with `0` model turns spent on the retiring slice. |

### Pass rule

The programme passes when `C1`–`C6` hold on `3` of `3` clean chains run from step `00` at one pair of `emery` and `emery-adapters` commits under one pinned `CURSOR_MODEL`. A resumed chain (`--resume <card> --at-step <K>`) verifies a defect fix before re-running a clean chain and never substitutes for one. A miss on any check is a defect in the engine, the adapter, or `amend`, located by the measures above, fixed with a deterministic regression test, and run again; neither the checks nor the rewrite and locality rule are relaxed. Costs are recorded, not gated.

## Results

| Round | Cards | `emery` | `emery-adapters` | Model | `C1`–`C6` | Verdict |
| --- | --- | --- | --- | --- | --- | --- |

_No live chains recorded yet. Run the three chains under the protocol above and record per-chain step tables, the `C1`–`C6` check matrix, and the read-back here._
