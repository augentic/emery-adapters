# Shared spec runtime bundle

This directory is the single **emery-runtime bundle**: the canonical copy of the runtime references each source adapter ships. Each adapter exposes it as `references/emery-runtime/` via a single directory symlink (`sources/<name>/prose/references/emery-runtime -> ../../../../codex/references/runtime`), so adapter prompts can link with `../references/emery-runtime/...` without escaping the adapter tree. The `prose` crate's build-time embed dereferences the symlinks when it bakes the bundle into each published component, so consumers receive self-contained regular files.

**Relationship to `augentic/emery`:** the engine's synthesis prose (reconciliation playbook, spec formatting, tags) is embedded in the engine's own prompt corpus (`crates/engine/prose/`) and is not mirrored here. This bundle carries only the *adapter-facing boundary* references — the vocabulary an extract prompt needs to align with the engine without depending on the engine repo at runtime.

The bundle:

| Document                 | What adapters consume it for                                          |
| ------------------------ | --------------------------------------------------------------------- |
| `reconciliation.md`      | The specify pipeline and the claim-id / extras rules extraction feeds |
| `synthesis/authority.md` | The authority hierarchy source adapters declare Evidence against      |

## Editing rules

- Edit the file here — this tree is the canonical bundle for adapters. Never replace an adapter's `references/emery-runtime` symlink with a directory of copies.
- Adding a new shared reference: drop the file here. Every adapter inherits it through its directory symlink automatically. Add a reference here only when at least two adapters consume it; single-adapter material belongs in that adapter's own `references/`.
- Keep agent-critical prose in this bundle (or the adapter's own `references/`); do not make prompts depend on the emery repo's `docs/` tree at runtime.
