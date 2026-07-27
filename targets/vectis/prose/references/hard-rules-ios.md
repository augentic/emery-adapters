# iOS-Writer Rules and Important Notes

**When to read this**: open this file at the start of every iOS shell run, and again before final verification. It captures the scaffold immutability contract plus the normative facts about the iOS build surface — Makefile / `project.yml` / `sim-build.sh` ownership, simulator destination policy, and verify-repair scope — that are easy to violate by hand-editing the rendered scaffold or copying from worked examples.

## Scaffold immutability (create and update mode)

1. **Create mode must materialize first.** Copy from `$TEMPLATE_DIR` (`../vectis-template` or `VECTIS_TEMPLATE_DIR`) per `build.md` § Template materialize, then regenerate the Xcode project. Do not invent Swift or DX files when the template is missing.
2. **Never hand-author DX / pin files.** `iOS/Makefile`, `iOS/project.yml`, and other template-owned DX must come from `$TEMPLATE_DIR` after identity substitution — not from worked examples or memory.
3. **Keep DX aligned with `$TEMPLATE_DIR`.** On drift, re-copy those paths from the template; agents must not patch pins or destinations during verify-repair or feature work.
4. **Never set a named simulator destination in verify scripts.** Use the template's generic/`simctl` DX only. Do not substitute `name=iPhone …`, inline `-destination` in the Makefile, or run `xcodebuild` with a device-specific destination.
5. **Orchestrator runs verify; repair sub-agents are Swift-only.** The `/emery:build` orchestrator executes `swiftformat`, `make build`, and `make sim-build`. `ios-verify-repair` sub-agents must not run `make`, `xcodebuild`, or edit DX paths — they return Swift edits only.

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

- **Core must exist first**: the iOS shell writer runs against an existing Crux core. Materialize (or update) the `shared` crate before the iOS write leg.
- **Shell is thin**: all business logic lives in the Rust core. The shell only renders SwiftUI and performs platform I/O.
- **Worked examples show Swift patterns only**: [`examples/ios/`](examples/ios/) demonstrate `Core.swift` and view wiring — not authoritative Makefile or `project.yml` content. `$TEMPLATE_DIR` is the sole authority for those files.
- **BoltFFI / pin drift**: if the app fails with FFI contract or toolchain mismatches that look like pin drift, re-copy from `$TEMPLATE_DIR` — do not invent versions.
- **XcodeGen picks up nested dirs**: new theme, component, and asset directories under `iOS/<APP_NAME>/` require no `project.yml` edits.
