---
id: VECTIS-008
title: Vectis Prompts Forbid Named Simulator Destinations
severity: important
trigger: A Vectis target prompt instructs agents to set or pick a named iOS simulator destination instead of the template-owned generic destination.
applicability:
  adapters: [vectis]
references:
  - label: iOS hard rules
    path: adapters/targets/vectis/prose/references/hard-rules-ios.md
  - label: iOS build write prompt
    path: adapters/targets/vectis/prose/prompts/build/ios/write.md
---

## Rule

Vectis iOS verify and merge prompts must never tell agents to patch `iOS/Makefile` or `iOS/project.yml` with a named simulator (`name=iPhone …`, `platform=iOS Simulator,name=…`). The generic destination is template-owned in `iOS/Makefile` as `DESTINATION ?= generic/platform=iOS Simulator`. On DX drift, re-copy those paths from `$TEMPLATE_DIR` with identity substitution — do not invent Makefile content.

## Look For

- Prompt prose or shell recipes that set `platform=iOS Simulator,name=` or `name=iPhone` in agent-facing instructions.
- Verify-repair guidance that tells agents to pick a simulator from `xcrun simctl list` and edit DX files.
- Residual references to `iOS/.vectis/sim-build.sh` as the destination owner (retired — the Makefile owns `DESTINATION`).

## Fix

Remove named-destination instructions. Point agents at re-copy from `$TEMPLATE_DIR` and Swift-only repair per the Vectis iOS build prompt (`targets/vectis/prose/prompts/build/ios/write.md`).
