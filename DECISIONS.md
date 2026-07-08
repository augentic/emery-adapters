# Decisions

Standing architectural decisions for the Specify adapters repository. Read before changing the adapter guest / core split, the vectis validation or materialization engines, embedded asset homes, or the verification posture of a target's prompts.

Each entry records the decision, why it was taken, and the consequences a change must reckon with — not how the feature works today. Current behavior lives in module-level rustdoc and the per-target prompts; entries here point at those rather than restating them.

## Prose overlay dev loop

**Decision (2026-07).** The adapter author's prose iteration loop pays no compilation: the `adapter` crate has a dev-only `prose-overlay` cargo feature, off by default and compiled out of `cargo make release` and every published component. The overlay exists to iterate, never to certify; graded evidence comes from embedded builds only. Behavior lives in module rustdoc: `crates/adapter/src/registry.rs`, `evals/live.rs`, `evals/runtime.rs`.

- **The overlay overrides bodies, never the doc set.** Registry lookups consult `.eval/prose/<key>` before the embedded table, but only for paths the table declares.
- **The cargo-skip is stamp-guarded.** In overlay mode the runner skips cargo legs when artifacts exist and a SHA-256 stamp matches the adapter wasm from the last overlay-flagged build.
- **The eval guest drives one operation per invocation.** Its argv leads with an operation selector over the judgment-bearing seam operations.
- **Model ids stay out of guests.** `SPECIFY_EVAL_MODEL` is read by the eval driver alone.

The hands-off loop is the `eval-watch` cargo-make task: cargo-watch over one adapter's prose trees re-invoking a single scenario filter with the overlay active.

## One component, no manifest

**Decision (2026-07).** The deployable adapter artifact is exactly one wasm component: every fact a manifest once carried moves into the component or the registry reference.

- **`describe` operation.** Both axis interfaces expose `describe: func(id: adapter-id) -> manifest` — deterministic, effect-free, compiled-in constants. Source manifests carry an optional `specify-floor`; target manifests add `inputs[]` and an optional `platforms-capability`.
- **Identity in the guest crate.** Semver lives in each guest crate's `Cargo.toml` `version` — the single identity declaration and the `<semver>` the publish workflow reads via `cargo metadata` to push `specify:<name>@<semver>`.
- **No committed wasm.** Components are release-built to `target/wasm32-wasip2/release/<name>.wasm`, which is where the publish workflow pushes from. Runtime tests build guests from source into the cargo target directory.
- **wasm-pkg publish.** `.github/workflows/release.yaml` release-builds guests and pushes each component with `wkg publish` under the `specify` namespace on `augentic.io`.

## Prose tree rename — `briefs/` → `prompts/`

**Decision (2026-07).** Every adapter's `prose/briefs/` tree is `prose/prompts/`. These files are embedded prompt fragments: `build.rs` emits them into the guest, the core assembles them into judgment-leg system prompts, and sequencing lives in Rust.

## Vectis self-containment

**Decision (2026-07).** All vectis deterministic tooling lives in `vectis-core` as plain library code consumed by the guest. Former extension modules map to core modules (`infer`, `verify`, `scaffold`, `sync`, `android`, `schema_source`, `shell`, `prepare`). Unit tests live in `core/tests/` as integration tests over the public surface. Assets (`schemas/`, `templates/`, `assets/`, `versions.toml`) are core-owned.

## Prompt and prose cutover

**Decision (2026-07).** No compiled-in prompt instructs the agent to run retired CLI surfaces (`specify extension run`, `specify catalog infer`). Validation, catalog infer, scaffold, sync, Android setup, shell verify, and bootstrap app-icon gates run in-guest; host builds stay agent-run in the lent workspace per the write prompts.

## Verification audit posture

**Decision (2026-07).** Each first-party target's prompts and deterministic legs carry the verification the engine's pre-merge gate and post-merge hooks used to run. File-shaped and deterministic checks run in guest Rust; platform builds (`cargo`, `xcodebuild`, `gradlew`) stay agent-side because the guest cannot spawn host processes.

## Guest-model capability deferral

**Decision (2026-07).** The `adapter` crate's `Model` trait stays despite upstream omnia growing its own guest-side model capability. Every judgment `create` these adapters issue requires MCP references grants and the `"."` workspace lend — fields the upstream capability does not carry today.

**Swap criteria.** Revisit when upstream carries MCP tool grants and a workspace-lend affordance usable from a wasm-free core.

## Composed tests at root `tests/` with shared `harness` crate

**Decision (2026-07).** The composed-deployment test package lives at the workspace-root `tests/` directory (`adapter-tests`); shared host-side helpers live in `crates/harness`. `evals/` stays a separate package for the CI/live boundary and the dual native/wasm32 build.

## WIT consumption

**Decision (2026-07).** The engine repo (`augentic/specify`) owns and publishes the adapter contract as the wasm-pkg package `specify:adapter`; this repo consumes a vendored copy at `wit/specify.wit`. Refresh manually with `wkg get` — see [`wit/README.md`](wit/README.md). The `specify:` namespace routes to `augentic.io` via [`wit/.wkg-config.toml`](wit/.wkg-config.toml).

## Idempotent adapter publishing

**Decision (2026-07).** Adapter publishing is idempotent: each `specify:<name>@<version>` identity is probed with `wkg get` before pushing; a present identity is skipped. The probe treats only definitive not-found as permission to publish; any other failure aborts.

The publish loop lives in the `publish` cargo-make task; the release workflow is a thin caller. Publish auth is `GITHUB_TOKEN` alone.

_Codified in: `Makefile.toml` (`publish`); `.github/workflows/release.yaml`._

## Codex ownership

**Decision (2026-07).** Shared codex packs (`UNI-*`, `CORE-*`) are owned by the engine repo and materialize into consumer projects at init / rules sync. This repo's manually synced `codex/rules/` copy is deleted. `codex/references/` and per-adapter `prose/rules/` overlays stay here.

Adapter prompts cite `UNI-*` ids and point readers at `specify rules export` and the materialized codex cache — not repo-relative rule paths.

## Vectis validation and materialization

Provenance and rationale for the deterministic validation engine. Code citations point at `vectis-core` (`targets/vectis/core/`).

### Vectis UI artifact surface

The umbrella decision for `tokens.yaml`, `assets.yaml`, `layout.yaml`, `composition.yaml`, the embedded JSON Schemas, and the validator surface.

### §A — Unwired-subset rule

> `layout.yaml` is the unwired subset of the patched composition schema: it MUST NOT use the `delta` shape, and it MUST NOT carry any define-owned wiring keys (`maps_to`, `bind`, `event`, `error`, overlay `trigger`, conditional visual `*-when` keys). Wiring is added by `/spec:define` when it produces `composition.yaml`. The bare `when:` (`stateEntry.when`) is part of the unwired subset and is preserved.

_Codified in: `src/validate/engine/layout.rs::validate_layout`, `walk_unwired`, and `forbidden_wiring_key`._

### §E — Resolution checks live in the input validation gate

> Cross-artifact resolution checks (file existence for raster / vector assets, per-platform source coverage for composition-referenced assets, vector-source `sources.<plat>` requirement, and raster optional-density warnings) all live in the validation engine rather than in downstream consumers. Density warnings only fire for composition-referenced assets so unreferenced manifest entries do not generate noise.

_Codified in: `src/validate/engine/assets.rs::validate_assets`, `check_asset_files`, `check_platform_coverage`, and `check_file`._

### §F — V1 token-reference categories

> Composition-document keys map to `tokens.yaml` categories as follows: `color`, `background`, `border.color` → `colors.<name>`; `elevation` (groupProps) → `elevation.<name>`; string-valued `gap`, `padding`, `padding.<side>` → `spacing.<name>`; string-valued `corner_radius` → `cornerRadius.<name>`. `style`, `size.width`, and `size.height` are deliberately excluded from V1 reference resolution.

_Codified in: `src/validate/engine/composition.rs::resolve_token_references`, `walk_token_refs`, `token_category_for_key`, and `check_token_ref`._

### §G — Structural-identity rule

> Every group carrying the same `component: <slug>` directive MUST share a single canonical skeleton across the document. Slug instances MAY differ in `bind`, `event`, `error`, asset / token references, `*-when` condition values, and free text content, but their group skeleton MUST match across all base instances. `*-when` *key presence* participates in skeleton identity even though *condition values* do not. Per-instance `platforms.*` overrides MAY diverge from the base skeleton (edge case 3) and are exempt from base-equality.

_Codified in: `src/validate/engine/composition.rs::check_structural_identity`, `walk_for_components`, `build_group_skeleton`, `build_node_skeleton`, plus the `Skeleton` and `ComponentInstance` types. Layout mode reuses the same engine via `engine/layout.rs::validate_layout`._

### §H — CLI validation modes and default-path resolution

> When no `[path]` positional is supplied, each per-mode validator walks up from the current working directory looking for a `.specify/` ancestor and expands the canonical path cascade with `<name>` resolved against the alphabetically-first directory under `.specify/slices/`. Sibling discovery (assets → composition, composition → tokens / assets) routes through the same resolver. `validate all` fans out across `layout` → `composition` → `tokens` → `assets` and folds each per-mode envelope into a combined `{ "mode": "all", "results": [...] }` shape. Sub-modes whose default-resolved input is missing surface as a synthetic `{ skipped: true }` sub-report so the combined run does not bail. The dispatcher exits non-zero on errors, zero with a printed warning report on warnings, zero silently on a clean run.

_Codified in: `src/validate/engine/paths.rs::{resolve_default_path, resolve_default_path_with_root, default_project_root, discover_artifact, find_project_root, paths_for_key, expand_path_template, EMBEDDED_ARTIFACT_PATHS}` and `engine/all.rs::validate_all`._

### §I — Validation gate

> Composition mode auto-invokes sibling `tokens.yaml` and `assets.yaml` validators (in that order) when the files exist, and folds their per-mode envelopes into `results: [{ mode, report }]`. The fold shape matches `validate all` so the recursion-aware exit code helper picks up nested findings without extra plumbing.

_Codified in: `src/validate/engine/composition.rs::validate_composition` and `engine/mod.rs::run_inner`._

### Appendix A — embedded `tokens.schema.json`

> The embedded tokens schema is the tool-owned canonical `schemas/tokens.schema.json` in this crate; there is no upstream copy to mirror.

_Codified in: `src/validate/engine/shared.rs::TOKENS_SCHEMA_SOURCE` and `tokens_validator`._

### Appendix B — embedded `assets.schema.json`

> The embedded assets schema is the tool-owned canonical `schemas/assets.schema.json` in this crate. The order of platform densities (`1x`, `2x`, `3x` for iOS; `mdpi` … `xxxhdpi` for Android) matches the schema's `propertyNames` and is the order warnings render in.

_Codified in: `src/validate/engine/shared.rs::ASSETS_SCHEMA_SOURCE`, `assets_validator`, and `engine/assets.rs::raster_densities`._

### Appendix C — example `layout.yaml`

> Pinned verbatim as the happy-path schema fixture; any future drift surfaces in the layout-mode test suite first.

_Codified in: `core/tests/appendices.rs::APPENDIX_C_LAYOUT_YAML`._

### Appendix D — example `tokens.yaml`

> Pinned verbatim as the happy-path tokens schema fixture; any future drift surfaces in the tokens-mode test suite first.

_Codified in: `core/tests/appendices.rs::APPENDIX_D_TOKENS_YAML`._

### Appendix E — example `assets.yaml`

> Pinned verbatim as the happy-path assets schema fixture; any future drift surfaces in the assets-mode test suite first.

_Codified in: `core/tests/appendices.rs::APPENDIX_E_ASSETS_YAML`._

### Appendix F — patched `composition.schema.json`

> The embedded composition schema is the tool-owned canonical `schemas/composition.schema.json` in this crate, with the F-patch baked in. The schema is shared between `layout` mode (unwired-subset runtime) and `composition` mode (full lifecycle runtime). The F.2 patch's `component.not.enum` rejects reserved slugs (`header`, `body`, `footer`, `fab`).

_Codified in: `src/validate/engine/shared.rs::COMPOSITION_SCHEMA_SOURCE` and `composition_validator`._

### §J — Conservative directive emission

> The structural-identity validator only flags disagreement; it does not require ≥2 instances. A single `component:` instance passes silently because it has nothing to compare against.

_Codified in: `src/validate/engine/composition.rs::check_structural_identity` (early-exit when `base.len() < 2`)._

### Wiring resolution rules

> `maps_to` / `bind` / `event` / overlay `trigger` / navigation target full resolution against `design.md` / `specs/` is deferred to a follow-on contract. Composition mode's schema regex patterns shape-check these fields at parse time; the runtime resolution layer is intentionally out of scope here.

_Codified in: `src/validate/engine/composition.rs::validate_composition` and `engine/assets.rs::collect_asset_references`._

### §K — Materialization and render-by-`kind`

> Canonical `source:` files under `design-system/assets/` are designer-owned. Per-platform exports under `design-system/assets/exports/<platform>/` are tool-generated or operator-pinned derivatives recorded in `sources.<platform>`. Consumer repos version-control committed `exports/`; CI does not require image-processing deps on every job.
>
> **The materialize step** converts canonical masters into per-platform exports and auto-writes absent `sources.<platform>` pins. Operator pins win silently — when `sources.<platform>` is set and the path exists on disk, materialize skips that slot. Invocation is automatic in the build prepare prelude for in-scope missing exports; prepare resolves scope and materializes missing in-scope ids only.
>
> **Render-by-`kind`:** shell writers copy materialized exports into shell resources and emit view code by entry `kind` — `vector` / `raster` from shell catalogs, `symbol` only via explicit `symbols.<platform>` at the call site. Build-time substitution of platform glyphs for `vector` / `raster` ids is forbidden.

_Codified in: `src/validate/engine/assets/`; `src/materialize/`; `src/prepare.rs`._

#### Illustration raster scale table

| Platform | Slot      | Scale factor (×1× canvas) |
| -------- | --------- | ------------------------- |
| iOS      | `@2x`     | 2.0                       |
| iOS      | `@3x`     | 3.0                       |
| Android  | `mdpi`    | 1.0                       |
| Android  | `hdpi`    | 1.5                       |
| Android  | `xhdpi`   | 2.0                       |
| Android  | `xxhdpi`  | 3.0                       |
| Android  | `xxxhdpi` | 4.0                       |

`role: photo` (`kind: raster`) uses a copy-only path: per-density entries under `sources.<platform>` are copied byte-for-byte into the conventional `exports/<platform>/…` layout; no `resvg` pass.

_Codified in: `src/materialize/render.rs`, `illustrations/`, `raster_copy.rs`, `paths::{ios_scale_factor,android_density_factor}`._

### §L — Bootstrap `app-icon` gate

> **Trigger.** `project.yaml.platforms` is the sole authority for platform intent: the launcher `app-icon` gate fires for every declared UI platform (`ios` and/or `android`).
>
> **Validation rule.** For each declared UI platform: (1) shell-resident escape hatch — if `shell_resident_app_icon(project_dir, π)` is true, pass for `π` without design-system inventory; (2) otherwise require a satisfiable `app-icon` entry via canonical `source:` materialization or operator-pinned exports. Failure → `plan-bootstrap-app-icon-missing`.
>
> **Enforcement.** The gate runs in the build prepare prelude and parks the build on error-severity findings.

_Codified in: `src/shell/launcher.rs`; `src/verify/app_icon.rs`; `src/prepare.rs` and `src/operations.rs::bootstrap_findings`._

### Scaffold version-pin resolution

> The scaffold renderer resolves Crux + toolchain pins from embedded defaults plus an optional explicit complete TOML override. It does not inspect project-local or user-local configuration.

_Codified in: `src/scaffold/versions.rs::Versions::resolve`, `load_required`, and `load_embedded`._

### JSON Pointer

> Every error / warning entry carries a `path` field shaped like a JSON Pointer. Reference tokens are escaped per §3: `~` becomes `~0` and `/` becomes `~1`.

_Codified in: `src/validate/engine/shared.rs::escape_pointer_token` and path-construction call sites under `engine/assets.rs`, `engine/layout.rs`, and `engine/composition.rs`._

### Verify subcommand

#### §J — Platform shell verification

> The shell verify gate reads `project.yaml.platforms` as authority. Only `core`, `ios`, and `android` have on-disk interpretations today; `web` and `desktop` emit `platform-not-yet-supported` info findings and are treated as present.

_Codified in: `src/verify.rs`._

#### §L — iOS scaffold file immutability

> `iOS/Makefile`, `iOS/project.yml`, and `iOS/.vectis/sim-build.sh` are agent-immutable — rendered from embedded templates. The shell verify gate emits `ios-scaffold-file-drift` error findings when on-disk bytes diverge.

_Codified in: `src/ios_scaffold.rs`; wired from `src/prepare.rs`, `src/sync.rs`, `src/operations.rs::sync_shell_scaffold`, and `src/verify.rs`._
