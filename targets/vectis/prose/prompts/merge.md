# Vectis target — `merge`

The adapter core inlines this document into the system prompt of the merge leg when the slice being merged declares `target: vectis`. The engine dispatches the adapter's merge operation twice around its deterministic core merge — `preflight` before the engine folds the slice's deltas into the baseline, `postflight` after the commit and archive. Deterministic delta promotion (spec deltas, the slice's `composition.yaml`, operator-curated `tokens.yaml` / `assets.yaml` updates), baseline coherence validation, the lifecycle transition, and the archive move all stay with the engine; this prompt owns only the two Vectis-specific gates around that commit.

Two things make the Vectis `merge` gates different from the bare slice merge:

1. **`composition.yaml` is a build output that lands at merge time.** It is not a Specify artifact authored under `.specify/specs/`; the `build` prompt regenerates it from `spec.md` + `design.md`, and the engine's deterministic merge promotes it into the baseline alongside the spec deltas. The preflight and postflight composition validators are the gate.
2. **The cap matrix is re-verified against the merged baseline.** A green slice build is necessary but not sufficient — the postflight leg re-runs `cargo` / `make build` / `gradlew` against the merged tree because cross-slice regressions (UniFFI bridging drift, Java 21 / Gradle wrapper changes, cargo-swift drift, cap-marker expansion) only surface after deltas land.

## Preflight — staged composition validation

The preflight dispatch is fully deterministic: the adapter runs its in-guest composition validator against the staged slice contents (`${SLICE_DIR}/composition.yaml`, with auto-invoked `tokens` / `assets` modes against any sibling manifests) and answers without a judgment leg. Errors are blocking — the report is `status: failure`, and the engine aborts the merge with the slice still at `built`. When the slice is core-only (no `composition.yaml` in `${SLICE_DIR}`), the validator exits cleanly.

## Merge surface (engine-owned)

The merge surface is broader than spec / design / task deltas. In addition to the markdown deltas, the engine's deterministic merge promotes:

- `composition.yaml` from the slice — lands as the baseline UI input set for downstream shell generations (`.specify/specs/composition.yaml`).
- `tokens.yaml`, `assets.yaml`, and any referenced asset files under `design-system/assets/**` (or slice-local `assets/`) when the slice carried operator-curated updates to those manifests.

Neither merge gate performs this promotion, resolves baseline conflicts, transitions the lifecycle, or moves the slice into the archive — the engine owns all of it.

## Postflight — host cap-matrix re-verification

The postflight dispatch runs after the engine's commit: the slice's deltas are promoted into the baseline, the slice is `merged`, and its directory is archived. Verify the now-updated project root with host commands that match the assemblies present in the merged tree:

```bash
# core, when ${PROJECT_DIR}/shared exists
cd "$PROJECT_DIR" && cargo fmt --check
cd "$PROJECT_DIR" && RUSTFLAGS="-D warnings" cargo check
cd "$PROJECT_DIR" && cargo clippy --all-targets -- -D warnings
cd "$PROJECT_DIR" && RUSTFLAGS="-D warnings" cargo test

# iOS, when ${PROJECT_DIR}/iOS exists (scaffold files are adapter-synced at build time)
cd "$PROJECT_DIR/iOS" && make build
cd "$PROJECT_DIR/iOS" && make sim-build

# Android, when ${PROJECT_DIR}/Android exists
rustup target list --installed | grep android
cd "$PROJECT_DIR/Android" && make verify
```

Record every host step in a structured list with these fields:

- `name` — stable step id (`core.cargo-check`, `ios.make-build`, `android.gradlew-assembleDebug`, `android.preflight-java21`).
- `passed` — boolean.
- `failure_snippet` — empty when passed; otherwise the first useful stderr / stdout lines.

Host prerequisite failures (missing `cargo`, `gradle`, `xcodebuild`, Java 21, Android SDK / NDK, `cargo-swift`, Rust Android targets, an unusable Gradle wrapper) are host verification failures, not WASI tool failures. Name them as prerequisite steps (`android.preflight-java21`) so the report makes the boundary clear.

When the slice modified neither the core nor a shell (e.g. a docs-only or UI-input-only slice that touched no Crux code), still run the applicable host checks against the merged tree — the cap matrix as a whole must remain green.

After the leg answers, the adapter re-runs its deterministic composition validator in-guest against the merged baseline (`.specify/specs/composition.yaml`, with auto-invoked `tokens` / `assets` modes), with one bounded repair leg. It runs even when the current slice generated no platform code, because later shell work will consume the merged baseline input set. Residual validation findings force `status: failure`; warnings flow into the operator-facing summary; clean runs are silent.

### Why postflight, not preflight

The postflight gate intentionally validates the merged baseline, not the staged delta. Shell verification (UniFFI bridging, Java 21 / Gradle wrapper, cargo-swift, cap-marker expansion) is only meaningful once the spec-level deltas are promoted and the writers have a stable baseline to build against. The build already verified the slice in isolation; this gate catches cross-slice regressions.

## Failure semantics

A blocking preflight finding aborts the merge before anything folds: the slice stays `built`, the plan entry stays `in-progress`, and the operator resolves and re-runs the merge. A blocking postflight finding is a terminal diagnostic, not a park: the engine has already committed and archived the merge, so the report surfaces the regression for a follow-up repair slice — never attempt to roll back the merge or edit the baseline's lifecycle state from this prompt.

For cap-matrix failures that look like version-pin drift (AGP / Gradle / uniffi mismatch surfaced after pins changed in this slice), record the failure in the report findings and surface it — **agents exit** without editing specify-adapters (see [Consumer tooling boundary](../references/spec-runtime/guardrails.md#consumer-tooling-boundary)). The operator decides whether the next step is a pin fix in specify-adapters ([build.md](build.md) § Template / version-pin drift handling), a pin rollback, or a follow-up slice.
