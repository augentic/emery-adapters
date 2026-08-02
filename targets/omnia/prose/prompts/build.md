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
$SLICE_NAME    = active in-progress plan entry's slice name (from `emery plan advance`)
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

The preparation leg ([`build/prepare.md`](build/prepare.md)) produces the read-only exemplar checkout at `target/omnia-exemplar/`. The compatibility contract — create-mode pin adoption, update-mode pin preservation, consumer-evidenced idioms winning conflicts — lives in [`exemplar.md`](../references/exemplar.md); writer prompts link into its navigation map rather than repeating any of it.

## Leg map

Between preparation and generation, the adapter runs its deterministic scaffold prelude in-guest: it validates the checkout's template contract, then writes any missing standard tooling file from the checkout (existing files are never overwritten; a missing or malformed checkout fails the build before generation). The generation user prompt carries the outcome as a `### scaffold prelude` block; never re-author the files it lists ([`configuration.md`](../references/configuration.md) describes them).

The adapter core drives the legs in a fixed order — preparation ([`build/prepare.md`](build/prepare.md)), generation (crate writer, test writer, guest writer in create mode, then the § verify-repair loop), capture replay ([`build/replay.md`](build/replay.md), dispatched only when the build context binds a `captures` source — skipped in-guest otherwise), then standards review ([`build/review.md`](build/review.md)), which closes the build: its answer carries the findings synthesis and output declaration the adapter assembles the build report from in-guest (see the review prompt's `## Build report`) — there is no separate report leg. Within the generation leg, write the crate before the tests, mark `tasks.md` checkboxes complete as each task lands, and never transition the slice lifecycle — the deterministic in-guest report gate checks the assembled report and the engine guest owns the `Refined → Built` transition.

## § Verify-repair loop (max 3 iterations)

Run after both crate writer and test writer have completed. Each iteration runs the four checks below; if any fail, classify the failure, apply the targeted fix, and start a new iteration.

```bash
cd $CRATE_PATH && cargo fmt --check
cd $CRATE_PATH && cargo check
cd $CRATE_PATH && cargo clippy --all-targets -- -D warnings
cd $CRATE_PATH && cargo test
```

If `cargo fmt --check` fails, run `cargo fmt` once. Formatting is mechanical; one pass suffices.

If `cargo check` or `cargo clippy` fails, re-enter [`build/crate.md`](build/crate.md) with the error output as context. Apply minimum-change repair discipline (see [`repair-patterns.md`](../references/repair-patterns.md)).

If `cargo test` fails, classify each failure: errors in `tests/` paths or `MockProvider` are test issues (re-enter [`build/test.md`](build/test.md)); errors in `src/` paths are code issues (re-enter [`build/crate.md`](build/crate.md)); manifest / workspace errors are fixed in `Cargo.toml` directly. The full classification table and the update-mode regression check (baseline capture, regression-vs-expected routing) live in [`repair-patterns.md`](../references/repair-patterns.md) — fetch it via MCP on the first test failure.

**Repair discipline.** Minimum change only — fix the reported error and nothing else. Scope the diff to files and functions named in the error output. Group failures by classification and re-enter each writer prompt once with all same-class errors. Full repair recipes: [`repair-patterns.md`](../references/repair-patterns.md).

Repeat until all four checks pass or 3 iterations exhausted. If still failing after 3 iterations: **STOP**. Surface the stop hint below with full error output and do not transition the slice — the standards-review leg maps the remaining failures as blocking findings, so the assembled build report is `status: failure` and parks the slice for human review.

## § Stop hint contract

A build failure surfaces a stop hint as the body's final output — a single structured message the parent skill or the parent loop can act on without re-deriving context:

- `slice` — slice name from `emery plan advance`.
- `phase` — `build`.
- `failing-task` — the `tasks.md` checkbox (or sub-step) that exited non-zero.
- `log-path` — absolute path to the captured stdout/stderr.
- `next-action` — typically `re-run /emery:build $SLICE after fix`.

Render the hint as the final visible output of the run, alongside the blocking findings that make the assembled build report `status: failure` (see the review prompt's `## Build report`). Never write the lifecycle yourself — the deterministic in-guest report gate checks the assembled report and the engine guest owns the lifecycle, so the slice stays `refined` and the loop (or a re-invocation) re-enters cleanly.

The standards-review surface and the build-report contract (in-guest assembly, derived status, findings shape) live with the leg that owns them: [`build/review.md`](build/review.md).

## References

Every reference below (and the rest of the corpus) is fetchable via the granted MCP references server; fetch on need rather than front-loading.

- [`guidance.md`](guidance.md), [`merge.md`](merge.md) — sibling prompts; [`build/prepare.md`](build/prepare.md), [`build/crate.md`](build/crate.md), [`build/test.md`](build/test.md), [`build/guest.md`](build/guest.md), [`build/replay.md`](build/replay.md), [`build/review.md`](build/review.md) — per-leg prompts.
- [`exemplar.md`](../references/exemplar.md) — the exemplar checkout contract and navigation map.
- [`hard-rules.md`](../references/hard-rules.md), [`guardrails.md`](../references/guardrails.md), [`wasm-constraints.md`](../references/wasm-constraints.md) — authority hierarchy, forbidden crates / APIs, WASM idioms.
- [`repair-patterns.md`](../references/repair-patterns.md) — repair recipes, test-failure classification, update-mode regression check.
- The remaining depth (capabilities, SDK / workspace / guest templates, update strategies, test depth, review depth) is catalogued in [`../references/README.md`](../references/README.md).
