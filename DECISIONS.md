# Decisions

Standing architectural decisions for the Specify adapters repository. Read before changing the adapter guest / core split, the vectis validation or materialization engines, embedded asset homes, or the verification posture of a target's prompts.

Each entry records the decision, why it was taken, and the consequences a change must reckon with — not how the feature works today. Current behavior lives in module-level rustdoc and the per-target prompts; entries here point at those rather than restating them.

**Anchor mapping note.** The sections under [§"Vectis validation and materialization"](#vectis-validation-and-materialization) (§A–§L and the appendices) were re-homed verbatim from `targets/vectis/extension/DECISIONS.md` at RFC-61 Step 5 Milestone A1, keeping every heading — and therefore every anchor — unchanged. External citations of the form `targets/vectis/extension/DECISIONS.md#k--materialization-and-render-by-kind` resolve here with the same fragment; Milestone A2 deleted the sidecar pointer stub with the extension crate, so this file is the only home.

## RFC-62 — the prose overlay dev loop

**Decision (2026-07).** The adapter author's prose iteration loop pays no compilation (RFC-62 — landed; removed from the `augentic/specify` tree, recoverable from its git history): the `adapter` crate gains a dev-only `prose-overlay` cargo feature, off by default and compiled out of `build-guests-release` and every published component — the overlay exists to iterate, never to certify, and graded evidence (committed run summaries, the eval sweep) comes from embedded builds only. Behavior lives in module rustdoc rather than here: the read path in `crates/adapter/src/registry.rs`, the runner in `evals/live.rs`, the driver in `evals/runtime.rs`. The standing consequences:

- **The overlay overrides bodies, never the doc set.** Registry and shelf lookups consult `.eval/prose/<key>` (under the guest's `"."` preopen) before the embedded table, but only for paths the table declares; a path absent from both keeps the registry's panic contract, and an overlay file that exists but cannot be read panics rather than silently serving the embedded body.
- **The cargo-skip is stamp-guarded.** In overlay mode the runner skips all three cargo legs when the artifacts exist *and* a SHA-256 stamp matches the adapter wasm from the last overlay-flagged build — presence alone cannot prove the feature is compiled in, because unflagged builds share the artifact path. A Rust edit under the overlay remains the RFC's stale-artifact trap; the escape hatch is re-running without it.
- **The eval guest drives one operation per invocation.** Its argv leads with an operation selector over the judgment-bearing seam operations (`survey`, `extract`, `guidance`, `build`, `merge`), so a source adapter's prompts are exercisable at all and a target author studies one prompt without paying a whole multi-leg build.
- **Model ids stay out of guests.** `SPECIFY_EVAL_MODEL` is read by the eval driver alone and applied as a decorator that fills `Request.model` only when the guest left it `None`; the cursor backend keeps its no-environment-configuration posture and the id never enters a guest or the WIT contract.

The hands-off loop is the `eval-watch` cargo-make task: a cargo-watch over one adapter's prose trees re-invoking a single required scenario filter with the overlay active — one model leg per save.

## RFC-64 Milestone R64-A — one component, no manifest

**Decision (2026-07).** The deployable adapter artifact is exactly one wasm component ([RFC-64](rfcs/rfc-64-adapter-artifact.md)): `adapter.yaml` and the committed `guest.wasm` blobs are retired, and every fact the manifest carried moves into the component or the registry reference. Consequences, piece by piece:

- **The `describe` operation.** Both axis interfaces in `wit/wit/specify.wit` gain `describe: func(id: adapter-id) -> manifest` — deterministic, effect-free, answerable from compiled-in constants (no model call, no filesystem access, no `result` wrapper: a describe that can fail is a design error). The source-axis `manifest` record carries only the optional `specify-floor` compatibility floor; the target-axis record adds the declared `inputs[]` (`build-input { path, required }`) and an optional `platforms-capability { required, allowed, default }`. Each core exposes `operations::describe()` over the wasm-free seam types (`SourceManifest` / `TargetManifest` in `adapter::seam`), and each shim maps it through the generated bindings — the same thin-delegation shape as every other operation. Current values mirror the retired manifests exactly: no adapter declares a floor; contracts declares the optional `contracts` input; vectis declares the three optional design-system inputs plus its required platforms capability; everyone else declares nothing.
- **Identity lives in the guest crate.** Each adapter's semver moved from `adapter.yaml.version` to its guest crate's `Cargo.toml` `version` (dropping `version.workspace = true`; all at `1.0.0`, vectis at `1.0.4`) — the single identity declaration, and the `<semver>` the publish workflow reads via `cargo metadata` to push `augentic:<name>@<semver>`. Axis is the exported world (`source` xor `target`); `description` moves to registry package metadata (the crate `description` survives as its source). **Amendment (RFC-65):** namespaces follow product, not org — the published identity is `specify:<name>@<semver>`; `augentic:` is reserved for future org-wide contracts, with no compatibility alias.
- **No committed wasm.** The eight `guest.wasm` blobs and the `refresh-guests` copy task are deleted; `build-guests-release` release-builds the components where cargo puts them (`target/wasm32-wasip2/release/<name>.wasm`), which is where the publish workflow pushes from. The runtime tests already build guests from source into the cargo target directory and were unaffected. Per the RFC's invariant: if the engine's sibling-checkout developer loop proves too slow, the fix is a fetch-from-registry developer manifest, never a return to committed blobs.
- **wasm-pkg publish replaces the tree pack.** `.github/workflows/release.yaml` no longer builds the platform `specify` binary or calls `specify adapter build / publish` (the RFC-48 `tar+zstd` single-layer path): it release-builds the guests and pushes each component with `wkg publish` under the `augentic` namespace (`augentic:<name>@<semver>`), with registry auth written into the `wkg` config from the existing username/password secrets. **Amendment (RFC-65):** the publish namespace is `specify` (`specify:<name>@<semver>`); `augentic.io` remains the registry host.
- **`check-pins` migration window.** The `describe` op landed in this repo's WIT first (the RFC is filed here temporarily). Until the sibling specify checkout re-vendors, `check-pins` skips the `wit/wit/specify.wit` byte-parity pair when the sibling copy predates `describe`, printing a notice; once the sibling carries `describe: func`, byte parity is enforced again. The answer-schema pairs stay strict throughout. **Retired (RFC-66):** the sibling-`cmp` WIT arm and its carve-out are gone — the WIT arm now verifies the vendored copy against the pinned published `specify:adapter` package (see §"WIT ownership flip: vendored published pin").

## Prose tree rename — `briefs/` → `prompts/`

**Decision (2026-07).** Every adapter's `prose/briefs/` tree is renamed to `prose/prompts/` (git renames across all eight adapters), and the word "brief" leaves the adapter vocabulary. The old name described the pre-WASM contract — an agent-read orchestration document with operator-facing flexibility. Post-cutover, these files are **embedded prompt fragments**: `build.rs` emits them into the guest via `prose`, the core's operation code assembles them into judgment-leg system prompts (`registry::body("prompts/…")`), and sequencing lives in Rust, not prose. The rename makes the on-disk name match the role. Consequences: registry keys, shelf-pointer prose, tests, and cross-references all use `prompts/…`; the parent/phase brief hierarchy and its lint rules (CORE-004/007/013/014) die engine-side in the same pass; "operator brief" (the intent source's input document) is a distinct surviving term and is untouched.

## RFC-61 Step 5 Milestone A1 — vectis self-containment

**Decision (2026-07).** The remaining `vectis-extension` subcommand logic is absorbed into `vectis-core` as plain library code; the extension crate became a thin CLI shim (clap parsing + JSON rendering + `resolve_project_root`) over the core, deleted at Milestone A2. Absorption map (extension module → core module):

| Extension module           | Core module                                      | Notes                                                                                                                                                                                                                 |
| -------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `infer`                    | `core/src/infer.rs`                              | The catalog-infer **report** phase (deterministic, name-free clustering); the engine's bind bookkeeping stays engine-side. The guest build runs the report in-guest and injects it into the composition leg's prompt. |
| `verify`                   | `core/src/verify.rs` (+ `verify/app_icon.rs`)    | `verify` and `bootstrap-app-icon` modes move; `host-prereq` mode stayed extension-local (`extension/src/host_prereq.rs`) — it probes the host environment and is not wasm-clean — and died with the extension at A2.  |
| `scaffold`                 | `core/src/scaffold.rs` (+ `scaffold/`)           | Render-only scaffolding with embedded version pins; `run_at` takes an explicit project dir (no env reads in core).                                                                                                    |
| `sync`                     | `core/src/sync.rs`                               | Lightweight scaffold repair without prepare side effects.                                                                                                                                                             |
| `android`                  | `core/src/android.rs`                            | Vendored Gradle-wrapper install; the `#[cfg(unix)]` chmod leg was a no-op under WASI and did not move.                                                                                                                |
| `schema` / `schema_source` | `core/src/schema_source.rs`                      | Published-schema source of truth; the JSON files live under `core/schemas/`.                                                                                                                                          |
| `shell`                    | `core/src/shell.rs` (+ `shell/launcher.rs`)      | Shell presence and shell-resident app-icon detection.                                                                                                                                                                 |
| `prepare` (orchestration)  | `core/src/prepare.rs` (`run_build`, `exit_code`) | The full prepare orchestration (materialize + bootstrap gate + android setup + iOS sync) joins the previously absorbed `materialize_step`.                                                                            |

Unit tests under `extension/src/**` (the 14-test ratchet budget) moved to `core/tests/` as integration tests over the core's public surface; the `vectis` line in `tools/rust-quality/rust_quality_budget.toml` is removed (implicit budget 0). The extension's integration tests kept passing against the shim re-exports as a secondary oracle until A2 deleted them (see the A2 entry below for what was re-homed).

**Asset relocation.** `extension/schemas/` → `core/schemas/`; `extension/templates/` → `core/templates/`; `extension/assets/` → `core/assets/`; `extension/versions.toml` → `core/versions.toml`. The template-registry generation from `extension/build.rs` merged into `core/build.rs` (which already embedded the prose registry); all `include_str!` / `include_bytes!` depths updated on both sides. Nothing asset-shaped remains extension-owned, so A2's deletion is a pure crate removal.

## RFC-61 Step 5 Milestone A1 — prompt and prose cutover

**Decision (2026-07).** No compiled-in prompt and no prose document in this repository instructs the spawned agent (or an operator) to run `specify extension run …` or `specify catalog infer` — both CLI surfaces die at the Step 5 cutover. The replacement posture, per capability:

- **Composition / tokens / assets validation** — deterministic in-guest gates: the build's post-composition gate with a bounded repair loop, the report-leg gate, the merge's pre-fold staged-slice gate, and the post-merge baseline gate (`operations.rs`). Prose describes "the adapter's deterministic composition validator", never a command.
- **Catalog infer** — the guest runs the deterministic name-free cluster report in-guest and injects it into the composition leg's prompt; the agent writes `{ fingerprint → slug }` decisions to `${SLICE_DIR}/build/component-bindings.yaml`; the workflow's deterministic bind bookkeeping (engine-side) is the only catalog writer.
- **Scaffold** — the guest scaffolds absent declared trees deterministically before the write legs (app name from existing shells or `project.yaml` `name:`); a scaffold the guest cannot run falls to the leg's writer per the write prompt.
- **Sync** — the guest re-renders the agent-immutable scaffold files deterministically before and after each shell write leg.
- **Android setup** — the vendored Gradle-wrapper install runs inside the deterministic Android sync leg; the scaffolded `Android/Makefile` drops its `setup-extension` target (`specify extension run vectis -- android setup`) and `make setup` covers only host-derived pieces (`local.properties`, Java home, NDK substitution).
- **Shell verify** — the deterministic shell verify gate runs in-guest at the report leg (findings ride in the prompt) and re-runs in the deterministic report gate.
- **Bootstrap app-icon gate (§L)** — runs in the guest build's deterministic prelude after materialize; error findings park the build.
- **Host builds** (cargo / swiftformat / make / xcodebuild / gradlew) — stay agent-run in the lent workspace, instructed by the write prompts and the merge prompt's host cap-matrix section; the adapter cannot spawn host commands.

Consequence: the `layout-inferer-contract`, verifier references, and `components.md` describe gates and modes, not invocations; prose must describe the surviving surface rather than retired CLI verbs such as `specify extension run` or `specify catalog infer`.

## RFC-61 Step 5 Milestone A1 — D2 verification audit

**Decision (2026-07).** Per engine decision D2 (deterministic-only guest merge) and parity-audit gap 4, each first-party target's prompts and deterministic legs were audited to confirm they carry the verification the native pre-merge gate and post-merge hooks used to run. Result, per target (home: **deterministic** = guest Rust before/after the model call; **prompt** = agent-run in the lent workspace, instructed by the compiled-in prompts; **added-now** = wired during this audit):

| Target    | Check                                                                           | Home                                                                                                                                           |
| --------- | ------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| omnia     | `cargo fmt --check` / `cargo check` / `cargo clippy -D warnings` / `cargo test` | prompt — build prompt §Verify-repair loop; re-run by the merge prompt §Omnia pre-merge gate                                                    |
| omnia     | `cargo build --target wasm32-wasip2 --release`                                  | prompt — merge prompt §Omnia pre-merge gate step 4                                                                                             |
| omnia     | capability / provider-trait conformance                                         | prompt — crate write prompt (capability-mapping) + review leg rules (OMNIA-001/002)                                                            |
| vectis    | composition / tokens / assets validation (build)                                | deterministic — post-composition gate with bounded repair + report gate                                                                        |
| vectis    | staged slice composition validation (pre-merge)                                 | **added-now** (deterministic) — `operations::merge` validates `.specify/slices/<slice>/composition.yaml` before the fold and parks on findings |
| vectis    | merged baseline composition validation (post-merge)                             | deterministic — merge report gate with one bounded repair leg                                                                                  |
| vectis    | platform builds (cargo / make build / make sim-build / make verify)             | prompt — write prompt verify loops at build; merge prompt §host cap-matrix re-verification at merge                                            |
| vectis    | shell verify (tree presence, APK, scaffold drift)                               | deterministic — in-guest verify at the report leg + report gate                                                                                |
| vectis    | bootstrap app-icon gate (§L)                                                    | **added-now** (deterministic) — guest build prelude after materialize                                                                          |
| vectis    | vendored Gradle-wrapper install                                                 | **added-now** (deterministic) — Android scaffold-sync leg (replaces the Makefile's `setup-extension`)                                          |
| contracts | contract validation (slice delta, build)                                        | deterministic — in-core validate gate with bounded repair + `enforce_validators` postlude                                                      |
| contracts | contract validation (merged baseline, post-merge)                               | deterministic — post-merge validator gate with one bounded repair leg                                                                          |

No check was left homeless. The vectis platform builds and the omnia cargo/wasm32 gates are deliberately agent-side (the guest cannot spawn host processes); everything file-shaped and deterministic runs in guest Rust.

## RFC-61 Step 5 Milestone A2 — old-stack deletion

**Decision (2026-07).** The legacy WASI extension stack is deleted: `targets/vectis/extension/` (65 files, ~8.9k lines — the clap shim, its `tests/cli.rs` + `tests/engine/**` integration suites, and the extension-local `host_prereq.rs`), `targets/contracts/extension/` (3 files, ~500 lines), the two committed `adapter.wasm` artifacts (`targets/{vectis,contracts}/adapter.wasm`), and the `targets/vectis/scripts/` native hook scripts (3 files, ~270 lines). The workspace drops the `"targets/*/extension"` members glob and the now-orphaned `clap` / `assert_cmd` `[workspace.dependencies]`. Standing consequences:

- **Hook scripts and `check-hook-scripts` die together.** `build-finalize-verify.sh` shelled out to `specify extension run vectis` (a verb that dies at cutover) and `build-host-prereq.sh` existed only for the manifest's `host_prereq:` field, which the manifest shrink below removes — with no adapter.yaml consumer and no runnable body, the scripts and the `check-hook-scripts` task (dropped from the `check` / `ci` dependency lists in `Makefile.toml`) have no surviving purpose. Host-toolchain preflight is operator-owned post-cutover; the merge prompt's host cap-matrix section instructs the agent-run platform builds that surface missing toolchains.
- **The extension integration suites are deleted, not ported.** A1 designated the core `tests/` tree the primary oracle; the extension suites covered the CLI wire contract (arg parsing, exit codes, stdout JSON), which has no post-cutover surface. The exception: the three DECISIONS-cited appendix fixture pins (Appendix C / D / E) re-home to `core/tests/appendices.rs` against the core engine API, so the "codified in" pointers under §"Vectis validation and materialization" stay true.
- **The rust-quality ratchet re-scopes.** `tools/rust-quality` counted `{targets,sources}/<name>/extension/src/**`; with no extension trees left that gate would be vacuously green forever, so it now counts every adapter `src/` tree (the guest shim and its `crates/*/src/` sub-crates), same budget semantics, all budgets at the implicit 0.
- **Lib-name reclaim.** With `vectis-extension` gone, the `specify_vectis` lib name is free; the vectis guest package drops its `[lib] name = "specify_vectis_adapter"` override and defaults to `specify_vectis`, so the built artifact is `vectis.wasm` like every other guest. The committed artifact name is unchanged (`targets/vectis/guest.wasm`); `refresh-guests`, the runtime tests' deployment manifests, and the vectis eval runner track the new artifact name. The committed `guest.wasm` bytes are deliberately not refreshed here — Milestone A3 owns artifact refresh.
- **CORE-061 dies with the extension machinery** (post-review addendum). `codex/rules/core/CORE-061-adapter-extension-crate-missing.md` checked that every adapter declaring `adapter.yaml.extension` shipped a co-located `extension/` crate and a committed `adapter.wasm` — all three of which this milestone deletes (the extension crates above, the `adapter.wasm` artifacts, and the manifest `extension:` field in the manifest-shrink entry below). The rule is vacuous with nothing left to trigger on, so the rule file is deleted; no index or citation referenced it.

## RFC-61 Step 5 Milestone A2 — `shape` → `guidance` rename

**Decision (2026-07).** Per RFC-61 §"The contract revision", the target read-at-synthesis leg is `guidance` everywhere; the adapter vocabulary's `shape` term is retired. `targets/{contracts,omnia,vectis}/briefs/shape.md` moved to `briefs/guidance.md` (git renames); the three cores' `guidance()` operations read `registry::body("briefs/guidance.md")`; brief H1s and self-references, the omnia/vectis build-leg brief assemblies, the registry/operations/runtime tests, and prose citing `shape` as the operation name (references READMEs, `vectis.mdc`, shared runtime references, CORE-014's brief path glob) all follow. Incidental uses of the word "shape" (`report-shape.md`, "wire shape", "data shape") are untouched. The `briefs.shape` manifest key needed no rename because the manifest shrink below deletes the `briefs:` map outright. The forked manifest-policing CORE rules (`CORE-001` / `CORE-004` / `CORE-007` and peers) described the full-shape manifest grammar whose key spelling was `shape` — they were deleted with the RFC-61 lint shrinkage pass alongside the sibling specify repo rather than being reworded here.

## RFC-61 Step 5 Milestone A2 — manifest shrink

**Decision (2026-07).** All eight `adapter.yaml` manifests pare down to the post-cutover field set: `name`, `version`, `axis`, `description`, plus `platforms` on vectis. The engine's shrunk-shape resolver (sibling Milestone S2) derives the operation set from the WIT contract, and the guests embed their own prose via `prose`, so nothing reads manifests for operation dispatch or agent handoffs. Field-by-field disposition:

| Field                               | Was on            | Disposition                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ----------------------------------- | ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `briefs:`                           | all eight         | dropped — operation set derives from `wit/wit/specify.wit`; briefs are compiled into the guests                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `execution:`                        | all eight         | dropped — the two-phase agent handoff dies at cutover; the WIT operations are the only dispatch surface                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `extension:`                        | vectis, contracts | dropped — the extension crates are deleted above; `specify extension run` dies at cutover                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `prepare:`                          | vectis            | dropped — prepare runs in-guest as the build prelude (`prepare::run_build`), not as a host-dispatched extension subcommand                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `host_prereq:` / `finalize_verify:` | vectis            | dropped — the native two-phase `slice build` hook dispatch dies at cutover and the scripts are deleted above                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `catalog:`                          | vectis            | dropped — the `infer` flag existed to let `specify catalog infer` dispatch the extension's `infer` subcommand; that dispatch path is dead (extension deleted). The inference report already runs in-guest during every vectis build (`infer.rs` + the composition leg's prompt), so the capability marker is the guest itself; the engine's residual `catalog` verbs operate on the bindings file and `components.yaml`, not the manifest. If a future engine milestone needs a manifest-level capability marker again, it can be added back as a one-line field. |
| `inputs:`                           | vectis, contracts | dropped — the build-request assembly that consumed `inputs[]` is native two-phase machinery; the guests read their build inputs (`tokens.yaml`, `assets.yaml`, `components.yaml`, `contracts/`) directly from the lent workspace                                                                                                                                                                                                                                                                                                                                  |
| `platforms:`                        | vectis            | **kept** — `platforms.required` drives `specify init --platforms` validation engine-side, a live post-cutover consumer                                                                                                                                                                                                                                                                                                                                                                                                                                            |

Each shrunk manifest validates against the sibling repo's relaxed `source.schema.json` / `target.schema.json` (verified against the schemas at S2 head). The omnia and vectis `description` strings were reworded for the `shape` → `guidance` rename in the same pass.

## `tools/rust-quality` ratchet removal

**Decision (2026-07).** The `tools/rust-quality` workspace member and its per-adapter src unit-test budget gate are deleted. The gate was re-scoped at Milestone A2 to count adapter `src/` trees after the extension crates went away; every adapter had already reached implicit budget 0, and the prescribed WIT interface plus `core/tests/` and the composed-deployment integration suites (now the root `tests/` package) are the meaningful guardrails — a separate ratchet crate added maintenance surface without catching contract drift the existing layers do not already cover. The integration-first posture in `TESTING.md` stays; only the mechanical CI counter retires.

## Composed tests re-homed to root `tests/` with a shared `harness` crate

**Decision (2026-07).** The composed-deployment test package moves from `crates/tests` to the workspace-root `tests/` directory and is renamed `adapter-tests`; the stub `src/lib.rs` it carried is replaced by a real shared crate at `crates/harness`. A dev-only test host was a category error under `crates/` (shared guest support), and the generic package name made `cargo test -p tests` ambiguous. The workspace is virtual, so a root `tests/` member is unambiguous and mirrors the engine repo's root-`tests/`-for-E2E convention. The `harness` crate owns the host-side pieces both suites had duplicated — cargo-target-dir discovery, the subprocess `cargo` runner, deployment-manifest rendering over `Guest` entries, and `copy_tree` — consumed as a host-only dev-dependency by `tests/` and `evals/` (kept out of the wasm32 example builds by the usual target gate). Each package now has one job: `crates/` = libraries, `tests/` = the deterministic CI gate, `evals/` = the live eval loop. The `evals/` package stays separate: the CI/live boundary, the `omnia-cursor` git dependency, the dual native/wasm32 build, and the operator-facing scenario/run trees justify the package split; only the duplicated harness code collapses.

## RFC-61 Step 5 Milestone A3 — guest-model capability deferral

**Decision (2026-07).** The `adapter` crate's `Model` trait (`crates/adapter/src/model.rs`) stays, despite upstream omnia growing its own guest-side model capability (`omnia-guest::capabilities::Model`, present as of the rev this workspace pins). The upstream capability deliberately mirrors `omnia:model/completion` *minus `tools` and `grants`* — workspace lending borrows a `wasi:filesystem` descriptor resource that only exists on `wasm32`, so it always sends `tools: vec![]` and empty grants, and points guests needing more at the raw `omnia-wasi-model` binding. Every judgment `create` these adapters issue requires both: the MCP reference-shelf grant and the `"."` workspace lend. The trait exists precisely to carry those two fields across the wasm-free core boundary (`Request::mcp` + `Request::lend_workspace`, with the `wasm32` default body resolving the lend against the guest's own preopen), so the upstream capability cannot replace it today.

**Swap criteria.** Revisit if/when the upstream capability grows tools/grants support (likely alongside the post-RFC-60 verify work): the swap is worthwhile only if upstream carries MCP tool grants *and* a workspace-lend affordance usable from a wasm-free core. Until then the trait is a deliberate fork, not drift. The specify engine's `specify-guest-model` byte-mirror keeps the same posture; the engine repo records its own entry at its step boundary.

## RFC-61 Step 5 review fixes — ui-surface coherence moves in-guest

**Decision (2026-07).** The A4 ui-surface coherence check — the report's authored `ui-surface.screens` compared against the produced slice `composition.yaml`, warning ids `composition-unexpected-for-non-ui-slice` / `composition-empty-for-ui-slice` — now runs in the vectis guest's deterministic build report gate (`operations.rs::ui_surface_coherence`, appended after enforcement), with the engine semantics preserved verbatim: findings are non-blocking `suggestion` severity that ride the report but never fail it or trigger the bounded repair leg, and a report without `ui-surface` emits nothing. This closes the last finalize-era check that lived only in the engine's two-phase `slice build` path, so the build prompt's attribution of the warnings to the deterministic in-guest report gate is true ahead of the engine's Milestone S4 deletion.

## Vectis validation and materialization

Provenance and rationale for the deterministic validation engine, re-homed from `targets/vectis/extension/DECISIONS.md` (see the anchor mapping note above). Code citations point at `vectis-core` (`targets/vectis/core/`), where the engine lives as of Milestone A1; inline comments in `src/validate/engine/` state the rules without historical labels; this file carries the citation.

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

_Codified in: `src/validate/engine/paths.rs::{resolve_default_path, resolve_default_path_with_root, default_project_root, discover_artifact, find_project_root, paths_for_key, expand_path_template, EMBEDDED_ARTIFACT_PATHS}` and `engine/all.rs::validate_all` (the `validate all` fan-out). The exit-code framing describes the legacy CLI dispatcher; in-guest callers consume the envelope's errors / warnings directly._

### §I — Validation gate

> Composition mode auto-invokes sibling `tokens.yaml` and `assets.yaml` validators (in that order) when the files exist, and folds their per-mode envelopes into `results: [{ mode, report }]`. The fold shape matches `validate all` so the recursion-aware exit code helper picks up nested findings without extra plumbing.

_Codified in: `src/validate/engine/composition.rs::validate_composition` (auto-invoke + cross-artifact resolution layer) and `engine/mod.rs::run_inner` (the re-entrant dispatch helper)._

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

_Codified in: `src/validate/engine/shared.rs::COMPOSITION_SCHEMA_SOURCE` and `composition_validator`. Reserved-slug rejection is exercised by the layout- and composition-mode test suites._

### §J — Conservative directive emission

> The structural-identity validator only flags disagreement; it does not require ≥2 instances. A single `component:` instance passes silently because it has nothing to compare against.

_Codified in: `src/validate/engine/composition.rs::check_structural_identity` (early-exit when `base.len() < 2`)._

### Wiring resolution rules

> `maps_to` / `bind` / `event` / overlay `trigger` / navigation target full resolution against `design.md` / `specs/` is deferred to a follow-on contract. Composition mode's schema regex patterns (`bindValue`, `eventValue`, `triggerValue`) shape-check these fields at parse time; the runtime resolution layer is intentionally out of scope here. Phase 1.7's static-asset walker (the `image` / `icon` / `icon-button` / `fab` reference shape) is reused by composition mode for asset-id resolution.

_Codified in: `src/validate/engine/composition.rs::validate_composition` (deliberate deferral note) and `engine/assets.rs::collect_asset_references` (the shared walker composition mode reuses)._

### WASI command surface

> The deterministic engine and the embedded schemas live in the `vectis-core` library so every consumer — today, the guest's build / merge gates — has a single source of truth. The dispatcher renders a flat body with `mode`, `errors: [...]`, `warnings: [...]`, and (for `all` / auto-invoke) `results: [...]`; in-guest gates consume the envelope directly and treat any real sub-report error as blocking.

_Codified in: `src/validate.rs` (the public `ValidateMode`, `run`, and `validate_exit_code` surface); the legacy CLI shim entry point died with the extension crate at A2._

### RFC-46 — asset materialization

Canonical spec: [`augentic/specify` `rfcs/rfc-46-asset-materialization.md`](https://github.com/augentic/specify/blob/main/rfcs/rfc-46-asset-materialization.md).

### §K — Materialization and render-by-`kind`

> Canonical `source:` files under `design-system/assets/` are designer-owned. Per-platform exports under `design-system/assets/exports/<platform>/` are tool-generated or operator-pinned derivatives recorded in `sources.<platform>`. Consumer repos version-control committed `exports/`; CI does not require image-processing deps on every job.
>
> **The materialize step** (Phase 2) converts canonical masters into per-platform exports and auto-writes absent `sources.<platform>` pins. Operator pins win silently — when `sources.<platform>` is set and the path exists on disk, materialize skips that slot. Invocation: automatic in the build prepare prelude for in-scope missing exports (§2.1 of RFC-46); prepare resolves scope and materializes missing in-scope ids only.
>
> **Report envelope (stable):** success payloads are flat JSON with `command: "materialize assets"`, the resolved inventory `path`, `dry_run`, the effective `platforms` filter, and three arrays:
>
> - `materialized[]` — `{ "asset_id", "platform", "path" }` per export written (absent in `--dry-run` when no write would occur).
> - `skipped_pins[]` — `{ "asset_id", "platform", "pin" }` for operator-pinned slots skipped silently.
> - `errors[]` — `{ "path", "message" }` entries (JSON Pointer-shaped `path` when the failure targets a sub-document). Non-empty `errors` blocks the build; a missing inventory or bad arguments surface through the shared error envelope.
>
> **Render-by-`kind`:** shell writers copy materialized exports into shell resources and emit view code by entry `kind` — `vector` / `raster` from shell catalogs, `symbol` only via explicit `symbols.<platform>` at the call site. Build-time substitution of platform glyphs for `vector` / `raster` ids is forbidden.

_Codified in: `src/validate/engine/assets/` (export presence, `assets-materialization-missing`, `assets-app-icon-*`); `src/materialize/` (report envelope, `paths.rs` export conventions, `icons/` SVG→PDF/VD XML, `illustrations/` SVG→PNG, `raster_copy.rs` photo density copy); `src/prepare.rs` (slice-build prepare orchestration: scope resolution, conditional scoped materialize, bootstrap gate)._

#### Illustration raster scale table (R46-S18)

Vector `role: illustration` masters render from the SVG 1× logical canvas (`usvg` `Tree::size()` width/height) through `resvg` at platform density factors:

| Platform | Slot      | Scale factor (×1× canvas) |
| -------- | --------- | ------------------------- |
| iOS      | `@2x`     | 2.0                       |
| iOS      | `@3x`     | 3.0                       |
| Android  | `mdpi`    | 1.0                       |
| Android  | `hdpi`    | 1.5                       |
| Android  | `xhdpi`   | 2.0                       |
| Android  | `xxhdpi`  | 3.0                       |
| Android  | `xxxhdpi` | 4.0                       |

`role: photo` (`kind: raster`) uses a **copy-only** path: per-density entries under `sources.<platform>` are copied byte-for-byte into the conventional `exports/<platform>/…` layout (`paths.rs`); no `resvg` pass.

_Codified in: `src/materialize/render.rs`, `illustrations/`, `raster_copy.rs`, `paths::{ios_scale_factor,android_density_factor}`._

### §L — Bootstrap `app-icon` gate

> **Trigger.** `project.yaml.platforms` is the sole authority for platform intent: the launcher `app-icon` gate fires for every declared UI platform (`ios` and/or `android`). There is no filesystem shell scan, no `missing[]` probe, and no `plan.yaml` slice-name inspection — whether a shell tree happens to exist on disk is irrelevant to the trigger. A `core`-only project never triggers the gate.
>
> **Validation rule (§6.2).** For each declared UI platform `π`: (1) **shell-resident escape hatch** — if `shell_resident_app_icon(project_dir, π)` is true (§6.3), pass for `π` without design-system inventory; (2) otherwise require `design-system/assets.yaml` top-level `app-icon` pointing at a `role: app-icon` entry satisfiable for `π` via path A (canonical `source:` materializable) or path B (operator-pinned export tree at `exports/<π>/app-icon/`). Failure → an error-severity `plan-bootstrap-app-icon-missing` finding.
>
> **Enforcement point — build-time only.** The gate runs in the build prepare prelude (after conditional asset materialization) and parks the build on any error-severity finding. It does **not** run at `plan validate`: platform shell bootstrap is a build-time adapter concern, never a plan-time check.

_Codified in: `src/shell/launcher.rs` (`shell_resident_app_icon`, §6.3); `src/verify/app_icon.rs` (the gate behind `VerifyMode::BootstrapAppIcon`); `src/prepare.rs` and `src/operations.rs::bootstrap_findings` (the deterministic prelude enforcement)._

### Scaffold version-pin resolution

> The scaffold renderer resolves Crux + toolchain pins from embedded defaults plus an optional explicit complete TOML override. It deliberately does not inspect project-local or user-local configuration, keeping the render surface deterministic across hosts.

_Codified in: `src/scaffold/versions.rs::Versions::resolve`, `load_required`, and `load_embedded`._

### JSON Pointer

> Every error / warning entry carries a `path` field shaped like a JSON Pointer (the same `instance_path` the `jsonschema` crate reports for schema findings, and a hand-rolled equivalent for our own cross-artifact findings) so operators can locate the offending sub-document. Reference tokens are escaped per §3: `~` becomes `~0` and `/` becomes `~1`.

_Codified in: `src/validate/engine/shared.rs::escape_pointer_token` and the path-construction call sites under `engine/assets.rs`, `engine/layout.rs`, and `engine/composition.rs`._

### Verify subcommand

#### §J — Platform shell verification

> The shell verify gate reads `project.yaml.platforms` as authority and inspects on-disk shell trees to determine which declared platforms are present. Only three platforms have on-disk interpretations today: `core` → `shared/src/app.rs`; `ios` → `iOS/` with ≥ 1 `.swift` file; `android` → `Android/` with ≥ 1 `.kt` file. `web` and `desktop` are accepted but have no on-disk interpretation — they emit a `platform-not-yet-supported` info finding and are treated as present.
>
> Two modes: `detect` returns the missing set (always clean); `verify` emits `diagnostic.schema.json`-shaped findings with `severity: error` for missing supported platforms — any error finding blocks. Runtime failures (missing `project.yaml`, parse errors) surface as errors in their own right.

_Codified in: `src/verify.rs` (`run`, `check_platform`, `render_detect`, `render_verify`, `verify_exit_code`)._

#### §L — iOS scaffold file immutability

> `iOS/Makefile`, `iOS/project.yml`, and `iOS/.vectis/sim-build.sh` are agent-immutable. They render exclusively from the embedded iOS assembly templates. The simulator destination lives only in `sim-build.sh` as `generic/platform=iOS Simulator`; the Makefile delegates `sim-build` to that script.
>
> The prepare orchestration auto-syncs all three paths when `ios` is declared and `iOS/` exists; the guest build re-renders the same files deterministically around each shell write leg without prepare side effects. The shell verify gate emits `ios-scaffold-file-drift` error findings when on-disk bytes diverge.

_Codified in: `src/ios_scaffold.rs`; wired from `src/prepare.rs`, `src/sync.rs`, `src/operations.rs::sync_shell_scaffold`, and `src/verify.rs`._

## WIT ownership flip: vendored published pin (RFC-66)

> The engine repo (`augentic/specify`) owns and publishes the adapter contract as the wasm-pkg package `specify:adapter`; this repo only consumes it. The vendored copy lives at `wit/specify.wit` (conventional wasm-tools deps layout); the root `wit/wit/specify.wit` copy is deleted. The pin is declared in exactly one place — the `WIT_PIN` env var at the top of `Makefile.toml` — and `cargo make wit-vendor` refreshes the vendored file from the published package (temp-file fetch, move on success, so a failed fetch never corrupts the vendored copy). `cargo make wit-vendor-sibling` is the dev-loop override: it copies `../specify/wit/wit/specify.wit` into the vendored location while a contract change is iterating in the engine before the new version is published; the published pin is the release posture.
>
> `check-pins`'s WIT arm verifies the vendored bytes against the pinned published version instead of a sibling `cmp`, and the RFC-64 migration-window carve-out (the `describe: func` probe) is deleted. Pre-first-publish posture: until `specify:adapter@<WIT_PIN>` is fetchable (the first publish rides the next engine tag), the arm skips with a notice naming the pin, keeping CI green on runners with no registry access. The three answer-schema arms stay sibling-based and strict — schema ownership is not RFC-66's story.

_Codified in: `Makefile.toml` (`WIT_PIN` / `WIT_VENDORED`, `check-pins`, `wit-vendor`, `wit-vendor-sibling`); `wit/.wkg-config.toml` (the `specify:` → `augentic.io` namespace routing); `wit/README.md`; `crates/adapter/src/{source,target}.rs` and `evals/guest.rs` (`wit_bindgen::generate!` paths)._

## Idempotent adapter publishing on GITHUB_TOKEN (RFC-66)

> Adapter publishing is idempotent: each `<namespace>:<name>@<version>` identity is probed with `wkg get` before pushing, and a present identity is skipped — a version is published at most once, ever, so a tag carrying a mixed bag of bumped and untouched adapters (or a workflow re-run against an already-published tag) succeeds and pushes only what moved. The probe treats only a definitive not-found as permission to publish; any other failure (network unreachable, auth, timeout) aborts the leg non-zero, because absent and unreachable are indistinguishable and guessing "absent" would re-push into an immutable identity.
>
> The publish loop lives in the `publish-adapters` cargo-make task; the release workflow is a thin caller. Local emergency publishing runs the same task with the developer's own token in their wkg config — one code path, two invocation surfaces. Publish auth is `GITHUB_TOKEN` alone (`permissions: packages: write` plus the workflow-written wkg config); the `SPECIFY_REGISTRY_USERNAME` / `SPECIFY_REGISTRY_PASSWORD` secrets are retired.

_Codified in: `scripts/wkg-publish-idempotent.sh` (the probe-then-publish helper); `Makefile.toml` (`publish-adapters`); `.github/workflows/release.yaml`._

## Codex ownership flip: shared packs live in the engine (RFC-66)

> The shared codex packs (`codex/rules/universal/` — `UNI-*` — and `codex/rules/core/` — `CORE-*`) are owned by the engine repo (`augentic/specify`): they compile into the `specify` binary there and materialize into each consumer project's cache (`<project-cache>/codex/codex/rules/{universal,core}/`) at init / rules sync. The engine's ancestor walk that used to discover a `codex/rules/universal/` tree near a resolved adapter component is deleted, so this repo's manually synced `codex/rules/` copy — the shipping fork that walk existed to find — is deleted with it, and the two-repo rules-sync discipline dies. `codex/references/` (the prose overlay tree embedded into components) is unaffected and stays; per-adapter overlays (`targets/<name>/prose/rules/`, `sources/<name>/prose/rules/`) stay here as adapter-owned policy.
>
> Adapter prompts no longer link shared rules by repo-relative path. They cite `UNI-*` ids (stable citation keys, never renumbered) and point readers at the consume surfaces that exist in a consumer project: `specify rules export` and the binary-materialized codex cache — the same surfaces the prompts' deterministic-review sections already describe. `CORE-*` namespace enforcement (the CORE-009 family) continues where the rules now solely live; this repo's CI never ran `specify lint framework`, so no gate is lost.

_Codified in: the deleted `codex/rules/` tree; citation re-points in `targets/omnia/prose/{prompts/build.md,prompts/build/review.md,references/review-categories.md,references/team-protocol-crate.md,references/review-output-template.md,references/README.md}` and `targets/vectis/prose/{prompts/build/{core,ios,android}/review.md,references/review/universal-checks.md}`. See DECISIONS.md §"Codex ownership flip: shared packs embed in the binary" in `augentic/specify` (RFC-66 — landed; removed from that tree)._
