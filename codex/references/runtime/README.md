# Shared spec runtime bundle

This directory is the single **spec-runtime bundle**: the canonical copy of the runtime references each adapter ships. Each source and target adapter exposes it as `references/spec-runtime/` via a single directory symlink (`{sources,targets}/<name>/prose/references/spec-runtime -> ../../../../codex/references/runtime`), so adapter prompts can link with `../references/spec-runtime/...` without escaping the adapter tree. The `prose` crate's build-time embed dereferences the symlinks when it bakes the bundle into each published component, so consumers receive self-contained regular files.

**Relationship to `augentic/specify`:** the engine's core judgment prose (reconciliation and synthesis playbooks, spec formatting, tags, Decision Record authoring) is embedded in the workflow crates' prompt corpora (`crates/slice/prompts/` and `crates/change/prompts/`) and is not mirrored here. This bundle carries only the *adapter-facing boundary* references — the vocabulary and contracts an adapter prompt needs to align with the engine without depending on the engine repo's `docs/` tree at runtime. There is no hand-maintained cross-repo parity table.

The bundle:

| Document                            | What adapters consume it for                                                                     |
| ----------------------------------- | ------------------------------------------------------------------------------------------------ |
| `guardrails.md`                     | Consumer-project boundaries every phase respects                                                  |
| `reconciliation.md`                 | How leads become slices and Evidence becomes a spec — the survey/extract vocabulary               |
| `specialist-usage.md`               | Where adapters read and write; the artifact-vs-specialist dividing line                           |
| `artifact-validation-checklist.md`  | Source-facing artifact self-review (target-specific checklists live with their owning adapters)   |
| `phase-outcome-contract.md`         | How a phase's report becomes the loop's outcome classification                                    |
| `spec-to-test-mapping.md`           | Shared requirement-to-test derivation rules build prompts cite                                    |
| `standards-layer-snippet.md`        | The workflow / artifacts / engineering-standards triad build prompts restate                      |
| `synthesis/authority.md`            | The authority hierarchy source adapters declare Evidence against                                  |

## Editing rules

- Edit the file here — this tree is the canonical bundle for adapters. Never replace an adapter's `references/spec-runtime` symlink with a directory of copies.
- Adding a new shared reference: drop the file here. Every adapter inherits it through its directory symlink automatically. Add a reference here only when at least two adapters consume it; single-adapter material belongs in that adapter's own `references/`.
- Keep agent-critical prose in this bundle (or the adapter's own `references/`); do not make prompts depend on the specify repo's `docs/` tree at runtime.

## Review-team protocol

The review-team protocol is a separate surface and is **not** part of the spec-runtime bundle above. It is exposed here as `review-team-protocol.md`, and each target adapter exposes it as `references/agent-teams.md -> ../../../../codex/references/runtime/review-team-protocol.md`. Overlays MUST be symlinks (regular-file copies are forbidden). The document is forked from `docs/reference/review-team-protocol.md` in the specify repo, where a framework-quality cargo test (`tests/framework/prose.rs`) guards the canonical document's presence.
