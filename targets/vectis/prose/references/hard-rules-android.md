# Android-Writer Rules and Important Notes

**When to read this**: open this file at the start of every Android shell run, and again before final verification. It captures the Update Mode preservation contract plus the normative facts about the platform — BoltFFI bridging, generated-type packages, Java tooling, network security config, defensive error handling, and crash recovery — that are easy to violate by hand-editing template-owned DX.

## Scaffold immutability (create and update mode)

1. **Keep DX aligned with `$TEMPLATE_DIR`.** `Android/Makefile`, `Android/settings.gradle.kts`, `Android/build.gradle.kts`, `Android/app/build.gradle.kts`, and `Android/shared/build.gradle.kts` come from the local `vectis-exemplar` checkout after identity substitution. On drift, re-copy from `$TEMPLATE_DIR` — agents must not invent pins during repair or feature work.
2. **Gradle wrapper lands from the template.** Materialize copies `gradlew` and `gradle/wrapper/` — do not invent a wrapper pin. Host SDK paths stay in `local.properties` (denylisted from materialize; operator/host owned).

## Preservation Rules (Update Mode)

1. **Never regenerate a file from scratch.** Make targeted edits.
2. **Preserve custom styling** that the developer added beyond the Material 3 defaults.
3. **Preserve custom composable logic** (e.g., animations, gestures) that is not driven by the ViewModel.
4. **Preserve `@Preview` blocks** on unchanged composables.
5. **Preserve Gradle customizations** (signing, flavors, custom build phases).
6. **Preserve `Makefile` customizations** (additional targets, environment variables) that do not rewrite BoltFFI pack recipes or pin files.

## Important Notes

- **Core must exist first**: This skill generates the Android shell for an existing Crux core. Run the core-writer skill first to generate the `shared` crate.
- **Shell is thin**: All business logic lives in the Rust core. The shell only renders composables and performs platform I/O. Never add business logic to Kotlin code.
- **BoltFFI bridging**: The shared crate packs via `boltffi pack android` (Makefile `package` target). Generated Kotlin / JNI libs land under `Android/generated/` and are consumed by the `:shared` Gradle module. There is no UniFFI library-override `Application` class in the live template — `Core` constructs `CoreFfi` from the BoltFFI-generated package (`{android_package}.shared` / app types under `{android_package}`).
- **Generated types**: Hand-written Kotlin under `{android_package}` must import BoltFFI / facet-generated types explicitly from the package identity in `shared/boltffi.toml` after materialize substitution. Unresolved references usually mean a missing import or a stale `make build` (typegen + pack).
- **Two Core patterns**: Simple apps (Render-only) and complex apps (with HTTP/SSE) both follow the template's `Core` + `StateFlow` shape; keep effect handlers thin.
- **Gradle wrapper is required**: The `gradlew` script must exist before any `./gradlew` command works. It lands from `$TEMPLATE_DIR` at materialize time — do not invent a wrapper pin.
- **Java compatibility**: Follow the template's `compileOptions` / Kotlin JVM target. When the host's default Java breaks AGP/Kotlin, pin a compatible JDK via `org.gradle.java.home` in `gradle.properties` (host-local; not a template pin invent).
- **Network security config**: Android 9+ blocks cleartext HTTP traffic by default. Apps with HTTP or SSE effects MUST include a `network_security_config.xml` to allow cleartext to localhost/`10.0.2.2` for development. Without it, the app crashes on first network request.
- **Defensive error handling**: Core FFI calls throw with a meaningful Rust-side error message. Always use `try/catch` with `Log.e(TAG, "context: ${e.message}", e)` so the diagnostic is visible in logcat. The `Effect.Render` handler must preserve the existing view on failure. All async effect handlers (SSE, Time) that run in `scope.launch` blocks MUST wrap their bodies in `try/catch`. Always rethrow `CancellationException`.
- **themes.xml is mandatory**: `AndroidManifest.xml` references a theme resource. The `res/values/themes.xml` file MUST exist or the build fails.
- **No Android Studio required for builds**: The Gradle wrapper (`./gradlew`) handles compilation. Android Studio is only needed for initial SDK/NDK installation or for the visual layout editor.
- **Hot reloading**: Jetpack Compose's built-in Live Edit and `@Preview` composables provide the development-time iteration equivalent. Every screen composable should include a `@Preview` with sample data (checked by AND-008).
- **Crash recovery handler**: Prefer persisting crash info and restarting the Activity instead of letting the app terminate — Crux cores re-render from state. See references/crux-android-shell-pattern.md for the pattern when present in the template.
- **Emery integration**: When `slice-dir` is provided, the skill reads the `## Android Shell Requirements` section from the feature spec and the `## Android Shell Details` section from design.md. The primary input remains `app.rs` from the core; the feature spec's platform section supplements with requirements that may not be expressed in the Rust types alone.
- **Verify stamp**: The engine-dispatched `verify` operation runs the checks; after a clean `make build` it writes `Android/.vectis/verify.ok` (adapter stamp; not template DX — verify may report workspace writes under `written`). `android-repair` sub-agents return Kotlin edits only and never write the stamp.
