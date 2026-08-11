# Omnia build — close-out (phase report)

Loaded with [`../build.md`](../build.md) for the close-out leg — the build's final leg, after generation (and capture replay, when the build context binds `captures`). It writes no product code, runs no checks, and reviews no standards: verification and standards review are separate engine-dispatched operations ([`../verify.md`](../verify.md), [`../review.md`](../review.md)). This leg has exactly three jobs, then answers the build's phase report.

## 1. Mark `tasks.md` checkboxes — stage copy only

The lent workspace record carries a writable **artifact stage** seeded from the slice tree; the user prompt names the exact stage path for `tasks.md`. Check off every completed task; leave genuinely unfinished tasks unchecked.

- Write **only** the staged `tasks.md` — it is omnia's sole declared writable slice artifact; a staged write outside that grant fails the build at an engine gate.
- Never write the authoritative slice tree (`.emery/slices/...`): the engine validates and promotes the staged diff only after the whole build loop succeeds.

## 2. Declare outputs

List the build outputs in the answer's `outputs[]`: the slice's crate tree (`$CRATE_PATH`) and, when this build wrote the guest scaffolding (create mode), the workspace-root guest files — each as `platform: core` with a path relative to the workspace root. Declare only paths the workspace actually contains; the engine fails the build on a declared-but-missing output. Only `build` declares outputs — later phases changing code beneath a declared output do not change the declaration.

## 3. Synthesise the generation pass's findings

Fold what this build could not resolve into the answer's `findings[]`:

- a stale exemplar checkout the preparation leg proceeded with (non-blocking, `suggestion`);
- unresolved capture-replay failures (the replay outcome rides the user prompt's `Phase outcomes` block);
- gaps the writers could not close — artifacts demanding behaviour the generated code does not carry.

Each finding uses the full diagnostic shape: `title`, `severity` (`critical` / `important` / `suggestion` / `optional`), `source: model-assisted`, `kind` (default `violation`), `artifact` (`code` / `tests` / `tasks` / …), optional `location` (`path` + `line`), `evidence` (usually `kind: snippet`), `impact`, `remediation`, and `rule-id` when it cites a codex rule. A build that cannot produce its candidate must carry at least one blocking (`critical` / `important`) finding. Do **not** report check-suite or standards findings — those passes have not run yet.

**Deferred requirements are out of scope.** The build request's `deferred[]` set (RFC-86a D4) excluded those requirements from the build's obligations: a deferred requirement's absence is not a gap finding, the workspace must carry no implementation, scaffolding, placeholders, or TODO markers for one, and the answer must never claim a deferred id in its coverage declaration (`covered[]`, where the report shape carries one) — the engine's report gate rejects a report that claims coverage of a deferred requirement (`target-build-deferred-covered`).

## Phase report contract

Answer with one phase report:

- `outcome: completed` (`not-applicable` never applies to an omnia build), `source: model-assisted`.
- `outputs[]` and (omnia: omitted) `ui-surface` per `## 2`.
- `written[]` audit entries for what this build touched: `root: artifacts` with path `tasks.md` for the staged checkbox write, `root: workspace` with workspace-relative paths for the product code the writers produced (the top-level trees suffice; RFC-87 capture remains the authoritative write record).
- No continuation payload rides the answer.

The engine owns the loop, budgets, terminal report, and lifecycle: never claim success past a blocking finding, select a next operation, or transition the slice.
