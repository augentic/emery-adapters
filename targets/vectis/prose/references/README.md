# Vectis reference material

Reference documentation for the Vectis target adapter at [`adapters/targets/vectis/`](..). In Emery, Vectis is a **target adapter** — `guidance`, `build`, `merge` — not a slash-command plugin.

The Vectis core / test / iOS / Android writer and reviewer orchestration lives in [`../prompts/build.md`](../prompts/build.md) and eight phase prompts under [`../prompts/build/`](../prompts/build/). The depth (Crux idioms, SwiftUI patterns, Compose patterns, hard rules, review check libraries, token templates, design-system integration) lives in this folder. The live `$TEMPLATE_DIR` ([`vectis-exemplar`](https://github.com/augentic/vectis-exemplar)) checkout is the worked example for core + shells + DX; sample Emery artifacts sit under [`examples/`](examples/README.md).

## Prompts

| Prompt | Purpose |
|--------|---------|
| [`guidance.md`](../prompts/guidance.md) | Idiom guidance for core synthesis. |
| [`build.md`](../prompts/build.md) | Parent build prompt: phase vocabulary, sub-agent contract, verify-serial / review-parallel rule, consolidation, template-drift signal, phase outcome. |
| [`build/composition.md`](../prompts/build/composition.md) | Regenerate `composition.yaml` from `spec.md` + `design.md`; gated by the deterministic validator. |
| [`build/core/write.md`](../prompts/build/core/write.md) | Generate / update the Crux shared core. |
| [`build/core/review.md`](../prompts/build/core/review.md) | Agent-team review of the Rust `shared` crate. |
| [`build/test.md`](../prompts/build/test.md) | Generate / update Crux tests; run the core verify-repair loop. |
| [`build/ios/write.md`](../prompts/build/ios/write.md) | Generate / update the SwiftUI iOS shell + verify. |
| [`build/ios/review.md`](../prompts/build/ios/review.md) | Agent-team review of the iOS shell. |
| [`build/android/write.md`](../prompts/build/android/write.md) | Generate / update the Compose Android shell + verify. |
| [`build/android/review.md`](../prompts/build/android/review.md) | Agent-team review of the Android shell. |
| [`merge.md`](../prompts/merge.md) | Merge-leg gates around the delta fold. |

## References

### Template bootstrap

- [`template-capabilities.md`](template-capabilities.md) — DX completeness after materialize; late-capability re-adoption from `$TEMPLATE_DIR` (strip grammar stays in `$TEMPLATE_DIR/AGENTS.md`).

### Runtime schemas

- [`schemas.md`](schemas.md) — tool-owned JSON Schemas (`tokens`, `assets`, `composition`) and how to retrieve their bodies.

### Hard rules

- [`hard-rules-core.md`](hard-rules-core.md) — Crux core hard rules.
- [`hard-rules-ios.md`](hard-rules-ios.md) — iOS shell hard rules (scaffold immutability, Makefile ownership).
- [`hard-rules-android.md`](hard-rules-android.md) — Android shell hard rules.

### Crux core depth

- [`crux/app-pattern.md`](crux/app-pattern.md) — `App` trait, `update()` / `view()`, `Model` / `Event` / `Effect` shapes.
- [`crux/capabilities.md`](crux/capabilities.md) — built-in capabilities (HTTP, KV, Time, Render).
- [`crux/command-api.md`](crux/command-api.md) — `Command<Effect, Event>` builder methods.
- [`crux/custom-capabilities.md`](crux/custom-capabilities.md) — when and how to author a custom effect.
- [`crux/testing-patterns.md`](crux/testing-patterns.md) — synchronous test API, `expect_*` chains, `resolve()`.
- [`crux/artifact-to-code-mapping.md`](crux/artifact-to-code-mapping.md) — `spec.md` + `design.md` → Rust types / methods.
- [`crux/update-change-patterns.md`](crux/update-change-patterns.md) — diff-driven editing patterns.
- [`crux/generated-type-conventions.md`](crux/generated-type-conventions.md) — `#[repr(C)]`, `#[derive(Facet)]`, kebab/PascalCase rules.

### iOS shell depth

- [`ios/shell-pattern.md`](ios/shell-pattern.md) — Core.swift / ContentView.swift anatomy.
- [`ios/view-patterns.md`](ios/view-patterns.md) — SwiftUI view patterns and hazards.
- [`ios/token-templates.md`](ios/token-templates.md) — Swift theme code derived from `tokens.yaml`.
- [`ios/design-system-integration.md`](ios/design-system-integration.md) — Theme + Assets.xcassets integration.

### Android shell depth

- [`android/shell-pattern.md`](android/shell-pattern.md) — Core.kt / Application.kt / root composable anatomy.
- [`android/view-patterns.md`](android/view-patterns.md) — Compose view patterns and hazards.
- [`android/token-templates.md`](android/token-templates.md) — Kotlin theme code derived from `tokens.yaml`.
- [`android/design-system-integration.md`](android/design-system-integration.md) — Theme + drawable integration.

### Test writer depth

- [`test-runbook.md`](test-runbook.md) — operational runbook for create / update / repair modes.
- [`test-spec-mapping.md`](test-spec-mapping.md) — scenario → test function mapping rules.

### Review depth

- [`agent-teams.md`](agent-teams.md) — shared specialists + antagonist + lead synthesis pattern.
- [`review/team-protocol-core.md`](review/team-protocol-core.md), [`review/team-protocol-ios.md`](review/team-protocol-ios.md), [`review/team-protocol-android.md`](review/team-protocol-android.md) — per-platform team-spawn prompts.
- [`review/crux-checks.md`](review/crux-checks.md), [`review/logic-checks.md`](review/logic-checks.md), [`review/general-checks.md`](review/general-checks.md), [`review/ios-checks.md`](review/ios-checks.md), [`review/swift-quality-checks.md`](review/swift-quality-checks.md), [`review/android-checks.md`](review/android-checks.md), [`review/kotlin-quality-checks.md`](review/kotlin-quality-checks.md), [`review/universal-checks.md`](review/universal-checks.md) — check libraries.
- [`review/iteration-report.md`](review/iteration-report.md) — iteration-report template and finding-ID conventions.

### Asset materialization (implemented)

Canonical SVG → per-platform exports (the adapter's materialize step), render-by-`kind` shell writers, build-time `plan-bootstrap-app-icon-missing` gate (the adapter's bootstrap app-icon verify at build prepare; a shell-resident launcher icon satisfies it). Inference-only symbol fallback; see [`VECTIS-006`](../rules/VECTIS-006-asset-render-by-kind.md), [`ios/design-system-integration.md`](ios/design-system-integration.md), and [`android/design-system-integration.md`](android/design-system-integration.md).

### Layout inferer contract

- [`layout-inferer-contract.md`](layout-inferer-contract.md) — the inferer contract consumed by the [`sources/screenshots/`](../../../../sources/screenshots/) adapter.

### Sample artifacts + template pointer

- [`examples/`](examples/README.md) — sample `tokens.yaml` / `assets.yaml` plus the `$TEMPLATE_DIR` capability→path map (live [`vectis-exemplar`](https://github.com/augentic/vectis-exemplar) is the worked example for core, shells, DX, and strip units).
