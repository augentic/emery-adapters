# Shared spec runtime bundle

This directory is the single **spec-runtime bundle**: the canonical copy of the runtime references each adapter ships. Each source and target adapter exposes it as `references/spec-runtime/` via a single directory symlink (`{sources,targets}/<name>/prose/references/spec-runtime -> ../../../../codex/references/runtime`), so adapter prompts can link with `../references/spec-runtime/...` without escaping the adapter tree. The `prose` crate's build-time embed dereferences the symlinks when it bakes the bundle into each published component, so consumers receive self-contained regular files.

**Relationship to `augentic/specify`:** these files began as a fork of the plugin-runtime references at `plugins/spec/references/` in the specify repo, and the two surfaces have since evolved independently — this tree is canonical for adapter components; `plugins/spec/references/` is canonical for the Cursor plugin cache. When a change to one surface affects prose the other mirrors, port it by hand in the same change (there is no parity script or sync step).

| Bundle path                         | Historical origin (specify repo)                            |
| ----------------------------------- | ----------------------------------------------------------- |
| `guardrails.md`                     | `plugins/spec/references/guardrails.md`                     |
| `specialist-usage.md`               | `plugins/spec/references/specialist-usage.md`               |
| `reconciliation.md`                 | `plugins/spec/references/reconciliation.md`                 |
| `components.md`                     | `plugins/spec/references/components.md`                     |
| `standards-layer-snippet.md`        | `plugins/spec/references/standards-layer-snippet.md`        |
| `artifact-validation-checklist.md`  | `plugins/spec/references/artifact-validation-checklist.md`  |
| `phase-outcome-contract.md`         | `plugins/spec/references/phase-outcome-contract.md`         |
| `plan-lock.md`                      | `plugins/spec/references/plan-lock.md`                      |
| `stop-conditions.md`                | `plugins/spec/references/stop-conditions.md`                |
| `spec-to-test-mapping.md`           | `plugins/spec/references/spec-to-test-mapping.md`           |
| `cli/plan-propose.md`               | `plugins/spec/references/cli/plan-propose.md`               |
| `synthesis/authority.md`            | `plugins/spec/references/synthesis/authority.md`            |
| `synthesis/tags.md`                 | `plugins/spec/references/synthesis/tags.md`                 |
| `synthesis/provenance.md`           | `plugins/spec/references/synthesis/provenance.md`           |
| `synthesis/claim-reconciliation.md` | `plugins/spec/references/synthesis/claim-reconciliation.md` |

## Editing rules

- Edit the file here — this tree is the canonical bundle for adapters. Never replace an adapter's `references/spec-runtime` symlink with a directory of copies.
- Adding a new shared reference: drop the file here. Every adapter inherits it through its directory symlink automatically.
- Keep agent-critical prose in this bundle (or the adapter's own `references/`); do not make prompts depend on the specify repo's `docs/` tree at runtime.

## Review-team protocol

The review-team protocol is a separate surface and is **not** part of the spec-runtime bundle above. It is exposed here as `review-team-protocol.md`, and each target adapter exposes it as `references/agent-teams.md -> ../../../../codex/references/runtime/review-team-protocol.md`. Overlays MUST be symlinks (regular-file copies are forbidden). The document is forked from `docs/reference/review-team-protocol.md` in the specify repo, where a framework-quality cargo test (`tests/framework/prose.rs`) guards the canonical document's presence.
