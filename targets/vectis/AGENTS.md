# Vectis Target Adapter - Agent Instructions

These instructions extend the repository root [`AGENTS.md`](../../AGENTS.md) for all work under `targets/vectis/`.

Vectis is a Emery **target adapter**. Its `guidance`, `build`, and `merge` operations generate Crux shared cores, SwiftUI iOS shells, and Kotlin/Jetpack Compose Android shells from synthesised Emery artifacts plus operator-curated design-system inputs.

## Ownership boundaries

- `spec.md` stays behavioral and platform-neutral. `design.md` carries the Crux type system and platform design.
- `tokens.yaml` and `assets.yaml` are operator-curated build inputs; Vectis consumes them but never synthesises them.
- `composition.yaml` is a target build output. Regenerate it from `spec.md` and `design.md`; do not treat it as a Emery synthesis artifact.
- The `screenshots` source adapter owns image-to-layout Evidence. Vectis consumes spatial claims only after synthesis folds them into the canonical artifacts.
- Schemas, asset materializers, validators, and the `scaffold::materialize` allowlist are adapter tooling. **Structure and pins come from a local `$TEMPLATE_DIR` checkout** — not from an in-adapter `templates/` tree or `versions.toml`. Consumer build prompts must stop and report tooling drift rather than invent versions in-band.

## `$TEMPLATE_DIR` (operator prerequisite)

Greenfield and pin refresh require a local clone of [`augentic/vectis-template`](https://github.com/augentic/vectis-template):

| Resolve | Path |
| ------- | ---- |
| Default | `../vectis-template` relative to the **consumer** project root |
| Override | Absolute path in `VECTIS_TEMPLATE_DIR` |

The target guest cannot see a sibling checkout; the build agent materializes on the host via `vectis::scaffold::materialize`. Strip grammar lives only in **`$TEMPLATE_DIR/AGENTS.md`** (never copied into the consumer). Late-capability re-adoption: [`prose/references/template-capabilities.md`](prose/references/template-capabilities.md).

There is **no** adapter-side version registry. Fix pins upstream in `vectis-template`, then re-materialize / re-copy.

## Layout

```text
targets/vectis/
├── prose/
│   ├── prompts/       # guidance, build phases, merge
│   ├── references/    # Crux, shell, design-system, template-capabilities, review
│   └── rules/         # VECTIS-* engineering standards
├── src/               # wasm-free engines plus the wasm32 guest shim
│   └── scaffold/materialize.rs   # allowlisted copy contract (no embedded pins)
├── tests/             # native integration tests (template FS tests skip if missing)
└── schemas/           # composition, tokens, and assets JSON Schemas
```

The component identity is the crate version plus the target world exported through WIT. Resolve-time facts come from the WIT metadata operation; there is no `adapter.yaml`.

## Build behavior

The build order is:

1. Regenerate and validate `composition.yaml`.
2. Generate or update the shared Crux core (greenfield: materialize from `$TEMPLATE_DIR`, then strip).
3. Generate tests and run the core verify-repair loop.
4. Generate and verify the iOS shell when selected (`make build` / stamp `iOS/.vectis/verify.ok`).
5. Generate and verify the Android shell when selected (`make build` / stamp `Android/.vectis/verify.ok`).
6. Consolidate review findings and outputs.

Keep writer and reviewer contracts in the phase prompts under [`prose/prompts/build/`](prose/prompts/build/). Put reusable Crux, SwiftUI, Compose, design-system, and review depth under [`prose/references/`](prose/references/). Engineering constraints that should produce stable review findings belong in [`prose/rules/`](prose/rules/) with `VECTIS-*` IDs.

When changing platform support, preserve existing platforms and add only the selected platform's writer, validator, and capability coverage. Token or asset changes must flow through both shell design-system integrations without substituting platform glyphs for declared vector or raster assets. `web/` in the template remains out of scope for materialize and writers.

## Rust and template changes

- Keep deterministic validation, materialization, inference, and scaffold behavior in wasm-free modules under `src/`.
- Keep the wasm32-only `mod guest` in `src/lib.rs` limited to the single `adapter::target!(crate::Adapter)` invocation; boundary behavior belongs in the SDK's dispatch functions, operation behavior in the `adapter::Target` impl.
- Greenfield allowlist / denylist / identity substitution live in `src/scaffold/materialize.rs` and are tested against a local `vectis-template` checkout — pins are never re-authored in this crate.
- Fix pin or DX drift upstream in `vectis-template`, then re-materialize; do not reintroduce an adapter-side version registry.

## Tests and verification

Prefer Vectis crate integration tests under `tests/` for public behavior. Keep focused unit matrices only where the root testing policy permits them.

```bash
cargo nextest run -p vectis
cargo clippy -p vectis --all-targets --all-features -- -D warnings
cargo make check
```

Materialize FS tests resolve `VECTIS_TEMPLATE_DIR` or `../vectis-template` from the emery-adapters workspace and skip clearly when absent. For component-boundary changes, also run `cargo make wasm-run`. Live tests are reserved for prompt-quality evaluation.

## Troubleshooting signals

- Incomplete core generation: verify requirements, scenarios, the full Crux `Model` / `Event` / `ViewModel` / `Effect` design, and declared capabilities.
- iOS DX drift: re-copy from `$TEMPLATE_DIR` (`iOS/Makefile`, `iOS/project.yml`); regenerate the Xcode project via `make -C iOS generate-project` / `xcodegen`. Verify with `make -C iOS build` (not retired `sim-build`).
- Android failures: check BoltFFI-generated bindings, compatible JDK for the template's JVM targets, native library load via `CoreFfi`, and installed Rust Android targets. Prefer `make -C Android doctor` then `make build`.
- Late-cap wiring: copy the `cap=` strip-unit from `$TEMPLATE_DIR` per [`template-capabilities.md`](prose/references/template-capabilities.md); run `cargo make generate` when `Effect` / facet deps change.
- Test mismatch: check whether artifact requirements changed after generation before changing adapter behavior.

## References

- [`prose/prompts/guidance.md`](prose/prompts/guidance.md)
- [`prose/prompts/build.md`](prose/prompts/build.md)
- [`prose/prompts/merge.md`](prose/prompts/merge.md)
- [`prose/references/template-capabilities.md`](prose/references/template-capabilities.md)
- [`prose/references/README.md`](prose/references/README.md)
- [Emery artifact responsibilities](https://github.com/augentic/emery/blob/main/docs/explanation/artifacts.md)
