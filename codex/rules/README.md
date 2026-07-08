# Cross-target codex rules

Shared engineering standards that apply across every target adapter.

| Pack | Path | Namespace |
| ---- | ---- | ----------- |
| Universal | [`universal/`](universal/) | `UNI-*` |
| Target overlays | `targets/<name>/prose/rules/` | `OMNIA-*`, `VECTIS-*`, `IFACE-*`, … |
| Source overlays | `sources/<name>/prose/rules/` | `SRC-*` |

There is no framework rule pack: the [`augentic/specify`](https://github.com/augentic/specify) repository enforces its own authoring invariants with plain cargo tests (`tests/framework_quality/`), not codex rules.

The `specify` binary embeds `universal/` at build time from this checkout (sibling `../specify-adapters` or `SPECIFY_ADAPTERS`) and materializes it into the out-of-tree project codex cache at `specify init` / `specify adapters sync`.
