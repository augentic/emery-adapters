# Vectis target — `merge`

The adapter core inlines this document into the system prompt of the merge leg when the slice being merged declares `target: vectis`. The engine dispatches the adapter's merge operation twice around its deterministic core merge — `preflight` before the engine folds the slice's deltas into the baseline, `postflight` after the commit and archive. Deterministic delta promotion (spec deltas, the slice's `composition.yaml`, operator-curated `tokens.yaml` / `assets.yaml` updates), baseline coherence validation, the lifecycle transition, and the archive move all stay with the engine; this prompt owns only the two Vectis-specific gates around that commit.

Two things make the Vectis `merge` gates different from the bare slice merge:

1. **`composition.yaml` is a build output that lands at merge time.** It is not a Emery artifact authored under `.emery/specs/`; the `build` prompt regenerates it from `spec.md` + `design.md`, and the engine's deterministic merge promotes it into the baseline alongside the spec deltas. The preflight and postflight composition validators are the gate.
2. **The cap matrix is re-verified against the merged baseline.** Slice build already ran a final core verify-repair (and digest stamp) before report; postflight is a *second*, merged-baseline re-check for cross-slice / promotion regressions (BoltFFI bridging drift, Gradle wrapper changes, pin drift, cap-marker expansion) — not the primary build clippy gate.

## Preflight — staged composition validation

The preflight dispatch is fully deterministic: the adapter runs its in-guest composition validator against the staged slice contents (`${SLICE_DIR}/composition.yaml`, with auto-invoked `tokens` / `assets` modes against any sibling manifests) and answers without a judgment leg. Errors are blocking — the report is `status: failure`, and the engine aborts the merge with the slice still at `built`. When the slice is core-only (no `composition.yaml` in `${SLICE_DIR}`), the validator exits cleanly.

## Merge surface (engine-owned)

The merge surface is broader than spec / design / task deltas. In addition to the markdown deltas, the engine's deterministic merge promotes:

- `composition.yaml` from the slice — lands as the baseline UI input set for downstream shell generations (`.emery/specs/composition.yaml`). Writing it into `.emery/specs/` is this merge's job, atomically, alongside the spec / design deltas — never the build's.
- `tokens.yaml`, `assets.yaml`, and any referenced asset files under `design-system/assets/**` (or slice-local `assets/`) when the slice carried operator-curated updates to those manifests — promoted into `design-system/tokens.yaml` / `design-system/assets.yaml` (or slice-local equivalents) using the same delta merge path as the spec deltas.

The component catalog (`CATALOG_PATH`) is project-level and not slice-local; it is read as-is at build time and does not participate in the merge delta path. Neither merge gate performs the promotion above, resolves baseline conflicts, transitions the lifecycle, or moves the slice into the archive — the engine owns all of it.

## Postflight — host cap-matrix re-verification

The postflight dispatch runs after the engine's commit: the slice's deltas are promoted into the baseline, the slice is `merged`, and its directory is archived. Verify the now-updated project root with host commands scoped to the platforms declared in `.emery/project.yaml` (`platforms:`):

```bash
# core, when ${PROJECT_DIR}/shared exists
cd "$PROJECT_DIR" && cargo fmt --check
cd "$PROJECT_DIR" && RUSTFLAGS="-D warnings" cargo check
cd "$PROJECT_DIR" && cargo clippy --all-targets -- -D warnings
cd "$PROJECT_DIR" && RUSTFLAGS="-D warnings" cargo test

# iOS, when `ios` is in platforms:
cd "$PROJECT_DIR/iOS" && make build

# Android, when `android` is in platforms:
rustup target list --installed | grep android
cd "$PROJECT_DIR/Android" && make build
```

Record every host step in a structured list with these fields:

- `name` — stable step id (`core.cargo-check`, `ios.make-build`, `android.make-build`, `android.preflight-jdk`).
- `passed` — boolean.
- `failure_snippet` — empty when passed; otherwise the first useful stderr / stdout lines.

Host prerequisite failures (missing `cargo`, `gradle` / wrapper, `xcodebuild`, `xcodegen`, compatible JDK, Android SDK / NDK, `boltffi`, Rust Android targets) are host verification failures, not WASI tool failures. Name them as prerequisite steps (`android.preflight-jdk`) so the report makes the boundary clear.

When the slice modified neither the core nor a shell (e.g. a docs-only or UI-input-only slice that touched no Crux code), still run the applicable host checks against the merged tree — the cap matrix as a whole must remain green.

After the leg answers, the adapter re-runs its deterministic composition validator in-guest against the merged baseline (`.emery/specs/composition.yaml`, with auto-invoked `tokens` / `assets` modes), with one bounded repair leg. It runs even when the current slice generated no platform code, because later shell work will consume the merged baseline input set. Residual validation findings force `status: failure`; warnings flow into the operator-facing summary; clean runs are silent.

### Why postflight, not preflight

The postflight gate intentionally validates the merged baseline, not the staged delta. Shell verification (BoltFFI bridging, Gradle wrapper, pin faithfulness vs `$TEMPLATE_DIR`, cap-marker expansion) is only meaningful once the spec-level deltas are promoted and the writers have a stable baseline to build against. The build already verified the slice in isolation (including post-review final core clippy); this gate catches cross-slice / promotion regressions on the landed tree.

## Failure semantics

A blocking preflight finding aborts the merge before anything folds: the slice stays `built`, the plan entry stays `in-progress`, and the operator resolves and re-runs the merge. A blocking postflight finding is a terminal diagnostic, not a park: the engine has already committed and archived the merge (non-rollback), the plan entry is `done`, and the gate report lands at the archive's `merge/postflight.yaml` (including `status: failure`). Never attempt to roll back the merge, edit the baseline's lifecycle state, or retry the merge for that archived slice from this prompt.

Operator resume (engine-owned): inspect the archived `merge/postflight.yaml`, repair the unclean baseline (hand-fix or a follow-up slice via `/emery:plan`), then re-run `emery plan execute` to acknowledge the sticky `merge-postflight-failed` stop and continue (or finalize when the plan is otherwise drained).

For cap-matrix failures that look like version-pin drift (AGP / Gradle / BoltFFI mismatch surfaced after pins changed in this slice), record the failure in the report findings and surface it — **agents exit** without editing emery-adapters or inventing pins (see [Consumer tooling boundary](../references/emery-runtime/guardrails.md#consumer-tooling-boundary)). The operator decides whether the next step is a pin fix in [`vectis-exemplar`](https://github.com/augentic/vectis-exemplar) ([template-capabilities.md](../references/template-capabilities.md) § Template / version-pin drift handling), a pin rollback, or a follow-up slice.
