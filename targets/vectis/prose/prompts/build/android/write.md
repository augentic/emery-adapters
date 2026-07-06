# Vectis build — Android shell (write + verify)

Inlined by the adapter core into the Android shell leg's system prompt (alongside [../../build.md](../../build.md)) when `android` is in the platform set (carried from `project.yaml.platforms` via `proposal.md ## Platforms`). The composition validation gate ([../composition.md](../composition.md)) MUST have passed first.

Compose patterns, Crux Android shell anatomy, Kotlin token templates, and design-system integration depth live in [`../../../references/android/`](../../../references/android/).

## Mode detection

Inspect `${ANDROID_SHELL_DIR}/app/src/main/java/<package>/Core.kt`:

- Missing → **create mode**: the adapter scaffolds the Android shell deterministically from its embedded templates before this leg (see the scaffold prelude in the leg's prompt, which also installs the vendored Gradle wrapper); enter pre-flight (see § Verify below), then update mode.
- Present → **update mode**: diff core types against existing Kotlin code and apply targeted edits.

Spawn the writer sub-agent with `mode: create|update` and `skip_verification: true`; the orchestrator runs the verify loop (§ Verify) after the writer returns.

Repair sub-agent (`task: android-verify-repair`, invoked by the verify loop below) applies the minimum **structural** change to fix reported Kotlin / Gradle errors — never add or preserve `@Suppress` / `@file:Suppress`; refactor so `allWarningsAsErrors` and `JavaCompile -Werror` pass cleanly.

## Writer steps

1. **Read inputs.** `app.rs`, the regenerated `composition.yaml`, sibling `tokens.yaml` / `assets.yaml` when present, the `## Android Shell Requirements` section of `spec.md`, and the `## Android Shell Details` section of `design.md`. Extract App name, ViewModel / Effect / Event / Route variants, and the capability set.
2. **Build an inventory** of existing Kotlin code: effect handlers, ViewModel cases, screen composables, event dispatches, adapter clients (Ktor for HTTP / SSE, SharedPreferences for KV), DI modules (Koin when multiple non-Render effects are used).
3. **Diff Rust core types vs Kotlin inventory** by category (Effect → ViewModel → view-fields → Event → Route) and emit a summary edit plan.
4. **Apply changes.** Expand or strip CAP blocks in `Core.kt`, `AndroidManifest.xml`, and Gradle build files. Add or remove screen composables for each ViewModel variant under `Android/app/src/main/java/com/vectis/<app>/ui/screens/`. Update the root `when` over the `ViewModel` enum. Dispatch new `Event`s through `Core.update(...)`. Emit one named composable per `component: <slug>` directive in `composition.yaml` under `Android/app/src/main/java/com/vectis/<app>/ui/components/` (PascalCased — `tab-bar` → `TabBarComponent.kt`), with props inferred from variation across instances. When `CATALOG_PATH` exists, every `confirmed` catalog entry referenced by a `component:` directive in `composition.yaml` produces a shared composable file. Per-screen composables reference the shared component instead of inlining the layout. When the catalog is absent, component files are still emitted for any `component:` directives in `composition.yaml` (backward-compatible behaviour), but the catalog is the authoritative driver for which slugs to factor. **Retroactive factoring (B7).** When a component newly `confirmed` this build is referenced by baseline screens *outside the current slice's domains* (composition.md Step 6a attached the `component: <slug>` directive to those prior screens via `delta.modified`), generate the shared `<Slug>Component.kt` **and** refactor those prior screens' generated composables to consume it in place of the inlined layout — the writer runs in `update` mode against the live shell tree, so editing prior-slice composables is in scope. The refactor is behaviour-preserving because the skeletons are structurally identical by construction; the verify loop below catches any regression. Idempotent: a prior screen already consuming the shared composable needs no further edit. Patterns: [`android/shell-pattern.md`](../../../references/android/shell-pattern.md), [`android/view-patterns.md`](../../../references/android/view-patterns.md).
5. **Refresh generated UI surfaces.** Regenerate shell-local theme code under `Android/app/src/main/java/com/vectis/<app>/ui/theme/` (Material 3 fallback when `tokens.yaml` is absent — full templates: [`android/token-templates.md`](../../../references/android/token-templates.md)), and drawable / mipmap resources under `Android/app/src/main/res/`. Design-system integration depth: [`android/design-system-integration.md`](../../../references/android/design-system-integration.md).
   - **Materialize gate.** The adapter's deterministic build prelude materializes in-scope ids with missing `sources.android` pins; after editing canonical masters under `design-system/assets/`, operators re-materialize by re-running the slice build. Committed trees under `design-system/assets/exports/android/` are the copy source — never read canonical `source:` SVG/PNG at write time.
   - **Copy from `exports/android/`.** Resolve the effective `assets.yaml` (slice-local `.specify/slices/<name>/assets.yaml` when present, else `design-system/assets.yaml`). For each composition-referenced asset id:
     - `kind: symbol` — emit `Icons.Default.<glyph>` (or extended icon set) at the call site; **no** `res/` copy.
     - `role: app-icon` — copy the `assets/exports/android/app-icon/` tree (`mipmap-*/`, `mipmap-anydpi-v26/`, `drawable-*/ic_launcher_foreground.png`, `values/ic_launcher_background.xml`) into matching `res/` locations.
     - `role: icon` or `decorative` + `kind: vector` — copy `assets/exports/android/drawable/<id_snake>.xml` into `res/drawable/<id_snake>.xml`.
     - `role: illustration` + `kind: vector` — copy each `assets/exports/android/drawable-<density>/<id_snake>.png` into `res/drawable-<density>/<id_snake>.png`.
     - `kind: raster` (operator-pinned per-density masters) — copy pinned bucket files into matching `res/drawable-<density>/` paths per `sources.android`.
   - **Render by `kind`.** `vector` / `raster` ids emit `painterResource(R.drawable.<id_snake>)` from copied drawables; never substitute Material Icons for non-`symbol` entries.
6. **Update build configuration** (`libs.versions.toml`, `build.gradle.kts`, manifest permissions, `network_security_config.xml`) to match the changed capability set. Remove any legacy `:vectis-design` Gradle module references — there is no shared Compose module; the writer emits shell-local theme + drawable code exclusively. Replace any stale `import com.vectis.design.*` with `import com.vectis.<app>.ui.theme.*`.
7. **UniFFI bridging contract.** The `Application` class MUST set `System.setProperty("uniffi.component.shared.libraryOverride", "shared")` before any UniFFI class is loaded — without this the app fails with `UnsatisfiedLinkError` on launch. Imports for generated FFI types follow `import com.vectis.<app>.*` (not `com.vectis.design.*`). Rethrow `CancellationException` from coroutines — never swallow it.

## Hard rules

Full set at [`hard-rules-android.md`](../../../references/hard-rules-android.md). Highlights:

- Java 21 only — Java 25+ environments hit `IllegalArgumentException` in AGP; pin `org.gradle.java.home` in `gradle.properties`.
- Always include `@Preview` blocks for new composables.
- Coroutine cancellation MUST rethrow `CancellationException`.
- Zero-warning policy: fix structure, never suppress — no `@Suppress` / `@file:Suppress` in shell Kotlin (`Android/app/src/**` only; `:shared` compiles generated UniFFI and omits `allWarningsAsErrors`).

## Verify (max 3 iterations)

The shell leg's **orchestrating agent** runs the verify loop — not a sub-agent with shell access. The orchestrator is the **sole source of truth** for Android shell checkboxes in `tasks.md`; never mark an Android task complete or report success unless `make verify` has actually run and passed in the same iteration (`make verify` runs `setup`, typegen, `:shared:cargoBuild`, and `:app:assembleDebug`).

### Pre-flight (fail fast on misconfiguration)

Before entering the loop, probe host prerequisites yourself (`ANDROID_HOME` set, Rust Android targets installed via `rustup target list --installed`, Java 21 available). If host prerequisites are missing, report **deferred** and stop — do not build into a broken host.

`local.properties`, `org.gradle.java.home`, and NDK substitution are handled by `make verify` via `make setup` (`make setup-host`); the vendored Gradle wrapper is installed deterministically by the adapter at scaffold and build-prepare time. Do not bootstrap the wrapper manually with `gradle wrapper`.

### Build loop

After the writer sub-agent returns (the adapter has already re-rendered the agent-immutable scaffold files deterministically), the orchestrator executes this loop (max 3 iterations):

```bash
cd "${ANDROID_SHELL_DIR}" && make verify                # 1. Setup, typegen, cargoBuild, assembleDebug.
```

On failure the orchestrator captures stderr and spawns a **repair-only** sub-agent (`task: android-verify-repair`) with Kotlin-only edit scope — **no shell**. The sub-agent returns edited Kotlin files or a patch plan; the orchestrator applies edits and re-runs the loop from step 1. **Structural fix only for warnings** — refactor (underscore-prefixed unused parameters, real handler wiring) until `make verify` passes; never silence a warning with `@Suppress`.

**Gradle / Makefile drift errors:** the scaffold files are adapter-synced — retry the same command. Never edit `Android/Makefile`, `Android/settings.gradle.kts`, `Android/build.gradle.kts`, `Android/app/build.gradle.kts`, or `Android/shared/build.gradle.kts`. If strict-flag drift persists after a retry, escalate — do not patch adapter-owned scaffold files by hand.

If still failing after 3 iterations: **stop**, report the remaining failures with full error output, and escalate. Java 25+ environments hit `IllegalArgumentException`; the fix is pinning `org.gradle.java.home` to Java 21 in `gradle.properties` via `make setup-host`.

## Worked examples

- [`examples/android/01-simple-counter.md`](../../../references/examples/android/01-simple-counter.md) — minimal Core.kt + Application.kt + root composable.
- [`examples/android/02-http-counter.md`](../../../references/examples/android/02-http-counter.md) — Ktor HTTP capability, coroutine scope, suspending effect handlers.
