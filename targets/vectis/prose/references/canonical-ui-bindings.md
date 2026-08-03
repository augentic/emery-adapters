# Canonical UI bindings (composition + contract → codegen → shell)

Single source of truth for **UI test ids** is inline `test_id` on composition items/groups (authored in the composition leg). **Display strings** and **fixed error copy** live under `${PROJECT_DIR}/contract/`. Generated artifacts are never hand-edited.

## Layout

| Source | Authoring | Generated (examples) |
|--------|-----------|----------------------|
| `composition.yaml` inline `test_id:` | Composition leg (`bind` / `event` alongside) | `.vectis/generated/test-ids.yaml` (adapter projection during `emery build`) → `MaestroTestIds.*` (shell), `${MAESTRO_*}` (Maestro env) |
| `contract/test-ids.yaml` (optional demo overlay) | Template demo only — keep `test_ids: {}` in product apps | Unioned by exemplar codegen only when keys do not overlap canonical projection |
| `contract/ui-strings.yaml` | `strings:` keys | `UiStrings.*` (shell), `shared/src/ui_strings.rs` (core) |
| `contract/ui-errors.yaml` | `errors:` keys | `UiErrors.*` (shell), `shared/src/ui_errors.rs` (core) |
| `contract/seed.yaml` | Slice-start fixture data (app-defined shape) | `shared/src/seed_data.rs` (`include_str!`) + core serde test |

Codegen entrypoint: **`shared/src/bin/codegen/`** (plugin registry). Composition `test_id` values are harvested by the Vectis adapter during `emery build` into **`.vectis/generated/test-ids.yaml`**; exemplar codegen unions that flat file with `contract/test-ids.yaml` (demo overlay; duplicate keys fail at codegen). Emery product apps must keep `contract/test-ids.yaml` as `test_ids: {}` — the adapter verify gate enforces this (`canonical-test-id-contract-forbidden`). Refresh:

| Changed | Run |
|---------|-----|
| `composition.yaml` `test_id` | `emery build` (refreshes `.vectis/generated/test-ids.yaml`), then `cargo make generate-bindings` |
| `contract/ui-strings.yaml` / `contract/ui-errors.yaml` only | `cargo make generate-bindings` |
| Crux types / Effect variants | `cargo make generate` (includes bindings) |

Generated paths (do not edit):

- `iOS/generated/App/Sources/App/MaestroTestIds.swift`, `UiStrings.swift`, `UiErrors.swift`
- `Android/generated/<package>/MaestroTestIds.kt`, `UiStrings.kt`, `UiErrors.kt`
- `shared/src/ui_strings.rs`, `shared/src/ui_errors.rs`

Run **`cargo make build-hooks`** once per machine (or after pulling `tools/cursor-guard`) so `.cursor/hooks.json` can block direct edits to generated files during **desk / IDE** sessions. Hooks load at agent session start — they do not protect the first materialize build session. **Build-time enforcement** is the deterministic in-guest verify gate only (`canonical-ui-literal-hardcoded`, `canonical-test-id-raw`, `canonical-test-tag-resource-id`, `canonical-test-id-projection-stale`, `canonical-test-id-duplicated`, `canonical-test-id-contract-forbidden`, `canonical-seed-version`).

## Authoring rules (build agents)

1. **Test ids** — add `test_id: <kebab-case>` on interactive composition items/groups during the composition leg. Do **not** duplicate ids in `contract/test-ids.yaml` (product apps keep `test_ids: {}`). Run `emery build` to refresh `.vectis/generated/test-ids.yaml`, then `cargo make generate-bindings` before wiring shell tags.
2. **Strings and errors** — add keys to `contract/ui-strings.yaml` / `contract/ui-errors.yaml` (not in generated or shell sources).
3. **Run `cargo make generate-bindings`** — never hand-write generated files.
4. **Wire shell UI** — use generated constants (`MaestroTestIds.SPLASH_CTA`, `UiStrings.SPLASH_TITLE`, …). On Android, enable **`testTagsAsResourceId = true`** on the root `Surface` in `ContentView` so Maestro `id:` selectors resolve (exemplar ships this by default).
5. **Wire Rust core** — use `crate::ui_strings::…` / `crate::ui_errors::…`; do not duplicate string literals in `app.rs` when a contract key exists.
6. **Maestro YAML** — reference `${MAESTRO_…}` for test ids and `"${SPLASH_TITLE}"` for display strings (via `load-*.sh` + `-e` flags). Never copy raw id values or layout demo copy into journey files.

### Save FAB (todo-app design)

Screenshots show **icon-only** check FABs (no visible `"Save"` text). Use separate a11y keys — never a generic `Save`:

| Context | `contract/ui-strings.yaml` | Shell a11y |
|---------|---------------------------|------------|
| List save FAB | `SAVE_LIST: Save list` | `UiStrings.SAVE_LIST` |
| Task save FAB | `SAVE_TASK: Save task` | `UiStrings.SAVE_TASK` |

Do not assert visible `"Save"` in Maestro; tap by `test_id` / `${MAESTRO_…}`.

### Mechanical gates (build only)

Canonical UI rules are enforced **only** by the Vectis adapter **deterministic in-guest verify** during `emery build`. There is no separate desk/CI shell lint — a single implementation in `verify/` avoids drift.

| Finding id | What it blocks |
|------------|----------------|
| `canonical-ui-literal-hardcoded` | Hardcoded contract copy in shell/core static UI APIs |
| `canonical-test-id-raw` | Raw `.testTag("…")` / `accessibilityIdentifier("…")` instead of `MaestroTestIds.*` |
| `canonical-test-tag-resource-id` | Android `testTag` without root `semantics { testTagsAsResourceId = true }` |
| `canonical-seed-version` | `contract/seed.yaml` `version` is not `1` |

These error findings block `Refined → Built` until shell/core use generated bindings and expose Maestro `id:` selectors on Android. Seed **shape** (field names, types, required keys) is validated by the core via a serde deserialize test on `SEED_YAML` — not by the adapter. Do not store derived values (counts, aggregates) in seed; compute them in `view()`. Run **`cargo make generate-bindings`** after contract/composition edits so generated constants exist before build.

`design-system/layout.yaml` describes structure and may show illustrative demo strings — it is **not** the runtime assert authority for strings. Runtime copy comes from `contract/ui-strings.yaml` after codegen. Test ids authored in composition are the runtime assert authority for Maestro.

## Distinction from `component-bindings.yaml`

| Artifact | Purpose |
|----------|---------|
| `${SLICE_DIR}/build/component-bindings.yaml` | Composition component catalog (fingerprint → slug) |
| `composition.yaml` inline `test_id` | UI test ids (Maestro / accessibility SSOT) |
| `contract/ui-strings.yaml`, `contract/ui-errors.yaml` | Display strings and error messages |

Do not fold Maestro test ids into `component-bindings.yaml` — names collide with Vectis component catalog semantics. Use inline `test_id` on composition items instead of a separate bindings file.

## References

- `$TEMPLATE_DIR/.maestro/README.md` — operator runbook
- [`maestro/journey-authoring.md`](maestro/journey-authoring.md) — journey YAML after drain
- [`VECTIS-010`](../rules/VECTIS-010-canonical-ui-bindings.md) — review rule
