# Vectis sample artifacts + template pointer

The live [`vectis-exemplar`](https://github.com/augentic/vectis-exemplar) checkout is the worked example for the shared Crux core, iOS/Android shells, DX / pins, BoltFFI, and `VECTIS-OPTIONAL` strip units. Resolve it as `$TEMPLATE_DIR` (`../vectis-exemplar` relative to the consumer project root, or `VECTIS_EXEMPLAR_DIR`).

There are no in-adapter markdown walkthroughs under `examples/core|ios|android` — read the template tree, then the adapter pattern refs for anatomy that is not a file copy.

## Capability reading → `$TEMPLATE_DIR`

Strip grammar and marker discovery live only in `$TEMPLATE_DIR/AGENTS.md`. Typical paths (template identity `VectisApp` / `io.augentic.vectisapp` — rewrite after materialize):

| Need | Where to look |
| --- | --- |
| Render-only (after strip) | `$TEMPLATE_DIR/shared/src/` (`app.rs`, `model/`, `view/`, `effects.rs` with only `Render`) + shell bridges with cap handlers removed |
| `cap=http` | Android `Android/app/.../core/HttpClient.kt`; iOS `iOS/<APP>/http.swift` + `core.swift` dispatch; `shared` HTTP effect arms / deps |
| `cap=kv` | Android `core/KeyValueStore.kt`; iOS `keyvalue.swift` / `KeyValueStore.swift`; shared KV effect arms |
| `cap=time` | Android `core/TimeHandler.kt`; iOS `time.swift`; shared time effect arms |
| `cap=sse` | Android `core/SseClient.kt`; iOS `sse.swift`; `shared/src/effects/sse.rs` |
| DX / pins / BoltFFI | Root + `iOS/Makefile` / `iOS/project.yml` / Android Makefile + Gradle — never invent from memory |

Full strip-unit map and late-adoption steps: [`../template-capabilities.md`](../template-capabilities.md) and `$TEMPLATE_DIR/AGENTS.md`.

## Pattern refs (non-file anatomy)

- [`../crux/`](../crux/) — Crux idioms, capabilities, artifact→code mapping.
- [`../ios/shell-pattern.md`](../ios/shell-pattern.md) / [`../ios/view-patterns.md`](../ios/view-patterns.md) — SwiftUI shell anatomy.
- [`../android/shell-pattern.md`](../android/shell-pattern.md) / [`../android/view-patterns.md`](../android/view-patterns.md) — Compose shell anatomy.

## Sample artifacts

| File | Purpose |
| --- | --- |
| [tokens.yaml](tokens.yaml) | Example operator-curated `tokens.yaml` design-token input. |
| [assets.yaml](assets.yaml) | Example operator-curated `assets.yaml` asset-inventory input. |
