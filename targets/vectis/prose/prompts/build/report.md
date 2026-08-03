# Vectis build — report

Inlined by the adapter core into the report leg's system prompt (alongside [../build.md](../build.md)) after the final-core-verify leg, and into the deterministic report gate's bounded repair leg. Owns the shell verify gate contract, the report answer, the phase outcome contract, and the build-report shape.

## Shell verify gate (Phase 8)

The adapter runs the deterministic shell verify in-guest at the report leg (its findings ride in the prompt) and re-runs it in the deterministic report gate after the answer lands. A missing or empty tree for any supported declared platform (`core`, `ios`, `android`) forces `status: failure`. When the core tree is present, the gate also requires a fresh `shared/.vectis/verify.ok` digest stamp from the final-core-verify leg (`core-verify-stamp-missing` / `core-verify-stamp-stale`). For `android`, the gate also checks the Gradle wrapper, `local.properties`, and the debug APK at `Android/app/build/outputs/apk/debug/app-debug.apk` — compile assurance requires the android write verify loop to have run `make build` first and written `Android/.vectis/verify.ok`; this gate is necessary but not sufficient on its own. For `ios`, the gate expects `iOS/.vectis/verify.ok` after a clean `make build`. `web` and `desktop` are valid tokens but have no on-disk interpretation yet — the gate emits a `platform-not-yet-supported` info finding and treats them as present.

## Report answer (Phase 9)

Mark `tasks.md` checkboxes complete as each phase lands, then answer the build's report leg with the build report (§ Build report). This prompt never transitions the slice — the deterministic in-guest report gate checks the answer and the engine guest owns the `Refined → Built` transition.

## § Phase outcome contract

> See [Phase outcome contract](../../references/emery-runtime/phase-outcome-contract.md).

The `build` phase concludes with exactly one of `success` / `failure` / `deferred`:

- **success** — every in-scope verify-repair loop returned `success` within its iteration budget, the shell verify gate (step 8) passed, the orchestrator has both regenerated `composition.yaml` (or skipped it for a core-only slice) and the implementation code under `${PROJECT_DIR}`, and `outputs[]` is populated with each supported platform's artifact path (debug APK for `android`). Write a `status: success` build report (§ Build report); the engine guest owns the lifecycle transition.
- **failure** — any verify-repair loop exhausted its iterations, the composition validation gate ([composition.md](composition.md)) failed and could not be repaired, or the core review prompt's `## § Consolidate review findings` left unresolved blocking (`critical` / `important`) findings. Surface the load-bearing error line as `--summary` and the full output through `--context`, and write a `status: failure` build report with the blocking findings mapped where possible; the merge prompt refuses to run while the slice is in this state.
- **deferred** — a host prerequisite is missing (compatible JDK, Android SDK, Rust Android targets, `boltffi`, Gradle wrapper, Xcode CLT, `xcodegen`) or a template / pin drift issue surfaced and operator judgement is required. Surface the unresolved prerequisite or drift signal as `--summary` and write a `status: failure` build report (the report carries only `success` / `failure`; `deferred` is the operator-facing stop signal, not a built slice).

## Build report

When the algorithm resolves, return a schema-valid build report as the answer to the build's report leg (the schema-gated report answer — no report file is written). This is the build's final deliverable. This prompt never transitions the slice lifecycle — the deterministic in-guest report gate checks the answer's coherence against the working tree and the engine guest owns the `Refined → Built` transition.

```yaml
version: 1
slice: <slice-name>     # matches the build request's `slice`
target: vectis@1.0.2       # this adapter at its manifest version
status: success         # or: failure
findings: []            # structured diagnostics; default []
ui-surface:             # optional; this slice's UI-surface signal (see below)
  screens: 3
outputs:                # per-platform build outputs; default []
  - platform: core
    path: shared/
  - platform: ios
    path: iOS/
  - platform: android
    path: Android/app/build/outputs/apk/debug/app-debug.apk
```

The optional `ui-surface: { screens: <N> }` field carries this slice's UI-surface signal: `<N>` is the count of screen-bearing requirements this slice introduces or modifies, taken from the build's own `spec.md` screen-identification judgement (the same walk [composition.md](composition.md) Step 1 performs) — **never** from `## Platforms`, which is an app-level constant stamped verbatim to every slice and never narrows per slice. `screens: 0` means "no UI surface" (the composition skip case). The deterministic report gate compares this authored signal against the produced `composition.yaml` and surfaces non-blocking coherence warnings (`composition-unexpected-for-non-ui-slice` when `screens: 0` yet a non-empty composition was produced; `composition-empty-for-ui-slice` when `screens > 0` yet the composition is empty or absent). Omitting the field disables those warnings; set it on every slice so the self-consistency check is live.

The `outputs[]` array declares the per-platform build outputs produced by this build. Each entry carries a `platform` token and a `path` relative to `PROJECT_DIR`. The deterministic in-guest report gate verifies every declared path exists in the working tree; a missing output surfaces as a blocking gate finding that fails the report. Populate `outputs[]` with an entry for each supported platform in `project.yaml.platforms` that the build produced or maintained work for. For `android`, declare the debug APK path produced by `make build`, not merely the `Android/` tree. Omit entries for platforms with no on-disk interpretation (`web`, `desktop`).

**Success vs failure findings rule.** A `status: success` report carries an empty `findings[]` or only non-blocking findings (`suggestion` / `optional`); the deterministic report gate downgrades a `success` report carrying any blocking (`critical` / `important`) finding to `failure`. A `status: failure` report populates `findings[]` with the blocking violations the target can map from the composition validator gate, the per-platform verify-repair output, and unresolved blocking review findings, and leaves `findings: []` when no specifics are mappable.

- **Clean build** — composition regenerated and the validator gate ([composition.md](composition.md)) passed (or was skipped for a core-only slice), every in-scope verify-repair loop (mid-build core, iOS, Android, and the post-review final-core-verify) returned `success` within its budget, the final core pass wrote a matching `shared/.vectis/verify.ok` digest when the core tree is present, clippy / check / test ran under `-D warnings` per [test.md](test.md), the in-guest shell verify gate passed (DX/BoltFFI patterns, verify stamps, zero new inline lint suppressions — [`VECTIS-009`](../../rules/VECTIS-009-lint-suppression-forbidden.md)), and the core review prompt's `## § Consolidate review findings` produced no blocking findings → `status: success`, `findings: []` (or only advisory `suggestion` / `optional` findings), `outputs[]` populated with each supported platform's artifact path.
- **Unresolved build** — a verify-repair loop exhausted its iterations, the composition validator gate failed unrepaired, unresolved blocking review findings remain after consolidation, or a host prerequisite / template / pin drift signal forced a `deferred` outcome → `status: failure` with blocking findings mapped where possible.

Each `findings[]` item validates against `schemas/diagnostics/diagnostic.schema.json` (the structured-diagnostic shape distributed with the CLI; required fields include `id`, `title`, `severity`, `source`, `artifact`, `evidence`, `impact`, `remediation`, `fingerprint`). Map vectis's composition-validator, cargo / Gradle / Xcode verify, and review findings into that shape, carrying detail under `evidence.kind: structured` with `target-adapter: vectis`.
