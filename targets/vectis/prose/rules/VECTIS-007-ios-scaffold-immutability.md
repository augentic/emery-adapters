---
id: VECTIS-007
title: iOS Scaffold File Immutability
severity: important
trigger: An agent-authored or drifted iOS shell edits CLI-owned scaffold files or substitutes a named simulator destination in the iOS Makefile or sim-build script.
applicability:
  adapters: [vectis]
rule_hints:
  - kind: path-pattern
    value: iOS/Makefile
    description: Flag named simulator destinations or inlined xcodebuild -destination in the CLI-owned Makefile.
  - kind: path-pattern
    value: iOS/.vectis/sim-build.sh
    description: Flag named simulator destinations in the CLI-owned sim-build script.
  - kind: regex
    value: name=iPhone|platform=iOS Simulator,name=
    description: Forbidden named simulator destination in Makefile or sim-build.sh (not sim-dev.sh).
references:
  - label: iOS hard rules
    path: adapters/targets/vectis/prose/references/hard-rules-ios.md
  - label: iOS template manifest
    path: adapters/targets/vectis/core/templates/ios/MANIFEST.md
---

## Rule

`iOS/Makefile`, `iOS/project.yml`, `iOS/.vectis/sim-build.sh`, and `iOS/.vectis/sim-dev.sh` are rendered exclusively from the embedded scaffold templates. Agents must never author, copy from worked examples, or edit these files in create or update mode.

The simulator destination for **verify** lives only in `iOS/.vectis/sim-build.sh`:

```bash
DEST='generic/platform=iOS Simulator'
```

The Makefile `sim-build` target delegates to that script — it must not inline `xcodebuild -destination`. Local-dev targets (`sim-install`, `sim-launch`, `sim-run`, `run`) delegate to `iOS/.vectis/sim-dev.sh`, which uses `simctl` for install/launch and does not run `xcodebuild` with a named destination.

**Forbidden:**

- Hand-authoring `iOS/Makefile`, `iOS/project.yml`, `iOS/.vectis/sim-build.sh`, or `iOS/.vectis/sim-dev.sh` instead of running `vectis scaffold ios`.
- Editing those files during verify-repair or feature work.
- Named simulator destinations in the **verify** path (`name=iPhone …`, `platform=iOS Simulator,name=…` in Makefile or `sim-build.sh`).
- Direct `xcodebuild -destination` with a named device from agent-driven verify repair.

Prepare auto-syncs immutable scaffold files before agent work; the orchestrator runs `vectis sync ios-scaffold` in-loop during iOS verify; verify blocks drift at build finalize. Reviewers flag agent-side violations that survive those gates.

## Look For

- `iOS/Makefile`, `iOS/project.yml`, `iOS/.vectis/sim-build.sh`, or `iOS/.vectis/sim-dev.sh` content that diverges from the embedded template for the resolved app name.
- A `sim-build` destination using a named device instead of `generic/platform=iOS Simulator`.
- Evidence that an agent patched scaffold files after a simulator build failure rather than running `vectis sync ios-scaffold` or fixing Swift sources.

## Spec Guidance

When scaffold files drift, let the adapter repair them: it re-renders the agent-immutable files deterministically around each shell write leg and at build prepare. Do not hand-edit the Makefile or script to pick a simulator, and never hand-scaffold over an existing tree. Worked examples demonstrate Swift patterns only; they are not authoritative for Makefile, `project.yml`, or `sim-build.sh` content.
