---
id: CORE-062
title: Vectis Prompts Forbid Named Simulator Destinations
severity: important
trigger: A Vectis target prompt instructs agents to set or pick a named iOS simulator destination instead of the CLI-owned generic destination.
rule_hints:
  - kind: path-pattern
    value: targets/vectis/prose/prompts/**/*.md
  - kind: regex
    value: "platform=iOS Simulator,name=|-destination[^\\n]*name=iPhone"
    description: Vectis build prompts must not instruct agents to substitute named simulator destinations in scaffold files.
---

## Rule

Vectis iOS verify and merge prompts must never tell agents to patch `iOS/Makefile`, `iOS/project.yml`, or `iOS/.vectis/sim-build.sh` with a named simulator (`name=iPhone …`, `platform=iOS Simulator,name=…`). The generic destination is adapter-owned in `iOS/.vectis/sim-build.sh`; the adapter re-renders the scaffold files deterministically around each write leg, so drift repairs itself.

## Look For

- Prompt prose or shell recipes that set `platform=iOS Simulator,name=` or `name=iPhone` in agent-facing instructions.
- Verify-repair guidance that tells agents to pick a simulator from `xcrun simctl list` and edit scaffold files.

## Fix

Remove named-destination instructions. The adapter re-syncs the scaffold files deterministically; point agents at Swift-only repair per [`targets/vectis/prose/prompts/build/ios/write.md`](../../../targets/vectis/prose/prompts/build/ios/write.md).
