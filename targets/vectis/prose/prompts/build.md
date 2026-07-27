# Vectis target — build prompt

The adapter core inlines this document into the system prompt of every build leg for a `target: vectis` slice; the core — not this document — sequences the legs. The build produces a buildable cross-platform application (Crux shared core + per-platform shells) from the slice's already-synthesised `spec.md` and `design.md`. This document pins down three responsibilities in one place:

1. **`composition.yaml` regeneration.** Synthesis does not write `composition.yaml`. The build regenerates it from `spec.md` + `design.md` (which already carry every upstream spatial / structural claim synthesis folded in from source adapters) at the start of each build, alongside the code it accompanies. `merge` lands the regenerated file together with the implementation code.
2. **Phase prompts.** Each leg's in-leg instruction lives in a phase prompt under [`build/`](build/); the adapter core assembles each leg's system prompt from this document plus the leg's phase prompt.
3. **Design-system inputs.** `tokens.yaml` and `assets.yaml` are operator-curated and consumed as read-only build inputs; this prompt never invents or restates their contents. The component catalog (`.emery/design-system/components.yaml`) is the third design-system input, joining `tokens.yaml` and `assets.yaml`, but it is **agent-inferred and operator-reviewable**, not operator-curated: the workflow's deterministic bind bookkeeping writes it from the Step 0.5 bindings file (recording the names the build's Step 0.5 or operator parts supply), and the build reads the confirmed entries back during composition regeneration to factor shared component code per in-scope shell tree. Operators review and may `reject` or rename entries. When absent, no component factoring occurs.

The Vectis target stays three-capability (`guidance` / `build` / `merge`) — there is **no** fourth `refine` slot. Composition regeneration is part of `build`.

## Inputs

The build runs against the build request the CLI prepared at `.emery/slices/<slice>/build/request.yaml`; consume its `inputs` manifest rather than relying on convention. Every artifact path resolves against `inputs.root` (the slice tree).

- `inputs.artifacts.proposal` (`proposal.md`) — `## Platforms` scope (`core` / `ios` / `android`) and screen / interaction intent.
- `inputs.artifacts.specs[]` (`specs/<domain>/spec.md`) — behavioural requirements per domain: screen titles, scenarios, platform-specific behaviour, validation rules.
- `inputs.artifacts.design` (`design.md`) — domain model: ViewModel / `Event` / `Route` variants, per-page view structs, capability matrix.
- `inputs.artifacts.tasks` (`tasks.md`) — phase-completion tracking.
- `inputs.artifacts.additional[]` — the three design-system inputs the adapter's `metadata` record declares, **all optional** (`required: false`), each with an explicit absent-fallback:
  - `tokens.yaml` — design tokens; absent → HIG (iOS) / Material 3 (Android) theme fallback in the shell writers.
  - `assets.yaml` — asset inventory; the composition validator's `tokens` / `assets` modes run only when the respective file is present.
  - `components.yaml` — the agent-inferred component catalog (surfaced as `CATALOG_PATH`); written by the workflow's deterministic bind bookkeeping from the Step 0.5 bindings file and read back during composition regeneration; absent → no component factoring.

## Consumer posture

- Agents executing this prompt in a consumer project are **consumers**, not adapter maintainers.
- On template / verify / finalize / toolchain failure: **stop** with `deferred` or a failure report — see [Consumer tooling boundary](../references/emery-runtime/guardrails.md#consumer-tooling-boundary).
- **Never** edit `emery-adapters`, `vectis-template`, or the built guest component in-band — even when those repos are sibling checkouts.
- Pin and DX drift is fixed by re-copying from `$TEMPLATE_DIR` (or fixing the template repo in a maintainer session) — never by inventing versions in the consumer tree.

## Standard arguments

All phase prompts assume these symbols are resolved by the leg's orchestrating agent before any sub-agent fan-out:

| Symbol | Meaning |
| --- | --- |
| `SLICE_ID` | The active slice name (`emery plan next` output, or `emery slice` argument). |
| `SLICE_DIR` | `.emery/slices/<SLICE_ID>/`. |
| `DOMAIN_NAME` | The single domain spec folder under `SLICE_DIR/specs/`. When the slice carries multiple domains, iterate the per-domain phase prompts in declaration order. |
| `PROJECT_DIR` | The target project root (single-repo mode) or the resolved workspace slot (workspace mode). |
| `TEMPLATE_DIR` | Local [`vectis-template`](https://github.com/augentic/vectis-template) checkout. Default `${PROJECT_DIR}/../vectis-template`; override with `VECTIS_TEMPLATE_DIR`. Required for greenfield materialize and pin refresh. |
| `IOS_SHELL_DIR` | `${PROJECT_DIR}/iOS` (only when `ios` is in scope). |
| `ANDROID_SHELL_DIR` | `${PROJECT_DIR}/Android` (only when `android` is in scope). |
| `APP_NAME` | The Xcode target / Swift source folder name (derived from `design.md`'s `App` struct name). |
| `ANDROID_PACKAGE` | Android application id. Prefer the package declared in `design.md` (or the existing `Android/app/build.gradle.kts` applicationId). Fallback only: `com.vectis.<lowercase APP_NAME>` — do not keep the template's `io.augentic.vectisapp` unless that is the product id. Writers, reviewers, and imports MUST use this resolved value (dot form + slash form under `app/src/main/java/`) — never hardcode `com.vectis.*` or the template default when `ANDROID_PACKAGE` differs. |
| `CATALOG_PATH` | `${PROJECT_DIR}/.emery/design-system/components.yaml` when present. Optional — absent means no component factoring. |

## Platform scope

Every slice carries the full app platform set from `project.yaml.platforms` (stamped verbatim into `proposal.md ## Platforms` by synthesis). Each slice signifies core + all declared shell work; build determines the **actual per-platform work**:

- **create** — a declared tree is absent on disk → the agent materializes from `$TEMPLATE_DIR` on the **host** filesystem before write legs (the guest cannot see a sibling checkout). Follow § Template materialize below; the template-materialize prelude in each leg names which trees are absent. There is no separate plan-time bootstrap slice; `project.yaml.platforms` already declares the intent. Only `core`, `ios`, and `android` are materialized today (`web/` in the template is out of scope). Fail closed when `$TEMPLATE_DIR` is missing — do not invent a scaffold.
- **update** — the shell tree exists → diff core types against existing code and apply targeted edits (the normal feature-slice path).
- **no-op** — the platform is in scope but the slice introduces no changes for that shell (answer the leg with `applicable: false`).

Valid Vectis platform tokens are `core`, `ios`, `android`, `web`, and `desktop`. Only `core`, `ios`, and `android` have build prompts today; the adapter core silently skips `web` and `desktop` in the platform set (no shell leg to run). Token / asset / layout work is **input context**, never a platform.

## § Template materialize (greenfield)

This is **template materialize** (`vectis::scaffold::materialize`) — not asset materialize (`vectis::materialize` / the prepare prelude that exports design-system assets).

1. **Resolve `$TEMPLATE_DIR`.** Default `${PROJECT_DIR}/../vectis-template`, else `VECTIS_TEMPLATE_DIR`. If the directory is missing or is not a `vectis-template` checkout, **stop** (`deferred`) — clone https://github.com/augentic/vectis-template.git; never invent scaffold files or pins.
2. **Mechanical allowlisted copy** into `${PROJECT_DIR}` with identity substitution (`APP_NAME`, `ANDROID_PACKAGE`). Copy root DX (`Makefile`, `Makefile.toml`, `Cargo.toml`, `Cargo.lock` when present, `rust-toolchain.toml`, `deny.toml`, `README.md`, `.gitignore`), plus `shared/`, `iOS/`, `Android/` (including the Gradle wrapper), `supply-chain/`, and `.maestro/`. **Never** copy `.git/`, `.github/`, `web/`, or `AGENTS.md`. Skip machine junk (`target/`, `.gradle/`, `*.xcodeproj/`, `local.properties`, …) per the `scaffold::materialize` denylist. One materialize stands up the whole workspace — do not invent per-shell scaffolds.
3. **Strip `VECTIS-OPTIONAL`.** Follow **`$TEMPLATE_DIR/AGENTS.md`** (not a consumer copy) against the `design.md` `## Adapters` capability matrix: remove unused `cap=http|kv|time|sse` units and always strip `cap=demo` for product apps. Do not invent FFI shapes or dependency versions while stripping. Keep Maestro infra (`.maestro/config.yaml`, `.maestro/test-ids.yaml`, `.maestro/scripts/load-test-ids.sh`) and root / shell DX after strip — see [`template-capabilities.md`](../references/template-capabilities.md).
4. **iOS project generation.** After materialize, run `make -C iOS generate-project` (or `xcodegen`) — committed `.xcodeproj` trees are denylisted on purpose.
5. **Then** run the existing generate/update logic in the core / shell write prompts.

**Late capability adoption (update mode).** When a later slice's `## Adapters` turns a previously stripped cap on, copy that `cap=` strip-unit from `$TEMPLATE_DIR` per [`template-capabilities.md`](../references/template-capabilities.md) — do not invent versions or handler shapes. Strip grammar remains `$TEMPLATE_DIR/AGENTS.md`.

The adapter core processes platforms in dependency order: `core` first (the shells depend on it), then the declared `ios` / `android` shell legs — independent of each other, but run serially because their verify halves share the same Cargo workspace lock. When the platform set contains `core` only, the core skips the shell legs wholesale; this is a backend-only build.

## Phase order

Leg order is owned by the adapter core, not by this document: the core runs its deterministic prepare prelude, then the **composition** leg (Step 0.5 + Phase 1, gated by the in-guest composition validator with a bounded repair loop), the **core** leg (Phases 2–3), one **shell** leg per declared shell platform (Phases 4–5), the **review** leg (Phases 6–7, ending with `## § Consolidate review findings`), and finally the **report** leg (Phases 8–9), bracketed by the deterministic postlude gates. Each leg's system prompt carries this document plus the leg's phase prompt: [`build/composition.md`](build/composition.md) (Phase 1), [`build/core/write.md`](build/core/write.md) + [`build/test.md`](build/test.md) (Phases 2–3), [`build/ios/write.md`](build/ios/write.md) / [`build/android/write.md`](build/android/write.md) (Phases 4–5, when in scope), and [`build/core/review.md`](build/core/review.md) plus the in-scope shell review prompts (Phases 6–7). The remainder of this section carries only the in-leg instruction that crosses phase-prompt boundaries: Step 0.5, the shell verify gate, and the report answer.

**Step 0.5 — component inference (runs in the composition leg, ahead of composition regeneration).** Component *identity* is deterministic and owned by the adapter's in-guest clustering engine (a structural fingerprint over each `group`'s normalized skeleton); component *identification and naming* are model judgement and owned by this prompt. The engine carries **no** component vocabulary — it reports identity + evidence, and the workflow's deterministic bind bookkeeping records the names it is handed; this prompt decides what each clustered structure *is* and what to call it. Inference runs before composition regeneration so the regeneration at [`build/composition.md`](build/composition.md) step 6 reads an up-to-date component set. **Timing.** The report clusters against the **merged** baseline plus the candidate cache and `parts.yaml` — not the current slice's composition, which has not merged yet. With one screen per slice and the default occurrence threshold of 2, a baseline-only path surfaces a repeated structure at the **third** slice's build (once two prior screens have merged); the screenshots candidate cache (RFC §B4) can supply the second occurrence **during** the second slice's build when stage-6 sidecars exist. B7 retroactive factoring runs on whichever build first binds the component.

1. **Report.** The adapter runs the deterministic, **name-free** clustering in-guest against the current merged baseline (`${PROJECT_DIR}/.emery/specs/composition.yaml`) and injects the cluster report into the composition leg's prompt — do not attempt to re-run it. The clustering folds the screenshots candidate cache and, when present, the operator-authored `parts.yaml` (`${PROJECT_DIR}/.emery/design-system/parts.yaml`) into the same pass automatically. A `parts.yaml` part is a third authoritative input that carries two authorities the clustering honours silently: **naming** (its operator slug wins, so the matching cluster arrives with `bound-slug` already populated — leave it untouched in step 2) and **promotion** (a part matching at least one baseline group is surfaced as a cluster even below the occurrence threshold). Parts that match no baseline group surface in the report's non-blocking `unmatched-parts` list (informational); it never gates the build and is only authoritative over the complete baseline at change completion. An absent baseline yields an empty report (nothing to name). Each reported cluster carries a `fingerprint` (the opaque identity), an `occurrences` count, the `screens` provenance list, the representative normalized `skeleton`, an `evidence` block (`region`, `item-kinds`, `event-targets`, and an optional `candidate-names` list of stage-6 suggestions), and a `bound-slug` (the name already bound to that fingerprint, or `null`).
2. **Identify and name by judgement.** For each reported cluster whose `bound-slug` is `null`, decide *what the component is* and *what to call it*: read its `evidence` and representative `skeleton`, and choose a kebab-case slug. There is **no fixed component vocabulary** — a repeated footer of navigation icons might be a `tab-bar`, a `rail`, or a novel navigation form this app invents; name it on its merits rather than forcing it into a known label. The `evidence.candidate-names` suggestions (when present) are non-authoritative stage-6 hints you MAY adopt or override — never an identity. A cluster whose `bound-slug` is **already populated** is already named — from a prior run's catalog binding, or from an operator `parts.yaml` pin whose name wins — so leave it untouched.
3. **Bind.** Write your `{ fingerprint → slug }` decisions to the bindings file at `${SLICE_DIR}/build/component-bindings.yaml`; the workflow's deterministic bind bookkeeping records them into the catalog. The bindings file is a `bindings:` map keyed by each cluster's `fingerprint`, valued by the bare slug (or `{ slug, description }`):

```yaml
version: 1
bindings:
  <fingerprint-a>: tab-bar
  <fingerprint-b>:
    slug: detail-card
    description: "Repeated detail card across list rows."
```

   The bind bookkeeping applies its deterministic guards — one skeleton per slug, never overwrite a `confirmed` / `rejected` entry, and stable fingerprint-derived suffixing (`slug-<fp-prefix>`) on a name collision — and is the **only** writer of `components.yaml`; never edit the catalog directly. Skip this step when the report names no unbound clusters.
4. **Proceed.** Continue with composition regeneration (Phase 1): [`build/composition.md`](build/composition.md) step 6 treats your fresh bindings plus the existing catalog's confirmed entries as the effective component set and attaches `component: <slug>` directives to every group whose skeleton matches.

**Shell verify gate (Phase 8).** The adapter runs the deterministic shell verify in-guest at the report leg (its findings ride in the prompt) and re-runs it in the deterministic report gate after the answer lands. A missing or empty tree for any supported declared platform (`core`, `ios`, `android`) forces `status: failure`. For `android`, the gate also checks the Gradle wrapper, `local.properties`, and the debug APK at `Android/app/build/outputs/apk/debug/app-debug.apk` — compile assurance requires the android write verify loop to have run `make build` first and written `Android/.vectis/verify.ok`; this gate is necessary but not sufficient on its own. For `ios`, the gate expects `iOS/.vectis/verify.ok` after a clean `make build`. `web` and `desktop` are valid tokens but have no on-disk interpretation yet — the gate emits a `platform-not-yet-supported` info finding and treats them as present.

**Report answer (Phase 9).** Mark `tasks.md` checkboxes complete as each phase lands, then answer the build's report leg with the build report (§ Build report). This prompt never transitions the slice — the deterministic in-guest report gate checks the answer and the engine guest owns the `Refined → Built` transition.

## § Sub-agent delegation contract

Each writer / reviewer phase prompt runs in its **own sub-agent** with a clean context window. Core test verify-repair still runs inside its phase sub-agent. **iOS and Android shell verify are exceptions:** after each shell writer sub-agent returns, the orchestrator (not a verify sub-agent) runs that platform's verify shell commands; failures spawn platform repair sub-agents that may edit generated UI sources only. The leg's orchestrating agent coordinates the sequence and executes iOS and Android verify shell commands inline.

**Inputs (orchestrator → sub-agent):** `task` (one of `core-writer`, `test-writer`, `ios-writer`, `ios-verify-repair`, `android-writer`, `android-verify-repair`, `core-reviewer`, `ios-reviewer`, `android-reviewer`), `arguments` (standard arguments above), `mode` (`create`, `update`, or `repair` — decided by the orchestrator from on-disk inspection), `skip_verification` (true for shell writers; iOS and Android verify commands run from the orchestrator, not verify sub-agents), `artifact_paths` (paths to `spec.md`, `design.md`, `proposal.md`, regenerated `composition.yaml`, sibling `tokens.yaml` / `assets.yaml` when present, and `components.yaml` when `CATALOG_PATH` exists), `forbidden_paths` (`ios-verify-repair` only — `iOS/Makefile`, `iOS/project.yml`), `allowed_paths` (`ios-verify-repair` only — Swift under `iOS/<APP_NAME>/`, plus `Theme/`, `Components/`, `Resources/`; `android-verify-repair` only — Kotlin under `Android/app/src/main/java/`), `orchestrated` (reviewer sub-agents only; signals that the reviewer is running inside a build phase so its `design_findings` should flow into § Consolidate review findings — reviewers always return `design_findings` for the parent to consolidate, never auto-spawn follow-up slices), `extra_context` (phase-specific: `error_output` for repair sub-agents, baseline test log for regression checks, prior phase warnings).

**Outputs (sub-agent → orchestrator):** `status` (`success` / `failure` / `pending`), `files_modified`, `verification` (inline result when the sub-agent ran one), `errors`, `warnings`, `design_findings` (reviewers only; empty list when nothing surfaced).

### Why verify is serial; review is parallel

The iOS verify pipeline (`make build` → typegen + `boltffi pack apple` + xcodegen + simulator build) and the Android verify pipeline (`make build` → typegen + `boltffi pack android` + `:app:assembleDebug`) both invoke `cargo` against the same shared Rust workspace. Cargo uses a workspace-level lock file, so concurrent invocations serialise on the lock rather than running in parallel. The reviewers are pure code-analysis agent teams; they use different formatters (`swiftformat` vs Kotlin) and never invoke `cargo`, Gradle, or Xcode. With no shared mutable state and no build-tool contention, they are safe to run concurrently.

## § Consolidate review findings

When all in-scope reviews complete:

1. **Merge findings.** Combine `design_findings` from each reviewer into a single list. Deduplicate universal findings (UNI-prefixed) that both reviewers flagged with identical check IDs and matching evidence — keep the higher-severity instance. Platform-specific findings (CRX-, LOG-, GEN-, IOS-, SWF-, AND-, KTL-, INT-prefixed) are always distinct.
2. **Empty list.** Skip the rest of this section.
3. **Validate classifications.** Each finding already carries `code-fix` or `spec-change`. Treat that as the source of truth. Resolve disagreements between platforms by applying: spec is clear but code is wrong → `code-fix`; spec is silent, ambiguous, or problematic → `spec-change`.
4. **Surface findings.** Findings flow to the operator alongside the build outcome. Cross-platform follow-up work is queued as a new slice via the operator's normal `/emery:plan` flow rather than letting reviewers spawn slices directly.

## § Standards review surface

The per-platform reviewers above ([`build/core/review.md`](build/core/review.md), [`build/ios/review.md`](build/ios/review.md), [`build/android/review.md`](build/android/review.md)) carry the model-assisted surface — specialist + antagonist judgment per [`agent-teams.md`](../references/agent-teams.md), applying the engineering-standards rules shipped under [`../rules/`](../rules/) (the Vectis overlay plus the shared `UNI-*` pack at `rules/universal/`).

Vectis render-by-`kind` drift ([`VECTIS-006`](../rules/VECTIS-006-asset-render-by-kind.md)) is review-scoped in v1: iOS and Android Integration specialists run **IOS-020** / **AND-028** on the first full-scope iteration (see per-platform review prompts and team protocols).

Framework acceptance fixtures under `quality/fixtures/reference/targets/vectis/` version-control `design-system/assets/exports/` (see [`task-list/design-system/`](https://github.com/augentic/emery/tree/main/quality/fixtures/reference/targets/vectis/task-list/design-system)) so build prompt examples and eval pins demonstrate the materialize-then-copy hand-off without requiring image-processing deps in every CI job.

Per [Standards layer](../references/emery-runtime/standards-layer-snippet.md), standards findings may block CI but never transition plan entries, slices, or changes. CI wiring is consumer-project policy, not adapter policy; this prompt acknowledges the surface and links out for the contract.

## § Template / version-pin drift handling

Dependency pins live only as bytes in `$TEMPLATE_DIR`. There is no adapter-side version registry. Detect drift when a verify-repair loop fails repeatedly with cargo / Gradle / Xcode / BoltFFI errors that look like API renames, missing imports, or toolchain mismatches rather than feature-level bugs — or when consumer pin files diverge from the template counterparts after identity substitution.

**Pin-diff checklist (prompt-mandated; guest cannot see `$TEMPLATE_DIR`):** after materialize and on any pin-suspect failure, diff these consumer paths against the same relative paths under `$TEMPLATE_DIR`, allowing only identity substitution (`APP_NAME`, `ANDROID_PACKAGE` / package path forms):

- `Cargo.toml` (workspace deps)
- `Cargo.lock` (when present in both trees)
- `rust-toolchain.toml`, `deny.toml`
- `shared/Cargo.toml` (including the `boltffi = "…"` pin line)
- `shared/boltffi.toml` (structure + non-identity fields; package id may differ after substitution)
- `Android/gradle/libs.versions.toml`
- iOS / Android Makefiles and `iOS/project.yml` (BoltFFI pack recipes + `DESTINATION`)

**Agents:** detect → re-copy the drifted paths from `$TEMPLATE_DIR` with the same identity substitution as materialize → if the failure persists, mark the build `deferred` with a template / pin drift signal → **exit** (never invent versions). See [Consumer tooling boundary](../references/emery-runtime/guardrails.md#consumer-tooling-boundary).

**Operators (separate maintainer session):** fix pins in [`augentic/vectis-template`](https://github.com/augentic/vectis-template); consumers re-copy from the refreshed checkout. Do not patch version tables inside the Vectis adapter.

## § Phase outcome contract

> See [Phase outcome contract](../references/emery-runtime/phase-outcome-contract.md).

The `build` phase concludes with exactly one of `success` / `failure` / `deferred`:

- **success** — every in-scope verify-repair loop returned `success` within its iteration budget, the shell verify gate (step 8) passed, the orchestrator has both regenerated `composition.yaml` (or skipped it for a core-only slice) and the implementation code under `${PROJECT_DIR}`, and `outputs[]` is populated with each supported platform's artifact path (debug APK for `android`). Write a `status: success` build report (§ Build report); the engine guest owns the lifecycle transition.
- **failure** — any verify-repair loop exhausted its iterations, or the composition validation gate ([build/composition.md](build/composition.md)) failed and could not be repaired. Surface the load-bearing error line as `--summary` and the full output through `--context`, and write a `status: failure` build report with the blocking findings mapped where possible; the merge prompt refuses to run while the slice is in this state.
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

The optional `ui-surface: { screens: <N> }` field carries this slice's UI-surface signal: `<N>` is the count of screen-bearing requirements this slice introduces or modifies, taken from the build's own `spec.md` screen-identification judgement (the same walk `build/composition.md` Step 1 performs) — **never** from `## Platforms`, which is an app-level constant stamped verbatim to every slice and never narrows per slice. `screens: 0` means "no UI surface" (the composition skip case). The deterministic report gate compares this authored signal against the produced `composition.yaml` and surfaces non-blocking coherence warnings (`composition-unexpected-for-non-ui-slice` when `screens: 0` yet a non-empty composition was produced; `composition-empty-for-ui-slice` when `screens > 0` yet the composition is empty or absent). Omitting the field disables those warnings; set it on every slice so the self-consistency check is live.

The `outputs[]` array declares the per-platform build outputs produced by this build. Each entry carries a `platform` token and a `path` relative to `PROJECT_DIR`. The deterministic in-guest report gate verifies every declared path exists in the working tree; a missing output surfaces as a blocking gate finding that fails the report. Populate `outputs[]` with an entry for each supported platform in `project.yaml.platforms` that the build produced or maintained work for. For `android`, declare the debug APK path produced by `make build`, not merely the `Android/` tree. Omit entries for platforms with no on-disk interpretation (`web`, `desktop`).

**Success vs failure findings rule.** A `status: success` report carries an empty `findings[]` or only non-blocking findings (`suggestion` / `optional`); the deterministic report gate downgrades a `success` report carrying any blocking (`critical` / `important`) finding to `failure`. A `status: failure` report populates `findings[]` with the blocking violations the target can map from the composition validator gate and the per-platform verify-repair output, and leaves `findings: []` when no specifics are mappable.

- **Clean build** — composition regenerated and the validator gate ([build/composition.md](build/composition.md)) passed (or was skipped for a core-only slice), every in-scope verify-repair loop (core, iOS, Android) returned `success` within its budget, the core loop passed with zero compiler warnings (`RUSTFLAGS="-D warnings"` on `cargo check` / `cargo test` and `clippy -- -D warnings` per [build/test.md](build/test.md)), the in-guest shell verify gate passed (DX/BoltFFI patterns, verify stamps, zero new inline lint suppressions — [`VECTIS-009`](../rules/VECTIS-009-lint-suppression-forbidden.md)), and § Consolidate review findings produced no blocking findings → `status: success`, `findings: []` (or only advisory `suggestion` / `optional` findings), `outputs[]` populated with each supported platform's artifact path.
- **Unresolved build** — a verify-repair loop exhausted its iterations, the composition validator gate failed unrepaired, or a host prerequisite / template / pin drift signal forced a `deferred` outcome → `status: failure` with blocking findings mapped where possible.

Each `findings[]` item validates against `schemas/diagnostics/diagnostic.schema.json` (the structured-diagnostic shape distributed with the CLI; required fields include `id`, `title`, `severity`, `source`, `artifact`, `evidence`, `impact`, `remediation`, `fingerprint`). Map vectis's composition-validator, cargo / Gradle / Xcode verify, and review findings into that shape, carrying detail under `evidence.kind: structured` with `target-adapter: vectis`.

## Notes for downstream phases

- **`composition.yaml` is a build output.** It lives at `${SLICE_DIR}/composition.yaml` after the build succeeds; the merge prompt lands it into the baseline alongside the code. Operator-curated `tokens.yaml` / `assets.yaml` are also read by `merge`; the merge phase re-runs the adapter's deterministic composition validator against the merged baseline so cross-artifact regressions are caught even when the current slice only touched code.
- **Do not write `composition.yaml` into `.emery/specs/`.** That is `emery slice merge`'s job, atomically, alongside the spec / design deltas.
- **Operator-curated inputs.** `tokens.yaml` and `assets.yaml` updates accompany the slice when the operator edits them; the merge prompt promotes those edits into `design-system/tokens.yaml` / `design-system/assets.yaml` (or slice-local equivalents) using the same delta merge path as the spec deltas. The component catalog (`CATALOG_PATH`) is project-level and not slice-local; it is read as-is at build time and does not participate in the merge delta path.
