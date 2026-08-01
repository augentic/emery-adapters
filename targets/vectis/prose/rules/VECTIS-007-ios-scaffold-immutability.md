---
id: VECTIS-007
title: iOS DX File Immutability
severity: important
trigger: An agent-authored or drifted iOS shell edits template-owned DX files or substitutes a named simulator destination in the iOS Makefile.
applicability:
  adapters: [vectis]
references:
  - label: iOS hard rules
    path: adapters/targets/vectis/prose/references/hard-rules-ios.md
  - label: iOS build write prompt
    path: adapters/targets/vectis/prose/prompts/build/ios/write.md
---

## Rule

`iOS/Makefile` and `iOS/project.yml` are template-owned DX. They land from `$TEMPLATE_DIR` (`../vectis-exemplar` or `VECTIS_EXEMPLAR_DIR`) through the template-materialize copy procedure with identity substitution. Agents must never author, invent, or edit these files in create or update mode — `$TEMPLATE_DIR` is the sole shell example and DX authority.

The live template's verify destination is Makefile-owned:

```make
DESTINATION ?= generic/platform=iOS Simulator
```

`make build` (alias of `build-sim`) runs typegen, `boltffi pack apple`, `xcodegen`, then `xcodebuild` with that destination. There are no `iOS/.vectis/sim-build.sh` / `sim-dev.sh` scripts and no `cargo-swift` / `sharedFFI` recipes — BoltFFI owns the package step.

**Forbidden:**

- Hand-authoring `iOS/Makefile` or `iOS/project.yml` instead of materializing / re-copying from `$TEMPLATE_DIR`.
- Editing those files during verify-repair or feature work.
- Named simulator destinations in the verify path (`name=iPhone …`, `platform=iOS Simulator,name=…`).
- Direct `xcodebuild -destination` with a named device from agent-driven verify repair.
- Inventing UniFFI / `cargo-swift` / `--xcframework-name sharedFFI` DX when the template uses BoltFFI.

On drift, re-copy the DX paths from `$TEMPLATE_DIR` with the same identity substitution as materialize. The in-guest verify gate blocks missing DX and missing BoltFFI patterns; reviewers flag agent-side violations that survive those gates.

## Look For

- `iOS/Makefile` or `iOS/project.yml` content that diverges from `$TEMPLATE_DIR` after identity substitution (missing `boltffi pack apple`, missing `DESTINATION ?= generic/platform=iOS Simulator`, missing `path: ./generated/Shared`).
- A build destination using a named device instead of `generic/platform=iOS Simulator`.
- Evidence that an agent patched DX files after a simulator build failure rather than re-copying from `$TEMPLATE_DIR` or fixing Swift sources.

## Spec Guidance

When DX files drift, re-copy from `$TEMPLATE_DIR` — do not hand-edit the Makefile to pick a simulator, and never hand-scaffold over an existing tree. `$TEMPLATE_DIR` is the shell example and the only authority for Makefile / `project.yml` content. Pins live only as bytes in the template checkout — never invent versions.
