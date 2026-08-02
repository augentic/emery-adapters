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
- **Never** edit `emery-adapters`, `vectis-exemplar`, or the built guest component in-band — even when those repos are sibling checkouts.
- Pin and DX drift is fixed by re-copying from `$TEMPLATE_DIR` (or fixing the template repo in a maintainer session) — never by inventing versions in the consumer tree. Detection heuristics and the pin-diff checklist: [`template-capabilities.md`](../references/template-capabilities.md) § Template / version-pin drift handling (fetch via MCP on any pin-suspect failure).

## Standard arguments

All phase prompts assume these symbols are resolved by the leg's orchestrating agent before any sub-agent fan-out:

| Symbol | Meaning |
| --- | --- |
| `SLICE_ID` | The active slice name (`emery plan advance` output, or `emery slice` argument). |
| `SLICE_DIR` | `.emery/slices/<SLICE_ID>/`. |
| `DOMAIN_NAME` | The single domain spec folder under `SLICE_DIR/specs/`. When the slice carries multiple domains, iterate the per-domain phase prompts in declaration order. |
| `PROJECT_DIR` | The target project root (single-repo mode) or the resolved workspace slot (workspace mode). |
| `TEMPLATE_DIR` | Local [`vectis-exemplar`](https://github.com/augentic/vectis-exemplar) checkout. Default `${PROJECT_DIR}/../vectis-exemplar`; override with `VECTIS_EXEMPLAR_DIR`. Required for greenfield materialize and pin refresh. |
| `IOS_SHELL_DIR` | `${PROJECT_DIR}/iOS` (only when `ios` is in scope). |
| `ANDROID_SHELL_DIR` | `${PROJECT_DIR}/Android` (only when `android` is in scope). |
| `APP_NAME` | The Xcode target / Swift source folder name (derived from `design.md`'s `App` struct name). |
| `ANDROID_PACKAGE` | Android application id. Prefer the package declared in `design.md` (or the existing `Android/app/build.gradle.kts` applicationId). Fallback only: `com.vectis.<lowercase APP_NAME>` — do not keep the template's `io.augentic.vectisapp` unless that is the product id. Writers, reviewers, and imports MUST use this resolved value (dot form + slash form under `app/src/main/java/`) — never hardcode `com.vectis.*` or the template default when `ANDROID_PACKAGE` differs. |
| `CATALOG_PATH` | `${PROJECT_DIR}/.emery/design-system/components.yaml` when present. Optional — absent means no component factoring. |

## Platform scope

Every slice carries the full app platform set from `project.yaml.platforms` (stamped verbatim into `proposal.md ## Platforms` by synthesis). Each slice signifies core + all declared shell work; build determines the **actual per-platform work**:

- **create** — a declared tree is absent on disk → the agent materializes from `$TEMPLATE_DIR` on the **host** filesystem before write legs (the guest cannot see a sibling checkout). Follow the full procedure in [`template-materialize.md`](../references/template-materialize.md) (fetch via MCP); the template-materialize prelude in each leg names which trees are absent. There is no separate plan-time bootstrap slice; `project.yaml.platforms` already declares the intent. Only `core`, `ios`, and `android` are materialized today (`web/` in the template is out of scope). Fail closed when `$TEMPLATE_DIR` is missing — do not invent a scaffold.
- **update** — the shell tree exists → diff core types against existing code and apply targeted edits (the normal feature-slice path). Late capability adoption also follows [`template-materialize.md`](../references/template-materialize.md).
- **no-op** — the platform is in scope but the slice introduces no changes for that shell (answer the leg with `applicable: false`).

Valid Vectis platform tokens are `core`, `ios`, `android`, `web`, and `desktop`. Only `core`, `ios`, and `android` have build prompts today; the adapter core silently skips `web` and `desktop` in the platform set (no shell leg to run). Token / asset / layout work is **input context**, never a platform.

The adapter core processes platforms in dependency order: `core` first (the shells depend on it), then the declared `ios` / `android` shell legs — independent of each other, but run serially because their verify halves share the same Cargo workspace lock. When the platform set contains `core` only, the core skips the shell legs wholesale; this is a backend-only build.

## Phase order

Leg order is owned by the adapter core, not by this document: the core runs its deterministic prepare prelude, then the **composition** leg (Step 0.5 + Phase 1, gated by the in-guest composition validator with a bounded repair loop), the **core** leg (Phases 2–3 — mid-build verify-repair; no durable stamp), one **shell** leg per declared shell platform (Phases 4–5), the **review** leg (Phases 6–7, ending with the core review prompt's `## § Consolidate review findings`), the **final-core-verify** leg (Step 6 again + `shared/.vectis/verify.ok` digest stamp), and finally the **report** leg (Phases 8–9), bracketed by the deterministic postlude gates. Each leg's system prompt carries this document plus the leg's phase prompt: [`build/composition.md`](build/composition.md) (Step 0.5 + Phase 1), [`build/core/write.md`](build/core/write.md) + [`build/test.md`](build/test.md) (Phases 2–3), [`build/ios/write.md`](build/ios/write.md) / [`build/android/write.md`](build/android/write.md) (Phases 4–5, when in scope), [`build/core/review.md`](build/core/review.md) plus the in-scope shell review prompts (Phases 6–7), [`build/test.md`](build/test.md) again for final-core-verify (Step 6 + stamp only), and [`build/report.md`](build/report.md) (Phases 8–9: the shell verify gate, the phase outcome contract, and the build-report shape).

**Step 0.5 — component inference** runs in the composition leg, ahead of composition regeneration: the adapter injects the in-guest deterministic cluster report; the leg names unbound clusters by judgement and writes the bindings file. The full contract lives in [`build/composition.md`](build/composition.md) § Step 0.5.

**Final core verify (between review and report).** After review consolidation, the final-core-verify leg re-runs Step 6 of [`build/test.md`](build/test.md) against the tree about to be reported and, on success, writes the `shared/.vectis/verify.ok` digest stamp; the mid-build core loop must not write that stamp. An exhausted repair budget fails the build before report.

## § Sub-agent delegation

Each writer / reviewer phase prompt runs in its **own sub-agent** with a clean context window; iOS and Android shell verify run from the leg's orchestrator, not verify sub-agents. Before any fan-out, fetch the full delegation contract — sub-agent task names, inputs/outputs, allowed/forbidden path scopes, and why verify is serial while review is parallel — from [`sub-agent-contract.md`](../references/sub-agent-contract.md) via the granted MCP references.
