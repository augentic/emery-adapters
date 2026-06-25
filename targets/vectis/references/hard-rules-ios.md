# iOS-Writer Rules and Important Notes

**When to read this**: open this file at the start of every iOS shell run, and again before final verification. It captures the scaffold immutability contract plus the normative facts about the iOS build surface — Makefile / `project.yml` ownership, simulator destination policy, and verify-repair scope — that are easy to violate by hand-editing the rendered scaffold or copying from worked examples.

## Scaffold immutability (create and update mode)

1. **Create mode must scaffold first.** Run `specify extension run vectis -- scaffold ios <APP_NAME> [--caps <csv>]` before writing any Swift under `iOS/`. Do not create Swift files before scaffold; scaffold must be the first write to `iOS/`.
2. **Never hand-author scaffold files.** `iOS/Makefile`, `iOS/project.yml`, and the starter files emitted by scaffold (`<APP_NAME>App.swift`, starter `Core.swift`, `ContentView.swift`, starter `Views/`) must come from the scaffold renderer — not from worked examples or memory.
3. **Never edit `iOS/Makefile` or `iOS/project.yml`.** These files are CLI-owned. `specify slice build --phase prepare` auto-syncs them at build start; `specify extension run vectis -- sync ios-scaffold` repairs them in-loop during verify — agents must not patch them during verify-repair or feature work.
4. **Never set a named simulator destination.** The `sim-build` target must keep `-destination 'generic/platform=iOS Simulator'`. Do not substitute `name=iPhone …` or any device-specific destination — it breaks on hosts with ambiguous simulator names.
5. **Verify-repair scope is Swift and generated UI only.** When `make sim-build` fails, fix Swift under `iOS/<APP_NAME>/`, plus `Theme/`, `Components/`, and `Resources/`. Makefile and `project.yml` are out of scope — if destination drift is suspected, run `specify extension run vectis -- sync ios-scaffold` and retry `make sim-build` once; do not edit the Makefile by hand.

## Important notes

- **Core must exist first**: the iOS shell writer runs against an existing Crux core. Generate the `shared` crate before scaffolding the iOS tree.
- **Shell is thin**: all business logic lives in the Rust core. The shell only renders SwiftUI and performs platform I/O.
- **Worked examples show Swift patterns only**: [`examples/ios/`](examples/ios/) demonstrate `Core.swift` and view wiring — not authoritative Makefile or `project.yml` content. The embedded scaffold template is the sole authority for those files.
- **UniFFI / cargo-swift drift**: if the app panics with `UniFFI contract version mismatch`, surface a template / pin drift signal to the operator — do not patch Makefile or `project.yml` to work around it.
- **XcodeGen picks up nested dirs**: new theme, component, and asset directories under `iOS/<APP_NAME>/` require no `project.yml` edits.
