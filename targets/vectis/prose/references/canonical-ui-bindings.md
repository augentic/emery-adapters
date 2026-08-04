# Canonical UI bindings (composition + ui-contract → codegen → shell)

Single source of truth for **UI test ids** is inline `test_id` on composition items/groups (authored in the composition leg). **Display strings** and **fixed error copy** live under `${PROJECT_DIR}/ui-contract/`. Generated artifacts are never hand-edited.

## Layout

| Source | Authoring | Generated (examples) |
|--------|-----------|----------------------|
| `composition.yaml` inline `test_id:` | Composition leg (`bind` / `event` alongside) | `ui-contract/test-ids.yaml` (adapter projection during `emery build`) → `MaestroTestIds.*` (shell), `${MAESTRO_*}` (Maestro env) |
| `ui-contract/test-ids.yaml` (exemplar demo only) | Hand-written counter demo in template checkout | Same file → codegen (product apps overwrite from composition) |
| `ui-contract/ui-strings.yaml` | `strings:` keys | `UiStrings.*` (shell), `shared/src/ui_strings.rs` (core) |
| `ui-contract/ui-errors.yaml` | `errors:` keys | `UiErrors.*` (shell), `shared/src/ui_errors.rs` (core) |
| `ui-contract/seed.yaml` | Slice-start fixture data (app-defined shape) | `shared/src/seed_data.rs` (`include_str!`) + core serde test |

Codegen entrypoint: **`shared/src/bin/codegen/`** (plugin registry). In Emery-managed product apps, the Vectis adapter harvests composition `test_id` values during `emery build` and overwrites **`ui-contract/test-ids.yaml`**; exemplar codegen reads that single file. In a bare exemplar checkout (no composition), demo ids live in the same file under a `cap=demo` block. Refresh:

| Changed | Run |
|---------|-----|
| `composition.yaml` `test_id` | `emery build` (refreshes `ui-contract/test-ids.yaml`), then `cargo make generate-bindings` |
| `ui-contract/ui-strings.yaml` / `ui-contract/ui-errors.yaml` only | `cargo make generate-bindings` |
| Crux types / Effect variants | `cargo make generate` (includes bindings) |

Generated paths (do not edit):

- `iOS/generated/App/Sources/App/MaestroTestIds.swift`, `UiStrings.swift`, `UiErrors.swift`
- `Android/generated/<package>/MaestroTestIds.kt`, `UiStrings.kt`, `UiErrors.kt`
- `shared/src/ui_strings.rs`, `shared/src/ui_errors.rs`

Run **`cargo make build-hooks`** once per machine (or after pulling `tools/cursor-guard`) so `.cursor/hooks.json` can block direct edits to generated files during **desk / IDE** sessions. Hooks load at agent session start — they do not protect the first materialize build session. **Build-time enforcement** is the deterministic in-guest verify gate only (see *Mechanical gates* below).

## Authoring rules (build agents)

1. **Test ids** — add `test_id: <kebab-case>` on interactive composition items/groups during the composition leg. Do not hand-edit `ui-contract/test-ids.yaml` in product apps — `emery build` overwrites it from composition. Run `cargo make generate-bindings` before wiring shell tags.
2. **Strings and errors** — add keys to `ui-contract/ui-strings.yaml` / `ui-contract/ui-errors.yaml` (not in generated or shell sources).
3. **Run `cargo make generate-bindings`** — never hand-write generated files.
4. **Wire shell UI** — use generated constants (`MaestroTestIds.SPLASH_CTA`, `UiStrings.SPLASH_TITLE`, …). On Android, enable **`testTagsAsResourceId = true`** on the root `Surface` in `ContentView` so Maestro `id:` selectors resolve (exemplar ships this by default).
5. **Wire Rust core** — use `crate::ui_strings::…` / `crate::ui_errors::…`; do not duplicate string literals in `app.rs` when a ui-contract key exists.
6. **Maestro YAML** — reference `${MAESTRO_…}` for test ids and `"${SPLASH_TITLE}"` for display strings (via `load-*.sh` + `-e` flags). Never copy raw id values or layout demo copy into journey files.

### Icon-only and repeated controls

Controls with no visible label (icon-only buttons/FABs) or the same verb reused across screens must get **context-specific** a11y keys and `test_id`s — never a single generic `Save` / `Delete`. Give each occurrence its own `ui-contract` string key (e.g. per-screen `SAVE_<CONTEXT>`) and its own composition `test_id`. In Maestro, tap by `test_id` / `${MAESTRO_…}`; do not assert a shared visible label.

### Mechanical gates (build only)

Canonical UI rules are enforced **only** by the Vectis adapter **deterministic in-guest verify** during `emery build`. There is no separate desk/CI shell lint — a single implementation in `verify/` avoids drift.

| Finding id | What it blocks |
|------------|----------------|
| `canonical-ui-literal-hardcoded` | Hardcoded UI contract copy in shell/core static UI APIs |
| `canonical-test-id-raw` | Raw `.testTag("…")` / `accessibilityIdentifier("…")` instead of `MaestroTestIds.*` |
| `canonical-test-tag-resource-id` | Android `testTag` without root `semantics { testTagsAsResourceId = true }` |
| `canonical-test-id-projection-stale` | `ui-contract/test-ids.yaml` out of sync with composition `test_id` harvest |
| `canonical-seed-version` | `ui-contract/seed.yaml` `version` is not `1` |

These error findings block `Refined → Built` until shell/core use generated bindings and expose Maestro `id:` selectors on Android. Seed **shape** (field names, types, required keys) is validated by the core via a serde deserialize test on `SEED_YAML` — not by the adapter. Do not store derived values (counts, aggregates) in seed; compute them in `view()`. Run **`cargo make generate-bindings`** after ui-contract or composition edits so generated constants exist before build.

`design-system/layout.yaml` describes structure and may show illustrative demo strings — it is **not** the runtime assert authority for strings. Runtime copy comes from `ui-contract/ui-strings.yaml` after codegen. Test ids authored in composition are the runtime assert authority for Maestro.

## Distinction from `component-bindings.yaml`

| Artifact | Purpose |
|----------|---------|
| `${SLICE_DIR}/build/component-bindings.yaml` | Composition component catalog (fingerprint → slug) |
| `composition.yaml` inline `test_id` | UI test ids (Maestro / accessibility SSOT) |
| `ui-contract/ui-strings.yaml`, `ui-contract/ui-errors.yaml` | Display strings and error messages |

Do not fold Maestro test ids into `component-bindings.yaml` — names collide with Vectis component catalog semantics. Use inline `test_id` on composition items instead of a separate bindings file.

## References

- `$TEMPLATE_DIR/.maestro/README.md` — operator runbook
- [`maestro/journey-authoring.md`](maestro/journey-authoring.md) — journey YAML after drain
