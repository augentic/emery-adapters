# Vectis build — iOS shell (write)

Inlined by the adapter core into the iOS shell write leg's system prompt (alongside [../../build.md](../../build.md)) when `ios` is in the platform set (carried from `project.yaml.platforms` via `proposal.md ## Platforms`). The composition validation gate ([../composition.md](../composition.md)) MUST have passed first. This is a generation-only leg: the `swiftformat` / `make build` checks, the `iOS/.vectis/verify.ok` stamp, and any repair pass belong to the engine-dispatched `verify` and `repair` operations.

The SwiftUI patterns, Crux iOS shell anatomy, token templates, and design-system integration depth live in [`../../../references/ios/`](../../../references/ios/).

## Mode detection

Inspect `${IOS_SHELL_DIR}` for any `.swift` files:

- No Swift files → **create mode**: materialize from `$TEMPLATE_DIR` per [template-materialize.md](../../../references/template-materialize.md) (see the template-materialize prelude), strip unused `VECTIS-OPTIONAL` caps, then `make -C iOS generate-project` / `xcodegen` before writing feature Swift. Fail closed if `$TEMPLATE_DIR` is missing — do not invent an iOS scaffold.
- Swift files present → **update mode**: diff core types against existing Swift code and apply targeted edits. When a newly enabled adapter requires a shell handler that was stripped, copy that `cap=` FILE/block from `$TEMPLATE_DIR` ([`template-capabilities.md`](../../../references/template-capabilities.md)) — do not invent handler shapes.

Spawn the writer sub-agent with `mode: create|update` and `skip_verification: true`; the leg ends when the writer returns — the engine dispatches verification separately.

## Writer steps

1. **Read the input contract.** `app.rs`, `lib.rs`, `Cargo.toml`, the regenerated `composition.yaml`, sibling `tokens.yaml` / `assets.yaml` when present, and the `## iOS Shell Requirements` section of `spec.md` plus the `## iOS Shell Details` section of `design.md`.
2. **Diff core and UI artifacts.** Classify changes to `Effect`s, ViewModel variants, per-page view-struct fields, `Event`s, `Route`s, token categories, assets, components, and any `VectisDesign` references (the latter are forbidden — remove on sight).
3. **Apply core / view updates.** Edit `Core.swift` (the Crux bridge — effect handlers, serialization protocol), `ContentView.swift` (root branching on the `ViewModel` enum), per-screen views under `iOS/<APP_NAME>/Views/`, navigation wiring, Inject hot-reload boilerplate, and build config with targeted changes only. Patterns: [`ios/shell-pattern.md`](../../../references/ios/shell-pattern.md), [`ios/view-patterns.md`](../../../references/ios/view-patterns.md).
4. **Refresh generated UI surfaces.** Regenerate shell-local `iOS/<APP_NAME>/Theme/` (theme code derived from `tokens.yaml`, HIG fallback when `tokens.yaml` is absent — full templates: [`ios/token-templates.md`](../../../references/ios/token-templates.md)), `iOS/<APP_NAME>/Components/` (one named SwiftUI view per `component: <slug>` directive in `composition.yaml`, PascalCased — `task-row` → `TaskRowView`), and `iOS/<APP_NAME>/Assets.xcassets/`. Preserve operator-owned files. Design-system integration depth: [`ios/design-system-integration.md`](../../../references/ios/design-system-integration.md).
   - **Materialize gate.** The adapter's deterministic build prelude materializes in-scope ids with missing `sources.ios` pins; after editing canonical masters under `design-system/assets/`, operators re-materialize by re-running the build phase. Committed trees under `design-system/assets/exports/ios/` are the copy source — never read canonical `source:` SVG/PNG at write time.
   - **Copy from `exports/ios/`.** Resolve the effective `assets.yaml` (slice-local `.emery/slices/<name>/assets.yaml` when present, else `design-system/assets.yaml`). For each composition-referenced asset id:
     - `kind: symbol` — emit `Image(systemName: symbols.ios)` at the call site; **no** imageset copy.
     - `role: app-icon` — copy `assets/exports/ios/app-icon/AppIcon.appiconset/` into `Assets.xcassets/AppIcon.appiconset/`.
     - `role: icon` or `decorative` + `kind: vector` — copy `assets/exports/ios/<id>.imageset/` (PDF + `Contents.json`) into `Assets.xcassets/<id>.imageset/`.
     - `role: illustration` + `kind: vector` — copy `assets/exports/ios/<id>.imageset/` (`@2x` / `@3x` PNG + `Contents.json`).
     - `kind: raster` (operator-pinned per-density masters) — copy pinned `{1x,2x,3x}` files into `<id>.imageset/` per `sources.ios`.
   - **Render by `kind`.** `vector` / `raster` ids emit `Image("<id>")` from the copied imageset; never substitute SF Symbols for non-`symbol` entries. When `CATALOG_PATH` exists, every `confirmed` catalog entry referenced by a `component:` directive in `composition.yaml` produces a `<Slug>View.swift` file under `iOS/<APP_NAME>/Components/`. Per-screen views reference the shared component view instead of inlining the layout. Props are derived from the variation across instances of the same `component:` slug in `composition.yaml`. When the catalog is absent, component files are still emitted for any `component:` directives that appear in `composition.yaml` (backward-compatible behaviour), but the catalog is the authoritative driver for which slugs to factor. **Retroactive factoring (B7).** When a component newly `confirmed` this build is referenced by baseline screens *outside the current slice's domains* (composition.md Step 6a attached the `component: <slug>` directive to those prior screens via `delta.modified`), generate `Components/<Slug>View.swift` **and** refactor those prior screens' generated views to consume the shared component in place of the inlined layout — the writer runs in `update` mode against the live shell tree, so editing prior-slice views is in scope. The refactor is behaviour-preserving because the skeletons are structurally identical by construction; the engine-dispatched verify pass catches any regression. Idempotent: a prior screen already consuming the shared view needs no further edit.
5. **Enforce shell boundaries.** Keep all business logic in the Rust core; the shell only renders views and performs platform I/O. Remove any `import VectisDesign` — there is no shared Swift Package; the writer emits shell-local theme + asset code exclusively.
   - **Canonical UI bindings.** Per [`canonical-ui-bindings.md`](../../../references/canonical-ui-bindings.md): use **`MaestroTestIds.*`** / **`UiStrings.*`** for tags and display copy — never hand-write test tags or product strings when a generated constant exists. Run **`cargo make generate-bindings`** after shell/core edits.
6. **SwiftUI hazards to avoid.** Never place `TextField` or a small `Button` inside a `ScrollView` within a `NavigationStack` — the `UIScrollView` touch-delay mechanism suppresses taps. Always include `#Preview` blocks for new screens to keep Xcode previews working.
7. **Maestro journeys.** When the regenerated `composition.yaml` declares ≥1 `test_id` (the slice has an interactive UI surface), author one `.maestro/journeys/` flow per `spec.md` scenario — targeting the composition's `test_id` / `event` / screen map — and wire each into `maestro.mobile.yaml` via `runFlow`, per [`maestro/journey-authoring.md`](../../../references/maestro/journey-authoring.md). Mobile journeys are shared by iOS + Android: extend the existing entry idempotently, never duplicate. When `composition.yaml` has no `test_id` (e.g. a core-only slice), author nothing. Do **not** run `maestro test` in the build — it runs post-drain via `cargo make maestro-ios`.

## Hard rules

Full set at [`hard-rules-ios.md`](../../../references/hard-rules-ios.md). Highlights:

- Create mode materializes from `$TEMPLATE_DIR` before feature Swift exists under `iOS/`.
- Keep `iOS/Makefile` and `iOS/project.yml` consistent with `$TEMPLATE_DIR` after identity substitution; refresh by re-copying those paths — never invent pins or destinations. Prefer regenerating the Xcode project from `project.yml` over hand-editing it.
- Never substitute a named simulator destination (`name=iPhone …`); use the template's generic/`simctl` DX.
- Zero-warning policy: fix structure, never suppress — no `swiftlint:disable`, `swift-format-ignore`, or similar in shell Swift (`iOS/<APP_NAME>/**/*.swift` excluding `generated/`).

After the writer returns, the leg answers — do not run `swiftformat`, `make build`, or mark `tasks.md` checkboxes here. Verification (the format + `make build` pass and the `iOS/.vectis/verify.ok` stamp) is the engine-dispatched `verify` operation's pass; its findings route through `repair`. Operators may use `make run-sim` for local desk checks.

## Worked example

The live `$TEMPLATE_DIR` checkout is the worked example. After materialize identity substitution, read `$TEMPLATE_DIR/iOS/` — `core.swift`, `ContentView.swift`, views, and `cap=` handlers (`http.swift`, `keyvalue.swift`, `time.swift`, `sse.swift`) per `$TEMPLATE_DIR/AGENTS.md`. DX authority stays `$TEMPLATE_DIR` (`iOS/Makefile`, `iOS/project.yml`) — never invent pins or destinations. Pattern depth: [`ios/shell-pattern.md`](../../../references/ios/shell-pattern.md), [`ios/view-patterns.md`](../../../references/ios/view-patterns.md). Sample Emery inputs: [`examples/README.md`](../../../references/examples/README.md).
