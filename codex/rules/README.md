# Cross-target codex rules

Shared engineering standards that apply across every target adapter.

| Pack            | Path                          | Namespace                           |
| --------------- | ----------------------------- | ----------------------------------- |
| Universal       | [`universal/`](universal/)    | `UNI-*`                             |
| Target overlays | `targets/<name>/prose/rules/` | `OMNIA-*`, `VECTIS-*`, `IFACE-*`, … |
| Source overlays | `sources/<name>/prose/rules/` | `SRC-*`                             |

There is no framework rule pack: the [`augentic/emery`](https://github.com/augentic/emery) repository enforces its own authoring invariants with plain cargo tests (`tests/framework/`), not codex rules.

Each code-generating target adapter (omnia, vectis) embeds `universal/` into its own component via a `prose/rules/universal` symlink, alongside its overlay rules; review agents read the pack through the adapter's references server (`rules/universal/…`).
