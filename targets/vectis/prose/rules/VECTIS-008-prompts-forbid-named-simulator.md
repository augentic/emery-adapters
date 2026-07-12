---
id: VECTIS-008
title: Vectis Prompts Forbid Named Simulator Destinations
severity: important
trigger: A Vectis target prompt instructs agents to set or pick a named iOS simulator destination instead of the CLI-owned generic destination.
applicability:
  adapters: [vectis]
references:
  - label: iOS hard rules
    path: adapters/targets/vectis/prose/references/hard-rules-ios.md
  - label: iOS build write prompt
    path: adapters/targets/vectis/prose/prompts/build/ios/write.md
---

## Rule

Vectis iOS verify and merge prompts must never tell agents to patch `iOS/Makefile`, `iOS/project.yml`, or `iOS/.vectis/sim-build.sh` with a named simulator (`name=iPhone …`, `platform=iOS Simulator,name=…`). The generic destination is CLI-owned in `iOS/.vectis/sim-build.sh`; the orchestrator runs `vectis sync ios-scaffold` when repair is needed.

## Look For

- Prompt prose or shell recipes that set `platform=iOS Simulator,name=` or `name=iPhone` in agent-facing instructions.
- Verify-repair guidance that tells agents to pick a simulator from `xcrun simctl list` and edit scaffold files.

## Fix

Remove named-destination instructions. Point agents at the vectis guest's `sync ios-scaffold` helper and Swift-only repair per the Vectis iOS build prompt (`targets/vectis/prose/prompts/build/ios/write.md`).
