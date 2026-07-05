# iOS-Writer Rules and Important Notes

**When to read this**: open this file at the start of every iOS shell run, and again before final verification. It captures the scaffold immutability contract plus the normative facts about the iOS build surface — Makefile / `project.yml` / `sim-build.sh` ownership, simulator destination policy, and verify-repair scope — that are easy to violate by hand-editing the rendered scaffold or copying from worked examples.

## Scaffold immutability (create and update mode)

1. **Create mode must scaffold first.** The adapter renders the iOS scaffold deterministically from its embedded templates before the write leg. Do not create Swift files before the scaffold exists; the rendered scaffold must be the first write to `iOS/`.
2. **Never hand-author scaffold files.** `iOS/Makefile`, `iOS/project.yml`, `iOS/.vectis/sim-build.sh`, `iOS/.vectis/sim-dev.sh`, and the starter files emitted by scaffold (`<APP_NAME>App.swift`, starter `Core.swift`, `ContentView.swift`, starter `Views/`) must come from the scaffold renderer — not from worked examples or memory.
3. **Never edit adapter-owned scaffold files.** `iOS/Makefile`, `iOS/project.yml`, `iOS/.vectis/sim-build.sh`, and `iOS/.vectis/sim-dev.sh` are adapter-owned. The adapter re-renders them deterministically at build prepare and around each write leg — agents must not patch them during verify-repair or feature work.
4. **Never set a named simulator destination in verify scripts.** The destination lives only in `iOS/.vectis/sim-build.sh` as `generic/platform=iOS Simulator`. Do not substitute `name=iPhone …`, inline `-destination` in the Makefile, or run `xcodebuild` with a device-specific destination.
5. **Orchestrator runs verify; repair sub-agents are Swift-only.** The `/spec:build` orchestrator executes sync, `swiftformat`, `make build`, and `make sim-build`. `ios-verify-repair` sub-agents must not run `make`, `xcodebuild`, or edit scaffold paths — they return Swift edits only.

## Running iOS locally

Operators may desk-check the app outside the orchestrator verify loop:

```bash
cd iOS && make build && make sim-run
# or with an explicit simulator:
SIM_UDID=$(xcrun simctl list devices booted -j | ...) make sim-run
# disambiguate by OS version:
SIM_DEVICE="iPhone 17" SIM_OS="18.0" make sim-run
```

`make run` is an alias for `make sim-run` (Android parity). Built artifact:

```text
iOS/DerivedData/Build/Products/Debug-iphonesimulator/<AppName>.app
```

Environment variables (`SIM_UDID`, `SIM_DEVICE`, `SIM_OS`) are read by `iOS/.vectis/sim-dev.sh` only — not by the verify path.

## Important notes

- **Core must exist first**: the iOS shell writer runs against an existing Crux core. Generate the `shared` crate before scaffolding the iOS tree.
- **Shell is thin**: all business logic lives in the Rust core. The shell only renders SwiftUI and performs platform I/O.
- **Worked examples show Swift patterns only**: [`examples/ios/`](examples/ios/) demonstrate `Core.swift` and view wiring — not authoritative Makefile, `project.yml`, or `sim-build.sh` content. The embedded scaffold template is the sole authority for those files.
- **UniFFI / cargo-swift drift**: if the app panics with `UniFFI contract version mismatch`, surface a template / pin drift signal to the operator — do not patch scaffold files to work around it.
- **XcodeGen picks up nested dirs**: new theme, component, and asset directories under `iOS/<APP_NAME>/` require no `project.yml` edits.
