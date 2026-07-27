# Template capabilities — strip and late adoption

Greenfield trees come from a local [`vectis-template`](https://github.com/augentic/vectis-template) checkout (`$TEMPLATE_DIR`: default `../vectis-template`, override `VECTIS_TEMPLATE_DIR`). Pins, BoltFFI DX, and optional-capability shapes live only in that checkout — never invent versions or FFI layouts in the consumer.

**Strip grammar and marker discovery** stay in **`$TEMPLATE_DIR/AGENTS.md`** (not copied into the consumer). This reference covers DX completeness after materialize, and how a later slice **re-adopts** a capability that was stripped at greenfield.

## After materialize: DX that must remain

`vectis::scaffold::materialize` copies root + shell DX, `shared/` (including `boltffi.toml` and `shared/src/bin/codegen/`), `supply-chain/`, and `.maestro/`. After stripping `VECTIS-OPTIONAL` (especially `cap=demo`), keep:

| Path | Role |
|------|------|
| Root `Makefile`, `Makefile.toml`, `Cargo.toml` / `Cargo.lock`, `rust-toolchain.toml`, `deny.toml` | Workspace DX and pins |
| `iOS/Makefile`, `iOS/project.yml` | BoltFFI pack + xcodegen DX (`make build` / `run-sim`) |
| `Android/Makefile` + Gradle wrapper + `gradle/libs.versions.toml` | BoltFFI pack + assemble DX (`make build` / `doctor`) |
| `shared/boltffi.toml`, `shared/src/bin/codegen/` | FFI + Maestro test-id codegen |
| `.maestro/config.yaml`, `.maestro/test-ids.yaml`, `.maestro/scripts/load-test-ids.sh` | Maestro infra (not demo journeys) |

Strip demo Maestro journeys / `run-*.sh` / `cap=demo` blocks inside `test-ids.yaml` per `$TEMPLATE_DIR/AGENTS.md` and `$TEMPLATE_DIR/.maestro/README.md`. Do **not** delete the infra files above. README demo (`cap=demo` doc) blocks may be trimmed; keep the file.

`web/` is never materialized (out of scope). Regenerate `iOS/*.xcodeproj` via `make -C iOS generate-project` after materialize.

## Late capability adoption (later slice)

When a later slice's `design.md` `## Adapters` flips a capability from No → Yes (`http` / `kv` / `time` / `sse`), **copy the strip-unit for that `cap=` from `$TEMPLATE_DIR`** — do not invent dependency lines, effect variants, or shell handler shapes.

1. Confirm `$TEMPLATE_DIR` is present (same resolve rules as greenfield). Fail closed if missing.
2. In `$TEMPLATE_DIR`, discover the unit: `rg 'VECTIS-OPTIONAL.*cap=<cap>'` (and `FILE` markers). Read each opener's `Keep if:` / `Remove if:` / `Paired:` lines.
3. For every scope the unit spans (`dep`, `core`, `shell`, `test` — skip `web/` shell files), copy the corresponding marked block or file into the consumer at the same relative path after identity substitution (`APP_NAME`, `ANDROID_PACKAGE` / package path). Prefer whole `FILE`-marked handlers over inventing stubs.
4. Wire `Effect` / `Core` dispatch / imports so the project compiles; run `cargo make generate` when `Effect` variants or `facet_typegen` deps change (per template `Paired:` guidance).
5. Diff pin files against `$TEMPLATE_DIR` (see [build.md](../prompts/build.md) § Template / version-pin drift handling). On mismatch, re-copy from the template — never guess versions.
6. Do **not** re-introduce `cap=demo` for product apps.

Authoritative strip-unit map (paths use the template's `VectisApp` / `io.augentic.vectisapp` identity — rewrite to the consumer identity):

| Cap | Typical locations under `$TEMPLATE_DIR` |
|-----|----------------------------------------|
| `http` | `Cargo.toml` / `shared/Cargo.toml` deps; `shared/src/effects.rs` arms; iOS `http.swift` + `core.swift` dispatch; Android `core/HttpClient.kt` + `Core.kt` |
| `kv` | Same dep/effect pattern; iOS `keyvalue.swift` / `KeyValueStore.swift`; Android `core/KeyValueStore.kt` |
| `time` | Same dep/effect pattern; iOS `time.swift`; Android `core/TimeHandler.kt` |
| `sse` | `shared/src/effects/sse.rs` (FILE); effect mod; iOS `sse.swift`; Android `core/SseClient.kt` |

Full grammar and workflows: **`$TEMPLATE_DIR/AGENTS.md`**. Platform Makefile target names follow the live template (`make build`, iOS `run-sim`, Android `doctor`) — not retired `sim-build` / `make verify` names.
