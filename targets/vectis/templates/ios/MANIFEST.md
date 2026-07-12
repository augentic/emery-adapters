# iOS Assembly Template Manifest

Human reference for the iOS assembly templates. The canonical source-to-target
registry is [`../manifest.yaml`](../manifest.yaml) (`assemblies.ios`); `build.rs` validates that manifest and emits the embedded `registry.rs` consumed by the adapter's deterministic `scaffold` renderer (`ios` assembly).

Source paths are declared under `templates/ios/` (mostly flat filenames; nested sources such as `.vectis/sim-build.sh` use subdirectories). Nested target paths (especially the `iOS/__APP_NAME__/...` segment) are declared in `manifest.yaml`. The `__APP_NAME__` segment in target paths is substituted by the engine when writing each file, the same as inside file contents (e.g. `__APP_NAME__App.swift` becomes `CounterApp.swift`).

Total: 11 files (matches the file manifest contract for iOS assembly).

## Agent-immutable scaffold files

These paths are adapter-owned — agents must never author or edit them. The build guest re-renders them from the embedded templates on each build; the in-guest shell-verify gate blocks drift.

| Path | Policy |
| ---- | ------ |
| `iOS/Makefile` | Fully immutable for agents. `package` target: `cargo swift package` must pass `--xcframework-name sharedFFI`. `sim-build` delegates to `iOS/.vectis/sim-build.sh` — never inline `xcodebuild -destination`. Local-dev targets (`sim-install`, `sim-launch`, `sim-run`, `run`, `sim-app-path`) delegate to `iOS/.vectis/sim-dev.sh`. |
| `iOS/project.yml` | Fully immutable for agents. Sets `SWIFT_TREAT_WARNINGS_AS_ERRORS: YES` on the app target only — shell Swift warnings fail the build. Generated `Shared` / `SharedTypes` SPM packages are patched by `iOS/.vectis/relax-generated-spm-packages.sh` after typegen/package. Never add `OTHER_LDFLAGS: ["-w"]` or other linker warning suppression. XcodeGen picks up nested theme / component / asset directories automatically. |
| `iOS/.vectis/relax-generated-spm-packages.sh` | Fully immutable for agents. Relaxes compiler warnings on generated SPM `Package.swift` targets (UniFFI / facet output). |
| `iOS/.vectis/sim-build.sh` | Fully immutable for agents. Must set `DEST='generic/platform=iOS Simulator'` — never a named device (`name=iPhone …`). Writes `-derivedDataPath` to `iOS/DerivedData/` so verify and local-dev share a predictable `.app` path. |
| `iOS/.vectis/sim-dev.sh` | Fully immutable for agents. Local-dev install/launch only — not part of the orchestrator verify loop. Resolves simulator via `SIM_UDID`, or `SIM_DEVICE` + `SIM_OS`, or booted/first-available iPhone fallback. |

### Verify vs local-dev

- **Verify path** (orchestrator): `make build` → `make sim-build` → `.vectis/verify.ok`. Uses generic simulator destination only.
- **Local-dev path** (operators): `make run` / `make sim-run` → `sim-dev.sh run` (builds via `sim-build.sh` if needed, then `simctl install` + `simctl launch`).

### Stable build output

After `make sim-build`, the simulator `.app` is at:

```text
iOS/DerivedData/Build/Products/Debug-iphonesimulator/<AppName>.app
```

**Tradeoff:** in-repo `DerivedData` gives predictable paths and per-project cache isolation; first build may be cold vs a warm global Xcode cache. Global DerivedData is not used.

### Simulator selection (local-dev)

| Variable | Purpose |
| -------- | ------- |
| `SIM_UDID` | Use this simulator directly (highest priority) |
| `SIM_DEVICE` + `SIM_OS` | Match device name and runtime version (e.g. `SIM_DEVICE="iPhone 17"` `SIM_OS="18.0"`) |
| *(default)* | Booted simulator if any; else first available device whose name contains `iPhone` |

Swift sources under `iOS/<APP_NAME>/` (except the scaffold-only starter layout in create mode) and generated `Theme/`, `Components/`, `Resources/` remain agent-writable per the iOS build prompt.

## Placeholder reference

Always present in the iOS templates:

| Placeholder           | Example value | Files                                                                 |
| --------------------- | ------------- | --------------------------------------------------------------------- |
| `__APP_NAME__`        | `Counter`     | `project.yml`, `Makefile`, `App.swift`, `Core.swift`, `ContentView.swift`, `HomeScreen.swift`, `.vectis/*.sh` (and the file/folder paths in MANIFEST) |
| `__APP_NAME_LOWER__`  | `counter`     | `project.yml` (bundle id prefix and per-config bundle ids), `.vectis/sim-dev.sh` |

`__APP_NAME_LOWER__` is the lowercase form of the app name (no other
transformations -- `TodoApp` → `todoapp`). The engine in chunk 7 derives it
from `--app-name` rather than asking the user to provide it; it never appears
on the CLI surface.

There are no capability-version placeholders in the iOS assembly today. The
shell depends only on the generated `Shared` and `SharedTypes` Swift packages,
which are produced from the core's pinned Crux versions.

## Cap-marker reference

Capability-conditional regions follow the same convention as core (paired
`<<<CAP:<name>` / `CAP:<name>>>>` lines, each on their own line). The engine
treats the entire region (markers and content inclusive) as removable when the
cap is not selected, and drops only the marker lines (preserving content) when
the cap is selected.

| Cap        | Files                  |
| ---------- | ---------------------- |
| `http`     | `Core.swift`           |
| `kv`       | `Core.swift`           |
| `time`     | `Core.swift`           |
| `platform` | `Core.swift`           |

Notes for chunk 7:

- Marker open/close lines do not nest. Every `<<<CAP:foo` must be paired with
  the next `CAP:foo>>>` on its own line.
- The Swift compiler enforces exhaustive switches on enums. Each cap-conditional
  region in `Core.swift` must include both the matching `case` arm in
  `processEffect(_:)` _and_ any helper functions it relies on, all inside the
  same CAP marker. The engine does not do dead-code elimination on Swift -- if
  the cap is selected, both the case arm and the helper land in the rendered
  file together; if not, both vanish.
- The `sse` cap intentionally has no entry in `Core.swift` today. The render-
  only baseline of `app.rs` does not declare an `Effect::Sse(...)` variant
  (see `templates/core/MANIFEST.md`'s "Notes for chunk 5/6"), so the
  Swift `Effect` enum produced by the codegen has no `.sse` case to handle.
  When chunk 6 decides whether to add the Rust-side variant, this manifest and
  `Core.swift` should grow a matching `<<<CAP:sse` block.

## Inject

The templates deliberately omit `Inject` from the project's SPM dependency
list. **`Inject`** (hot-reload) is a per-developer convenience that requires
network resolution at first build and an external `InjectionIII` macOS app.
Including it would make the deterministic baseline depend on network
connectivity for the first `xcodegen`/`xcodebuild` cycle, which violates
the "one command, working project" promise.

Theme and token code is emitted as shell-local files under `iOS/<App>/Theme/`
by the `ios-writer` skill during Update Mode (the generated layout contract).
The CLI scaffold does not include theme files because `tokens.yaml` may not
exist at scaffold time; the writer adds them on first generation.

If hot-reload returns, it can be added as a cap-style toggle
(e.g. `--hot-reload`) and gated by its own marker.

## Self-check

Orphan detection and file-count parity (11 files) run in `build.rs` when the crate builds. After adding or renaming a template file, update [`../manifest.yaml`](../manifest.yaml) in the same change.
