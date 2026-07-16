# Vectis Target Adapter - Agent Instructions

These instructions extend the repository root [`AGENTS.md`](../../AGENTS.md) for all work under `targets/vectis/`.

Vectis is a Specify **target adapter**. Its `guidance`, `build`, and `merge` operations generate Crux shared cores, SwiftUI iOS shells, and Kotlin/Jetpack Compose Android shells from synthesised Specify artifacts plus operator-curated design-system inputs.

## Ownership boundaries

- `spec.md` stays behavioral and platform-neutral. `design.md` carries the Crux type system and platform design.
- `tokens.yaml` and `assets.yaml` are operator-curated build inputs; Vectis consumes them but never synthesises them.
- `composition.yaml` is a target build output. Regenerate it from `spec.md` and `design.md`; do not treat it as a Specify synthesis artifact.
- The `screenshots` source adapter owns image-to-layout Evidence. Vectis consumes spatial claims only after synthesis folds them into the canonical artifacts.
- Scaffold templates, schemas, materializers, validators, and version pins are adapter tooling. Consumer build prompts must stop and report tooling drift rather than patch this repository in-band.

## Layout

```text
targets/vectis/
├── prose/
│   ├── prompts/       # guidance, build phases, merge
│   ├── references/    # Crux, shell, design-system, and review depth
│   └── rules/         # VECTIS-* engineering standards
├── src/               # wasm-free engines plus the wasm32 guest shim
├── tests/             # native integration tests
├── templates/         # manifest-driven Crux and shell scaffolds
├── schemas/           # composition, tokens, and assets JSON Schemas
├── assets/            # binary/static scaffold inputs
└── versions.toml      # Crux and host-toolchain pins
```

The component identity is the crate version plus the target world exported through WIT. Resolve-time facts come from the WIT metadata operation; there is no `adapter.yaml`.

## Build behavior

The build order is:

1. Regenerate and validate `composition.yaml`.
2. Generate or update the shared Crux core.
3. Generate tests and run the core verify-repair loop.
4. Generate and verify the iOS shell when selected.
5. Generate and verify the Android shell when selected.
6. Consolidate review findings and outputs.

Keep writer and reviewer contracts in the phase prompts under [`prose/prompts/build/`](prose/prompts/build/). Put reusable Crux, SwiftUI, Compose, design-system, and review depth under [`prose/references/`](prose/references/). Engineering constraints that should produce stable review findings belong in [`prose/rules/`](prose/rules/) with `VECTIS-*` IDs.

When changing platform support, preserve existing platforms and add only the selected platform's writer, validator, templates, and capability coverage. Token or asset changes must flow through both shell design-system integrations without substituting platform glyphs for declared vector or raster assets.

## Rust and template changes

- Keep deterministic validation, materialization, inference, and scaffold behavior in wasm-free modules under `src/`.
- Keep the wasm32-only `mod guest` in `src/lib.rs` limited to the single `adapter::target!(crate::Vectis)` invocation; boundary behavior belongs in the SDK's dispatch functions, operation behavior in the `adapter::Target` impl.
- Update `templates/manifest.yaml` whenever adding or removing scaffold files; orphan or missing entries fail build-time generation.
- Treat generated `src/scaffold/templates/registry.rs` as build output.
- Keep `versions.toml`, templates, host verification, and troubleshooting guidance aligned when changing toolchain pins.

## Tests and verification

Prefer Vectis crate integration tests under `tests/` for public behavior. Keep focused unit matrices only where the root testing policy permits them.

```bash
cargo nextest run -p vectis
cargo clippy -p vectis --all-targets --all-features -- -D warnings
cargo make check
```

For component-boundary changes, also run the composed harness. Live tests are reserved for prompt-quality evaluation.

## Troubleshooting signals

- Incomplete core generation: verify requirements, scenarios, the full Crux `Model` / `Event` / `ViewModel` / `Effect` design, and declared capabilities.
- iOS scaffold drift around UniFFI or cargo-swift: check the active version pins, `--xcframework-name sharedFFI`, and the explicit `make typegen`, `make package`, and `make xcode` gates.
- Android failures: check generated bindings, Java 21 configuration, native library override initialization, and installed Rust Android targets.
- Test mismatch: check whether artifact requirements changed after generation before changing adapter behavior.

## References

- [`prose/prompts/guidance.md`](prose/prompts/guidance.md)
- [`prose/prompts/build.md`](prose/prompts/build.md)
- [`prose/prompts/merge.md`](prose/prompts/merge.md)
- [`prose/references/README.md`](prose/references/README.md)
- [Specify artifact responsibilities](https://github.com/augentic/specify/blob/main/docs/explanation/artifacts.md)
