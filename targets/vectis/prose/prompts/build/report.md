# Vectis build — phase report

Inlined by the adapter core into the report leg's system prompt (alongside [../build.md](../build.md)) after the write legs. Owns the build phase-report answer: the generation phase's own typed report. The **engine** assembles the terminal build report deterministically from the whole build loop — never write `build/report.yaml` or any report file, and never anticipate the verify / repair / review operations' results here.

## Task bookkeeping

Before answering, mark the `tasks.md` checkboxes the completed write legs earned at `STAGE_DIR/tasks.md` (the writable artifact stage — never the authoritative slice tree). Mark only phases that actually landed; a failed or skipped leg's boxes stay unchecked.

## § Phase outcome contract

> See [Phase outcome contract](../../references/emery-runtime/phase-outcome-contract.md).

The build leg answers with exactly one phase report:

- **completed** — the write legs ran; `outputs[]` and `ui-surface` are populated as below. A write leg that failed or was left incomplete stays `outcome: completed` with a blocking (`critical` / `important`) finding describing what failed — the engine, not this leg, decides what happens next. A missing host prerequisite (compatible JDK, Android SDK, Rust Android targets, `boltffi`, Xcode CLT, `xcodegen`) or template / pin drift needing operator judgement is likewise a blocking finding naming the prerequisite or drift signal.
- **not-applicable** — the slice introduces no work for this target at all (rare; a core-only slice is still `completed`).

## Phase report

Answer the report leg with a schema-valid phase report (the schema-gated answer — no report file is written). This report never transitions the slice: the engine folds it with the verify / repair / review reports into the terminal record and owns the `Refined → Built` transition.

```yaml
outcome: completed        # or: not-applicable
source: model-assisted    # the adapter core stamps `hybrid` when in-guest validation contributed
findings: []              # structured diagnostics; default []
ui-surface:               # optional; this slice's UI-surface signal (see below)
  screens: 3
outputs:                  # per-platform tree paths this build produced or maintained; default []
  - platform: core
    path: shared/
  - platform: ios
    path: iOS/
  - platform: android
    path: Android/
written: []               # notable stage/workspace writes (composition, bindings, tasks)
```

The optional `ui-surface: { screens: <N> }` field carries this slice's UI-surface signal: `<N>` is the count of screen-bearing requirements this slice introduces or modifies, taken from the build's own `spec.md` screen-identification judgement (the same walk [composition.md](composition.md) Step 1 performs) — **never** from `## Platforms`, which is an app-level constant stamped verbatim to every slice and never narrows per slice. `screens: 0` means "no UI surface" (the composition skip case). The adapter core compares this authored signal against the produced `composition.yaml` and attaches non-blocking coherence warnings (`composition-unexpected-for-non-ui-slice` when `screens: 0` yet a non-empty composition was produced; `composition-empty-for-ui-slice` when `screens > 0` yet the composition is empty or absent). Omitting the field disables those warnings; set it on every slice so the self-consistency check is live.

The `outputs[]` array declares the per-platform trees this build produced or maintained, each `path` relative to `PROJECT_DIR` (e.g. `shared/`, `iOS/`, `Android/`). Populate an entry for each supported platform in `project.yaml.platforms` the build wrote work for; omit platforms with no on-disk interpretation (`web`, `desktop`). Do not declare built binaries (the debug APK) — producing them is the `verify` operation's pass.

**Findings rule.** A clean generation pass carries an empty `findings[]` or only non-blocking findings (`suggestion` / `optional`). Any write leg that failed, was left incomplete, or hit an unresolved prerequisite contributes a blocking finding (`critical` / `important`) with the load-bearing error line as snippet evidence, `impact`, and `remediation`. The in-guest composition validator's deterministic findings are attached by the adapter core after your answer and ride the same report.
