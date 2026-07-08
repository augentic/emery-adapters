# Cross-target codex rules

Shared engineering standards that apply across every target adapter.

| Pack | Path | Namespace |
| ---- | ---- | ----------- |
| Universal | [`universal/`](universal/) | `UNI-*` |
| Target overlays | `targets/<name>/prose/rules/` | `OMNIA-*`, `VECTIS-*`, `IFACE-*`, … |
| Source overlays | `sources/<name>/prose/rules/` | `SRC-*` |

Framework-only `CORE-*` rules live in [`augentic/specify`](https://github.com/augentic/specify) under `codex/rules/core/` — they enforce the Specify framework repository, not consumer projects.

The `specify` binary embeds `universal/` at build time from this checkout (sibling `../specify-adapters` or `SPECIFY_ADAPTERS`) and materializes it into the out-of-tree project codex cache at `specify init` / `specify adapters sync`.
