# Vectis build — iOS shell (write + verify)

Inlined by the adapter core into the iOS shell leg's system prompt (alongside [../../build.md](../../build.md)) when `ios` is in the platform set (carried from `project.yaml.platforms` via `proposal.md ## Platforms`). The composition validation gate ([../composition.md](../composition.md)) MUST have passed first.

The SwiftUI patterns, Crux iOS shell anatomy, token templates, and design-system integration depth live in [`../../../references/ios/`](../../../references/ios/).

## Mode detection

Inspect `${IOS_SHELL_DIR}` for any `.swift` files:

- No Swift files → **create mode**: the adapter scaffolds the iOS shell deterministically from its embedded templates before this leg (see the scaffold prelude in the leg's prompt), then enter update mode. Do not create Swift files before the scaffold exists — the rendered scaffold must be the first write to `iOS/`.
- Swift files present → **update mode**: diff core types against existing Swift code and apply targeted edits.

Spawn the writer sub-agent with `mode: create|update` and `skip_verification: true`; the orchestrator runs the verify loop (§ Verify) after the writer returns.

Repair sub-agent (`task: ios-verify-repair`, invoked by the verify loop below) applies the minimum **structural** change to fix reported Swift / Xcode errors — never add or preserve `swiftlint:disable`, `swift-format-ignore`, or other lint/format suppression comments; refactor so `SWIFT_TREAT_WARNINGS_AS_ERRORS` passes cleanly.

## Writer steps

1. **Read the input contract.** `app.rs`, `lib.rs`, `Cargo.toml`, the regenerated `composition.yaml`, sibling `tokens.yaml` / `assets.yaml` when present, and the `## iOS Shell Requirements` section of `spec.md` plus the `## iOS Shell Details` section of `design.md`.
2. **Diff core and UI artifacts.** Classify changes to `Effect`s, ViewModel variants, per-page view-struct fields, `Event`s, `Route`s, token categories, assets, components, and any `VectisDesign` references (the latter are forbidden — remove on sight).
3. **Apply core / view updates.** Edit `Core.swift` (the Crux bridge — effect handlers, serialization protocol), `ContentView.swift` (root branching on the `ViewModel` enum), per-screen views under `iOS/<APP_NAME>/Views/`, navigation wiring, Inject hot-reload boilerplate, and build config with targeted changes only. Patterns: [`ios/shell-pattern.md`](../../../references/ios/shell-pattern.md), [`ios/view-patterns.md`](../../../references/ios/view-patterns.md).
4. **Refresh generated UI surfaces.** Regenerate shell-local `iOS/<APP_NAME>/Theme/` (theme code derived from `tokens.yaml`, HIG fallback when `tokens.yaml` is absent — full templates: [`ios/token-templates.md`](../../../references/ios/token-templates.md)), `iOS/<APP_NAME>/Components/` (one named SwiftUI view per `component: <slug>` directive in `composition.yaml`, PascalCased — `task-row` → `TaskRowView`), and `iOS/<APP_NAME>/Resources/Assets.xcassets/`. Preserve operator-owned files. Design-system integration depth: [`ios/design-system-integration.md`](../../../references/ios/design-system-integration.md).
   - **Materialize gate.** The adapter's deterministic build prelude materializes in-scope ids with missing `sources.ios` pins; after editing canonical masters under `design-system/assets/`, operators re-materialize by re-running the slice build. Committed trees under `design-system/assets/exports/ios/` are the copy source — never read canonical `source:` SVG/PNG at write time.
   - **Copy from `exports/ios/`.** Resolve the effective `assets.yaml` (slice-local `.specify/slices/<name>/assets.yaml` when present, else `design-system/assets.yaml`). For each composition-referenced asset id:
     - `kind: symbol` — emit `Image(systemName: symbols.ios)` at the call site; **no** imageset copy.
     - `role: app-icon` — copy `assets/exports/ios/app-icon/AppIcon.appiconset/` into `Resources/Assets.xcassets/AppIcon.appiconset/`.
     - `role: icon` or `decorative` + `kind: vector` — copy `assets/exports/ios/<id>.imageset/` (PDF + `Contents.json`) into `Resources/Assets.xcassets/<id>.imageset/`.
     - `role: illustration` + `kind: vector` — copy `assets/exports/ios/<id>.imageset/` (`@2x` / `@3x` PNG + `Contents.json`).
     - `kind: raster` (operator-pinned per-density masters) — copy pinned `{1x,2x,3x}` files into `<id>.imageset/` per `sources.ios`.
   - **Render by `kind`.** `vector` / `raster` ids emit `Image("<id>")` from the copied imageset; never substitute SF Symbols for non-`symbol` entries. When `CATALOG_PATH` exists, every `confirmed` catalog entry referenced by a `component:` directive in `composition.yaml` produces a `<Slug>View.swift` file under `iOS/<APP_NAME>/Components/`. Per-screen views reference the shared component view instead of inlining the layout. Props are derived from the variation across instances of the same `component:` slug in `composition.yaml`. When the catalog is absent, component files are still emitted for any `component:` directives that appear in `composition.yaml` (backward-compatible behaviour), but the catalog is the authoritative driver for which slugs to factor. **Retroactive factoring (B7).** When a component newly `confirmed` this build is referenced by baseline screens *outside the current slice's domains* (composition.md Step 6a attached the `component: <slug>` directive to those prior screens via `delta.modified`), generate `Components/<Slug>View.swift` **and** refactor those prior screens' generated views to consume the shared component in place of the inlined layout — the writer runs in `update` mode against the live shell tree, so editing prior-slice views is in scope. The refactor is behaviour-preserving because the skeletons are structurally identical by construction; the verify loop below catches any regression. Idempotent: a prior screen already consuming the shared view needs no further edit.
5. **Enforce shell boundaries.** Keep all business logic in the Rust core; the shell only renders views and performs platform I/O. Remove any `import VectisDesign` — there is no shared Swift Package; the writer emits shell-local theme + asset code exclusively.
6. **SwiftUI hazards to avoid.** Never place `TextField` or a small `Button` inside a `ScrollView` within a `NavigationStack` — the `UIScrollView` touch-delay mechanism suppresses taps. Always include `#Preview` blocks for new screens to keep Xcode previews working.

## Hard rules

Full set at [`hard-rules-ios.md`](../../../references/hard-rules-ios.md). Highlights:

- Create mode relies on the adapter's deterministic scaffold landing before any Swift files exist under `iOS/`.
- Never edit `iOS/Makefile`, `iOS/project.yml`, `iOS/.vectis/sim-build.sh`, or `iOS/.vectis/sim-dev.sh` — the adapter auto-syncs them from the embedded template around each write leg.
- Never substitute a named simulator destination (`name=iPhone …`); `sim-build` uses `generic/platform=iOS Simulator` via the CLI-owned script only.
- Zero-warning policy: fix structure, never suppress — no `swiftlint:disable`, `swift-format-ignore`, or similar in shell Swift (`iOS/<APP_NAME>/**/*.swift` excluding `generated/`). Generated `Shared` / `SharedTypes` SPM packages relax warnings via `relax-generated-spm-packages.sh`.

## Verify (max 3 iterations)

The shell leg's **orchestrating agent** runs the verify loop — not a sub-agent with shell access. The orchestrator is the **sole source of truth** for iOS shell checkboxes in `tasks.md`; never mark an iOS task complete or report success unless all three commands below have actually run and passed in the same iteration.

After the writer sub-agent returns (the adapter has already re-rendered the agent-immutable scaffold files deterministically), the orchestrator executes this loop (max 3 iterations):

```bash
swiftformat "${IOS_SHELL_DIR}/${APP_NAME}/"                    # 1. Format.
cd "$IOS_SHELL_DIR" && make build                              # 2. Build (typegen + package + xcodegen).
cd "$IOS_SHELL_DIR" && make sim-build                          # 3. Simulator build (delegates to .vectis/sim-build.sh).
```

On failure the orchestrator captures stderr and spawns a **repair-only** sub-agent (`task: ios-verify-repair`) with:

- `forbidden_paths: [iOS/Makefile, iOS/project.yml, iOS/.vectis/sim-build.sh, iOS/.vectis/sim-dev.sh]`
- `allowed_paths: iOS/<APP_NAME>/**/*.swift, Theme/, Components/, Resources/`
- `error_output:` the captured stderr from the failing step
- **No shell** — the sub-agent returns edited Swift files or a patch plan only; the orchestrator applies edits and re-runs the loop from step 1.

**Structural fix only for warnings** — refactor (underscore-prefixed unused parameters, real handler wiring, visibility adjustments) until `make build` / `make sim-build` pass; never silence a warning with `swiftlint:disable`, `swift-format-ignore`, or similar comments.

**Destination / simulator-not-found errors:** the scaffold files are adapter-synced — retry the same commands. Never edit Makefile, `project.yml`, `sim-build.sh`, or `sim-dev.sh`. Never run `xcodebuild` with a named device destination. If generic destination still fails after a retry, escalate — do not substitute `name=iPhone …`.

Operators may use `make run` / `make sim-run` for local desk checks; the orchestrator verify loop still uses only `make build` + `make sim-build`.

If still failing after 3 iterations: **stop**, report the remaining failures with full error output, and escalate.

If `make build` fails on `shared.swift` with `cannot find type 'RustBuffer'` or `ffi_shared_uniffi_contract_version`, the operator likely has cargo-swift 0.9 without a synced iOS scaffold — re-run the slice build so the adapter re-syncs the scaffold, then `make clean` and rebuild. This is a compile-time scaffold drift symptom, distinct from a runtime `UniFFI contract version mismatch` (which indicates an incompatible cargo-swift pin; see [../../build.md](../../build.md) § Template / version-pin drift handling).

## Worked examples

- [`examples/ios/01-simple-counter.md`](../../../references/examples/ios/01-simple-counter.md) — minimal Core.swift + ContentView.swift.
- [`examples/ios/02-http-counter.md`](../../../references/examples/ios/02-http-counter.md) — HTTP capability bridging, async / await effect handling.
