---
id: VECTIS-007
title: iOS Scaffold File Immutability
severity: important
trigger: An agent-authored or drifted iOS shell edits CLI-owned scaffold files or substitutes a named simulator destination in the iOS Makefile.
applicability:
  adapters: [vectis]
references:
  - label: iOS hard rules
    path: adapters/targets/vectis/references/hard-rules-ios.md
  - label: iOS template manifest
    path: adapters/targets/vectis/extension/templates/ios/MANIFEST.md
---

## Rule

`iOS/Makefile` and `iOS/project.yml` are rendered exclusively from the embedded scaffold templates. Agents must never author, copy from worked examples, or edit these files in create or update mode.

The `sim-build` target must keep:

```makefile
-destination 'generic/platform=iOS Simulator'
```

**Forbidden:**

- Hand-authoring `iOS/Makefile` or `iOS/project.yml` instead of running `vectis scaffold ios`.
- Editing those files during verify-repair or feature work.
- Named simulator destinations (`name=iPhone …`, `platform=iOS Simulator,name=…`).

Prepare auto-syncs immutable scaffold files before agent work; verify blocks drift at build finalize. Reviewers flag agent-side violations that survive those gates.

## Look For

- `iOS/Makefile` or `iOS/project.yml` content that diverges from the embedded template for the resolved app name.
- A `sim-build` destination using a named device instead of `generic/platform=iOS Simulator`.
- Evidence that an agent patched the Makefile after a simulator build failure rather than fixing Swift sources or relying on prepare sync.

## Spec Guidance

When scaffold files drift, run `specify slice build --phase prepare` (auto-sync) or regenerate via `specify extension run vectis -- scaffold ios <APP_NAME>` on a clean tree — do not hand-edit the Makefile to pick a simulator. Worked examples demonstrate Swift patterns only; they are not authoritative for Makefile or `project.yml` content.
