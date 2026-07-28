# iOS-Writer Rules and Important Notes

**When to read this**: open this file at the start of every iOS shell run, and again before final verification. It captures the DX immutability contract plus the normative facts about the iOS build surface — Makefile / `project.yml` ownership, BoltFFI pack flow, simulator destination policy, and verify-repair scope — that are easy to violate by hand-editing template-owned DX or inventing pins from memory.

## Scaffold immutability (create and update mode)

1. **Create mode must materialize first.** Copy from `$TEMPLATE_DIR` (`../vectis-exemplar` or `VECTIS_EXEMPLAR_DIR`) per `build.md` § Template materialize, then regenerate the Xcode project (`make -C iOS generate-project` / `xcodegen`). Do not invent Swift or DX files when the template is missing.
2. **Never hand-author DX / pin files.** `iOS/Makefile` and `iOS/project.yml` must come from `$TEMPLATE_DIR` after identity substitution — not from memory.
3. **Keep DX aligned with `$TEMPLATE_DIR`.** On drift, re-copy those paths from the template; agents must not patch pins or destinations during verify-repair or feature work.
4. **Never set a named simulator destination in verify DX.** The template Makefile owns `DESTINATION ?= generic/platform=iOS Simulator`. Do not substitute `name=iPhone …` or run `xcodebuild` with a device-specific destination.
5. **Orchestrator runs verify; repair sub-agents are Swift-only.** The `/emery:build` orchestrator executes `swiftformat` and `make build` (typegen + `boltffi pack apple` + xcodegen + simulator build). `ios-verify-repair` sub-agents must not run `make`, `xcodebuild`, or edit DX paths — they return Swift edits only. After a clean `make build`, write `iOS/.vectis/verify.ok` (adapter stamp; not template DX).

## Running iOS locally

Operators may desk-check the app outside the orchestrator verify loop:

```bash
cd iOS && make build && make run-sim
# or with an explicit simulator:
SIMULATOR_UDID=<udid> make run-sim
```

Built artifact:

```text
iOS/DerivedData/Build/Products/Debug-iphonesimulator/<AppName>-iOS.app
```

`SIMULATOR_UDID` (or `iOS/.env.local`) is read by the Makefile's `_run-sim` recipe — not by the verify path. There is no `sim-build` / `sim-run` alias in the live template (`build` / `run-sim`).

## Important notes

- **Core must exist first**: the iOS shell writer runs against an existing Crux core. Materialize (or update) the `shared` crate before the iOS write leg.
- **Shell is thin**: all business logic lives in the Rust core. The shell only renders SwiftUI and performs platform I/O.
- **`$TEMPLATE_DIR` is the shell example and DX authority**: read Swift patterns from the live template tree (`iOS/<APP>/` after identity substitution). `iOS/Makefile` and `iOS/project.yml` come only from `$TEMPLATE_DIR` — never invent them.
- **BoltFFI / pin drift**: if the app fails with FFI contract or toolchain mismatches that look like pin drift, re-copy from `$TEMPLATE_DIR` — do not invent versions. Diff pin-bearing files against the template (see `build.md` § Template / version-pin drift handling).
- **XcodeGen picks up nested dirs**: new theme, component, and asset directories under `iOS/<APP_NAME>/` require no `project.yml` edits.
