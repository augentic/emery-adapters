# Omnia target — build prompt

> The omnia adapter core inlines this document into the system prompt of every build leg — preparation, generation, capture replay, and standards review — alongside the leg's own prompt under [`build/`](build/). Leg sequencing lives in the adapter core (`src/operations.rs`), not here: each leg's user prompt names the sections of this document to follow. Synthesis idioms (provider DI, WASM guardrails, error variants, validation placement) live in [`guidance.md`](guidance.md) and must already be reflected in the slice's `specs/<domain>/spec.md` + `design.md` before the build runs.

## Inputs and bindings

The build runs against the build request the CLI prepared at `.emery/slices/<slice>/build/request.yaml`; consume its `inputs` manifest rather than relying on convention. Every artifact path resolves against `inputs.root` (the slice tree).

- `inputs.artifacts.proposal` (`proposal.md`) — domain inventory and slice scope.
- `inputs.artifacts.specs[]` (`specs/<domain>/spec.md`) — behavioural requirements, one file per `proposal.md ## Domains` entry.
- `inputs.artifacts.design` (`design.md`) — domain model, provider DI, error variants, and WASM idioms (see [`guidance.md`](guidance.md)).
- `inputs.artifacts.tasks` (`tasks.md`) — implementation sequencing and progress tracking.
- `inputs.artifacts.additional[]` — empty for omnia: the adapter's `metadata` record declares no extra slice-tree inputs. Omnia reads the project working tree's `Cargo.toml` directly for workspace context; that is not a slice-tree input.

These working names, bound from the request and the resolved crate, are used throughout:

```text
$SLICE_NAME    = active in-progress plan entry's slice name (from `emery plan next`)
$SLICE_DIR     = .emery/slices/$SLICE_NAME
$DOMAIN_NAME   = domain slug from proposal.md ## Domains (typically equals crate name for single-crate slices)
$SPEC_PATH     = $SLICE_DIR/specs/$DOMAIN_NAME/spec.md
$DESIGN_PATH   = $SLICE_DIR/design.md
$TASKS_PATH    = $SLICE_DIR/tasks.md
$CRATE_NAME    = $SLICE_NAME with kebab → snake (or the slice's plan-level `crate:` override)
$CRATE_PATH    = crates/$CRATE_NAME
$GUEST_NAME    = deployable guest package name (kebab-case; the root `[package].name` in `Cargo.toml`)
$GUEST_PATH    = . (workspace root — the guest is the root package; `src/lib.rs` exports HTTP / Messaging / WebSocket)
$REVIEW_OUTPUT = $CRATE_PATH/REVIEW.md
```

`$SLICE_NAME` arrives in each leg's user prompt, taken from the build request.

## Mode detection

Check whether `$CRATE_PATH/Cargo.toml` exists:

- **Missing** → **create mode**: generate the crate, tests, and (if workspace-root `src/lib.rs` is absent) the root-package guest scaffolding.
- **Present** → **update mode**: incremental change against the existing crate; guest wiring updates are folded into the crate-writer step (skip the guest phase). Preserve any legacy non-root guest layout the consumer already has.

## § Exemplar checkout

The build's first leg is the preparation leg ([`build/prepare.md`](build/prepare.md)): it produces the read-only exemplar checkout at `target/omnia-exemplar/` (clone or refresh unpinned `main`, proceed noted-stale when only the refresh fails, stop with a `## § Stop hint contract` hint when no checkout can be obtained). [`exemplar.md`](../references/exemplar.md) carries the compatibility contract: create mode adopts the checkout's `exemplar.yaml` Omnia `{ version, repository, rev }` when authoring dependencies; update mode preserves the consumer's existing pin, soft-warns on mismatch, and prefers consumer-evidenced idioms over exemplar idioms wherever the two conflict. Writer prompts link into `exemplar.md`'s navigation map rather than repeating any of this.

## Leg map

Between the preparation leg and generation, the adapter runs its deterministic scaffold prelude in-guest: it strictly validates the checkout's template contract (`exemplar.yaml` → `templates/guest/manifest.yaml`), then writes any missing standard tooling file (cargo-make, deny, cargo-vet scaffold, GitHub workflows, toolchain/editor config) from the checkout — existing files are never overwritten, and a missing or malformed checkout or a prelude I/O failure fails the build before generation. The generation user prompt carries the outcome as a `### scaffold prelude` block; never re-author the files it lists ([`configuration.md`](../references/configuration.md) describes them).

The adapter core drives the legs in a fixed order — preparation ([`build/prepare.md`](build/prepare.md), the exemplar checkout), generation (crate writer, test writer, guest writer in create mode, then the § verify-repair loop), capture replay ([`build/replay.md`](build/replay.md), dispatched only when the build context binds a `captures` source — the adapter skips it in-guest otherwise, with no leg spawned), then standards review ([`build/review.md`](build/review.md)), which closes the build: it drives the remediation cycle, marks the completed `tasks.md` checkboxes, and its answer carries the findings synthesis and output declaration the adapter assembles the build report from in-guest (see `## Build report`) — there is no separate report leg. Within the generation leg, write the crate before the tests, mark `tasks.md` checkboxes complete as each task lands, and never transition the slice lifecycle — the deterministic in-guest report gate checks the assembled report and the engine guest owns the `Refined → Built` transition.

## § Verify-repair loop (max 3 iterations)

Run after both crate writer and test writer have completed. Each iteration runs the four checks below; if any fail, classify the failure, apply the targeted fix, and start a new iteration.

```bash
cd $CRATE_PATH && cargo fmt --check
cd $CRATE_PATH && cargo check
cd $CRATE_PATH && cargo clippy -- -D warnings
cd $CRATE_PATH && cargo test
```

If `cargo fmt --check` fails, run `cargo fmt` once. Formatting is mechanical; one pass suffices.

If `cargo check` or `cargo clippy` fails, re-enter [`build/crate.md`](build/crate.md) with the error output as context. Apply minimum-change repair discipline (see [`repair-patterns.md`](../references/repair-patterns.md)).

If `cargo test` fails, classify each failure:

| Failure signal | Classification | Fix action |
|---|---|---|
| Error in `tests/` paths, `MockProvider`, or `provider.rs` | Test issue | Re-enter [`build/test.md`](build/test.md) |
| Error in `src/` paths, missing trait impls in production | Code issue | Re-enter [`build/crate.md`](build/crate.md) |
| Assertion mismatch where *actual* matches spec | Test issue | Test expectation is stale |
| Assertion mismatch where *expected* matches spec | Code issue | Handler returns the wrong result |
| MockProvider missing a trait impl the handler now requires | Test issue | Update MockProvider |
| Unresolved import or missing crate in `Cargo.toml` | Workspace issue | Fix `Cargo.toml` paths or workspace member list directly |

**Repair discipline.** Minimum change only — fix the reported error and nothing else. Scope the diff to files and functions named in the error output. Group failures by classification and re-enter each writer prompt once with all same-class errors. Full repair recipes: [`repair-patterns.md`](../references/repair-patterns.md).

**Update-mode regression check.** Before iteration 1, record the baseline: `cd $CRATE_PATH && cargo test 2>&1 | tee /tmp/${SLICE_NAME}-${CRATE_NAME}-baseline.txt`. After each iteration, for each test that passed before and now fails: if the spec explicitly changes the asserted behaviour → expected behavioural change, re-enter test writer to align expectations; if the spec does not change the asserted behaviour → true regression, route the fix through the classification table.

Repeat until all four checks pass or 3 iterations exhausted. If still failing after 3 iterations: **STOP**. Surface the stop hint below with full error output and do not transition the slice — the standards-review leg maps the remaining failures as blocking findings, so the assembled build report is `status: failure` and parks the slice for human review.

## § Stop hint contract

A build failure surfaces a stop hint as the body's final output — a single structured message the parent skill or the parent loop can act on without re-deriving context:

- `slice` — slice name from `emery plan next`.
- `phase` — `build`.
- `failing-task` — the `tasks.md` checkbox (or sub-step) that exited non-zero.
- `log-path` — absolute path to the captured stdout/stderr.
- `next-action` — typically `re-run /emery:build $SLICE after fix`.

Render the hint as the final visible output of the run, alongside the blocking findings that make the assembled build report `status: failure` (see `## Build report`). Never write the lifecycle yourself — the deterministic in-guest report gate checks the assembled report and the engine guest owns the lifecycle, so the slice stays `refined` and the loop (or a re-invocation) re-enters cleanly.

## § Standards review surface

The standards-review leg writes `$REVIEW_OUTPUT` (`REVIEW.md`) — the model-assisted surface: specialist + antagonist judgment per [`team-protocol-crate.md`](../references/team-protocol-crate.md) and [`build/review.md`](build/review.md), applying the engineering-standards rules shipped under [`../rules/`](../rules/) (the Omnia overlay plus the shared `UNI-*` pack at `rules/universal/`).

Per [Standards layer](../references/emery-runtime/standards-layer-snippet.md), standards findings may block CI but never transition plan entries, slices, or changes. CI wiring is consumer-project policy, not adapter policy; this prompt acknowledges the surface and links out for the contract.

## Build report

The build report is assembled **in-guest** from the standards-review leg's schema-gated answer — no report leg is spawned and no report file is written. The review answer's `## Build close-out` (see [`build/review.md`](build/review.md)) carries the report's judgmental residue: the findings left unresolved after the remediation cycle and the declared build outputs (the slice's crate tree, plus the guest scaffolding in create mode, as `platform: core` paths relative to the project root). Never transition the slice lifecycle — the deterministic in-guest report gate checks the assembled report's coherence against the working tree and the engine guest owns the `Refined → Built` transition.

**Status is derived, never judged.** The assembled report is `status: success` iff the review answer carries no blocking (`critical` / `important`) finding and every declared output exists in the working tree; the deterministic gate adds a blocking finding for any declared-but-missing output. A build that cannot succeed — an exhausted verify-repair budget, unresolved blocking review findings, replay failures the review confirms — must carry at least one blocking finding in the review answer.

- **Clean build** — the verify-repair loop passes (`cargo fmt --check`, `cargo check`, `cargo clippy -- -D warnings`, `cargo test`), the code-review remediation cycle leaves no unresolved `critical` / `important` findings in `REVIEW.md`, and replay passes when the build context binds `captures` → an answer with no blocking findings assembles as `status: success`.
- **Unresolved build** — the verify-repair budget is exhausted (3 iterations) or the review remediation cycle cannot clear its blocking findings → blocking findings in the review answer assemble as `status: failure`.

Each review-answer finding carries `title`, `severity`, `impact`, and `remediation` (plus `rule-id` when it cites a codex rule); the adapter folds them into the engine's report findings. Map omnia's verify-repair, `REVIEW.md`, and replay findings into that shape.

## References

- [`guidance.md`](guidance.md), [`merge.md`](merge.md) — sibling prompts.
- [`build/prepare.md`](build/prepare.md), [`build/crate.md`](build/crate.md), [`build/test.md`](build/test.md), [`build/guest.md`](build/guest.md), [`build/replay.md`](build/replay.md), [`build/review.md`](build/review.md) — per-leg prompts.
- [`../../../sources/captures/prose/references/capture-format.md`](../../../../sources/captures/prose/references/capture-format.md) — runtime capture wire format (when `captures` is bound).
- [`exemplar.md`](../references/exemplar.md) — the exemplar checkout: contract, compatibility behavior, navigation map.
- [`hard-rules.md`](../references/hard-rules.md) — full authority hierarchy and hard-rules set.
- [`guardrails.md`](../references/guardrails.md), [`wasm-constraints.md`](../references/wasm-constraints.md) — forbidden crates / APIs, statelessness, serde / DST idioms.
- [`capabilities.md`](../references/capabilities.md), [`capability-mapping.md`](../references/capability-mapping.md) — provider traits and artifact-to-trait mapping.
- [`sdk-api.md`](../references/sdk-api.md), [`cargo-toml.md`](../references/cargo-toml.md), [`error-handling.md`](../references/error-handling.md), [`configuration.md`](../references/configuration.md) — SDK / workspace / error / guest-config templates.
- [`cross-cutting-matrices.md`](../references/cross-cutting-matrices.md), [`update-patterns.md`](../references/update-patterns.md), [`change-classification.md`](../references/change-classification.md), [`repair-patterns.md`](../references/repair-patterns.md), [`todo-markers.md`](../references/todo-markers.md), [`checklists.md`](../references/checklists.md), [`output-documents.md`](../references/output-documents.md) — analysis tables, strategy patterns, recipes.
- [`mock-provider.md`](../references/mock-provider.md), [`spec-to-test-mapping.md`](../references/spec-to-test-mapping.md), [`replay-fixtures.md`](../references/replay-fixtures.md), [`replay-crate-layout.md`](../references/replay-crate-layout.md) — test depth.
- [`guest-patterns.md`](../references/guest-patterns.md), [`guest-wiring.md`](../references/guest-wiring.md), [`runtime.md`](../references/runtime.md), [`project-layout.md`](../references/project-layout.md) — guest depth.
- [`review-categories.md`](../references/review-categories.md), [`team-protocol-crate.md`](../references/team-protocol-crate.md), [`review-auto-fix.md`](../references/review-auto-fix.md), [`review-output-template.md`](../references/review-output-template.md), [`agent-teams.md`](../references/agent-teams.md), [`../rules/`](../rules/) (Omnia overlay), [`../rules/universal/`](../rules/universal/) (shared `UNI-*` pack, embedded in this adapter) — review depth.
- [`providers/README.md`](../references/providers/README.md) — per-trait selection notes (compiling usage lives in the exemplar).
- [`examples/`](../references/examples/) — retained walkthroughs only for subjects the exemplar does not yet demonstrate (see that folder's README).
