# Template capabilities — strip and late adoption

Greenfield trees come from a local [`vectis-exemplar`](https://github.com/augentic/vectis-exemplar) checkout (`$TEMPLATE_DIR`: default `../vectis-exemplar`, override `VECTIS_EXEMPLAR_DIR`). Pins, BoltFFI DX, and optional-capability shapes live only in that checkout — never invent versions or FFI layouts in the consumer.

**Strip grammar and marker discovery** stay in **`$TEMPLATE_DIR/AGENTS.md`** (not copied into the consumer). This reference covers DX completeness after materialize, and how a later slice **re-adopts** a capability that was stripped at greenfield.

## After materialize: DX that must remain

`vectis::scaffold::materialize` copies root + shell DX, `shared/` (including `boltffi.toml` and `shared/src/bin/codegen/`), `contract/` (Maestro test ids, UI strings, errors), `supply-chain/`, `.maestro/`, `tools/cursor-guard`, and `.cursor/hooks.json`. After stripping `VECTIS-OPTIONAL` (especially `cap=demo`), keep:

| Path | Role |
|------|------|
| Root `Makefile`, `Makefile.toml`, `Cargo.toml` / `Cargo.lock`, `rust-toolchain.toml`, `deny.toml` | Workspace DX and pins |
| `iOS/Makefile`, `iOS/project.yml` | BoltFFI pack + xcodegen DX (`make build` / `run-sim`) |
| `Android/Makefile` + Gradle wrapper + `gradle/libs.versions.toml` | BoltFFI pack + assemble DX (`make build` / `doctor`) |
| `shared/boltffi.toml`, `shared/src/bin/codegen/` | FFI + contract → shell/Rust binding codegen |
| `contract/test-ids.yaml` (optional demo seed), `contract/ui-strings.yaml`, `contract/ui-errors.yaml`, `contract/seed.yaml` | Canonical UI bindings — test ids from composition; strings/errors from contract; seed fixture (app-defined shape, core serde test) |
| `tools/cursor-guard/` | Generated-binding edit guard (`cargo make build-hooks`) |
| `.maestro/config.yaml`, `.maestro/entries/maestro.{mobile,web}.yaml`, `.maestro/scripts/run-maestro.sh`, `.maestro/scripts/load-{test-ids,strings,errors}.sh`, `.maestro/scripts/maestro-env.sh` | Maestro infra (not demo journeys) |
| `tools/cursor-guard/`, `.cursor/hooks.json` | Guard generated bindings from direct agent edits (`cargo make build-hooks`) |

Strip demo Maestro journeys (`journeys/smoke-counter/`, etc.) and `cap=demo` blocks inside `contract/*.yaml` per `$TEMPLATE_DIR/AGENTS.md` and `$TEMPLATE_DIR/.maestro/README.md`. Do **not** delete platform entries (`maestro.mobile.yaml`, `maestro.web.yaml`), `run-maestro.sh`, or the load scripts above. README demo (`cap=demo` doc) blocks may be trimmed; keep the file.

`web/` is never materialized (out of scope). Regenerate `iOS/*.xcodeproj` via `make -C iOS generate-project` after materialize.

## Late capability adoption (later slice)

When a later slice's `design.md` `## Adapters` flips a capability from No → Yes (`http` / `kv` / `time` / `sse`), **copy the strip-unit for that `cap=` from `$TEMPLATE_DIR`** — do not invent dependency lines, effect variants, or shell handler shapes.

1. Confirm `$TEMPLATE_DIR` is present (same resolve rules as greenfield). Fail closed if missing.
2. In `$TEMPLATE_DIR`, discover the unit: `rg 'VECTIS-OPTIONAL.*cap=<cap>'` (and `FILE` markers). Read each opener's `Keep if:` / `Remove if:` / `Paired:` lines.
3. For every scope the unit spans (`dep`, `core`, `shell`, `test` — skip `web/` shell files), copy the corresponding marked block or file into the consumer at the same relative path after identity substitution (`APP_NAME`, `ANDROID_PACKAGE` / package path). Prefer whole `FILE`-marked handlers over inventing stubs.
4. Wire `Effect` / `Core` dispatch / imports so the project compiles; run `cargo make generate` when `Effect` variants or `facet_typegen` deps change (per template `Paired:` guidance).
5. Diff pin files against `$TEMPLATE_DIR` (see § Template / version-pin drift handling below). On mismatch, re-copy from the template — never guess versions.
6. Do **not** re-introduce `cap=demo` for product apps.

Authoritative strip-unit map (paths use the template's `VectisApp` / `io.augentic.vectisapp` identity — rewrite to the consumer identity):

| Cap | Typical locations under `$TEMPLATE_DIR` |
|-----|----------------------------------------|
| `http` | `Cargo.toml` / `shared/Cargo.toml` deps; `shared/src/effects.rs` arms; iOS `http.swift` + `core.swift` dispatch; Android `core/HttpClient.kt` + `Core.kt` |
| `kv` | Same dep/effect pattern; iOS `keyvalue.swift` / `KeyValueStore.swift`; Android `core/KeyValueStore.kt` |
| `time` | Same dep/effect pattern; iOS `time.swift`; Android `core/TimeHandler.kt` |
| `sse` | `shared/src/effects/sse.rs` (FILE); effect mod; iOS `sse.swift`; Android `core/SseClient.kt` |

Full grammar and workflows: **`$TEMPLATE_DIR/AGENTS.md`**. Platform Makefile target names follow the live template (`make build`, iOS `run-sim`, Android `doctor`) — not retired `sim-build` / `make verify` names.

## § Template / version-pin drift handling

Dependency pins live only as bytes in `$TEMPLATE_DIR`. There is no adapter-side version registry. Detect drift when a verify-repair loop fails repeatedly with cargo / Gradle / Xcode / BoltFFI errors that look like API renames, missing imports, or toolchain mismatches rather than feature-level bugs — or when consumer pin files diverge from the template counterparts after identity substitution.

**Pin-diff checklist (prompt-mandated; guest cannot see `$TEMPLATE_DIR`):** after materialize and on any pin-suspect failure, diff these consumer paths against the same relative paths under `$TEMPLATE_DIR`, allowing only identity substitution (`APP_NAME`, `ANDROID_PACKAGE` / package path forms):

- `Cargo.toml` (workspace deps)
- `Cargo.lock` (when present in both trees)
- `rust-toolchain.toml`, `deny.toml`
- `shared/Cargo.toml` (including the `boltffi = "…"` pin line)
- `shared/boltffi.toml` (structure + non-identity fields; package id may differ after substitution)
- `Android/gradle/libs.versions.toml`
- iOS / Android Makefiles and `iOS/project.yml` (BoltFFI pack recipes + `DESTINATION`)

**Agents:** detect → re-copy the drifted paths from `$TEMPLATE_DIR` with the same identity substitution as materialize → if the failure persists, mark the build `deferred` with a template / pin drift signal → **exit** (never invent versions). See [Consumer tooling boundary](emery-runtime/guardrails.md#consumer-tooling-boundary).

**Operators (separate maintainer session):** fix pins in [`augentic/vectis-exemplar`](https://github.com/augentic/vectis-exemplar); consumers re-copy from the refreshed checkout. Do not patch version tables inside the Vectis adapter.
