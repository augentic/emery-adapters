---
id: VECTIS-010
title: Canonical UI Bindings
severity: important
trigger: Shell or core sources hand-write UI test ids, display strings, or error copy that should come from composition inline test_id or contract YAML and codegen.
applicability:
  adapters: [vectis]
references:
  - label: Canonical UI bindings
    path: adapters/targets/vectis/prose/references/canonical-ui-bindings.md
  - label: Maestro journey authoring
    path: adapters/targets/vectis/prose/references/maestro/journey-authoring.md
---

## Rule

Agents MUST:

1. Add or update **test ids** as inline `test_id: <kebab-case>` on composition items/groups (composition leg) — not in shell sources or `component-bindings.yaml`.
2. Add or update **strings and errors** in `contract/{ui-strings,ui-errors}.yaml` when those files exist (not in generated or shell sources).
3. Run `cargo make generate-bindings` before wiring new test tags, copy, or error messages.
4. Run **`cargo make generate-bindings`** after contract/composition or shell/core edits. The Vectis adapter projects composition `test_id` values into **`.vectis/generated/test-ids.yaml`** during `emery build` (after composition gate). The Vectis adapter **deterministic in-guest verify** enforces canonical UI rules at build time (`canonical-ui-literal-hardcoded`, `canonical-test-id-raw`, `canonical-test-tag-resource-id`, `canonical-test-id-projection-stale`, `canonical-test-id-duplicated`, `canonical-test-id-contract-forbidden`, `canonical-seed-version`) — build cannot succeed until shells consume generated constants and expose Maestro `id:` selectors on Android. Seed shape is validated by a core serde deserialize test on `SEED_YAML`, not by the adapter.
5. Consume generated `MaestroTestIds.*`, `UiStrings.*`, `UiErrors.*`, `crate::ui_strings::*`, and `crate::ui_errors::*` — never duplicate the same literal in shell Kotlin/Swift or Rust core. On Android, keep `testTagsAsResourceId = true` on the root `Surface` in `ContentView`.

Optional `contract/test-ids.yaml` is a template demo seed only; product apps author test ids in composition.

## Look For

- `Text("Save")` / `stringResource(R.string.…)` with hand-written product copy when `UiStrings.SAVE` exists or should exist in contract.
- Hardcoded accessibility/test tags instead of `MaestroTestIds.*` or `testTag(MaestroTestIds.…)`.
- Hand-edited `shared/src/ui_strings.rs` or `shared/src/ui_errors.rs` (generated — use contract + codegen).
- Maestro YAML with raw id strings instead of `${MAESTRO_…}`.
- Confusing `component-bindings.yaml` (composition catalog) with inline `test_id` or `contract/test-ids.yaml`.

## Fix

Add composition `test_id` and/or contract keys → `cargo make generate-bindings` → wire shell/core with generated constants → update Maestro YAML to use env vars from `load-*.sh`. Build verify enforces bindings.
