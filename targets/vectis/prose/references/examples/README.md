# Vectis Worked Examples

Index of the worked-example corpus for Vectis generation. Examples are numbered in increasing capability order and paired across trees: read the `core/` example first, then the matching shell example for each declared platform.

**Pin / DX authority:** dependency versions, Makefiles, Gradle files, and BoltFFI config come from `$TEMPLATE_DIR` (`../vectis-template` or `VECTIS_TEMPLATE_DIR`). Example `Cargo.toml` / Makefile / Gradle blocks are pedagogical — never invent pins from memory or from retired UniFFI / `cargo-swift` snippets.

## core/ — shared Crux core

| File | Read when generating |
| --- | --- |
| [core/01-simple-counter.md](core/01-simple-counter.md) | A render-only app (no effects beyond render). |
| [core/02-http-counter.md](core/02-http-counter.md) | An app with HTTP effects (render + HTTP). |
| [core/03-kv-notes.md](core/03-kv-notes.md) | An app with key-value persistence (render + KV). |

## ios/ — SwiftUI shell counterparts

| File | Read when generating |
| --- | --- |
| [ios/01-simple-counter.md](ios/01-simple-counter.md) | The iOS shell for the render-only counter. |
| [ios/02-http-counter.md](ios/02-http-counter.md) | The iOS shell for the HTTP counter. |

## android/ — Compose shell counterparts

| File | Read when generating |
| --- | --- |
| [android/01-simple-counter.md](android/01-simple-counter.md) | The Android shell for the render-only counter. |
| [android/02-http-counter.md](android/02-http-counter.md) | The Android shell for the HTTP counter. |

## Sample artifacts

| File | Purpose |
| --- | --- |
| [tokens.yaml](tokens.yaml) | Example operator-curated `tokens.yaml` design-token input. |
| [assets.yaml](assets.yaml) | Example operator-curated `assets.yaml` asset-inventory input. |
