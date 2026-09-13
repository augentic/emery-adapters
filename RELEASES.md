## 0.13.0

Unreleased

### Changed

- Cut to extract-only (ADR-0008 / ADR-0009): each source adapter exports `extract` + `metadata` only. Survey prompts, the target axis (`vectis`, `omnia`, `contracts`), and the extra extract sources (`captures`, `screenshots`) are deleted from the live tree — retrieve them at tag `v1`.
- Engine crates import under their package names (`emery_sdk::`, `emery_prose::`, `emery_adapter::`). Until an engine release tag carries the extract-only SDK, `[patch.crates-io]` fetches `augentic/emery` by git (path patches stay commented for sibling co-development).
- The embedded prose registry moved to `emery-prose`: adapters call `emery_prose::registry!()` and name `emery_prose::registry::Doc`, with `emery-prose` declared twice (runtime for the registry, build-time with the `emit` feature for the walker). The WIT contract lives in the `emery-adapter` contract crate, named for the `emery:adapter` package, as its `source` axis module; the SDK re-exports it, and the `crates/test-programs` guest helpers depend on it directly (`emery_adapter::source::{Source, SourceInput, Evidence, …}`).
- Adapters depend on `emery-sdk` (the adapter SDK's new name) alone: `emery_sdk::source!(crate::Adapter)` at the crate root, `use emery_sdk::{SourceAdapter, Context, Material, Evidence, Error, Model, …}` everywhere else. There is no `emery-source` crate.
- The graded live eval is a public-contract client of the shipped `emery` binary: one `specify` per case over the built components, `emery show spec` for grading, mechanical CC-05 / CC-06 grading, dated scorecard. Operator-invoked, never CI. Catalog: `orders-docs`, `omnia-r9k`.
- Adapters implement the reshaped `SourceAdapter` (engine `emery-sdk`): `const SOURCE` names the source noun, `extract(model, ctx)` takes the input through `Context { adapter_id, input }`, and the provided `Self::evidence(model, ctx, Material::Bound | Material::Prepared(note))` reads the embedded `prompts/extract.md` itself — `EvidenceTurn`, the free `evidence`, `content_note`, `registry::body`, and the SDK's `types` module are gone; the SDK re-exports the contract's types at its root and omnia's `model` module.
- The component rung is rebuilt on omnia's `test-programs` pattern. `crates/test-programs` replaces `examples/caller` and `examples/conformance`: its `build.rs` runs one `omnia_test::build::Components` build into one `gen.rs` — every `sources/*` component as `ADAPTER_<NAME>` + `foreach_adapter!`, every `programs/<group>/<scenario>.rs` guest program as `<GROUP>_<SCENARIO>` + `foreach_<group>!`. The one seam driver (`source/extract`: `metadata`, then `extract` over both `SourceInput` arms expecting gated evidence, or, told an Omnia error code, expecting that refusal class) and the fixture adapters standing in for the one under test (`probe/refusing`, `probe/upstream`, one per WIT `error` arm) are programs; host suites live in the crate under test — each adapter's `tests/source.rs` (`foreach_source!`, asserting what only the host sees of its component), the root `tests/probe.rs` (`foreach_probe!`), and the root `tests/prose.rs` (`foreach_adapter!` over the 800-line corpus cap, which absorbs the per-adapter `tests/registry.rs`). The SDK's spent-budget refusal is no longer re-proved per adapter under the runtime — the SDK's suite owns it, the probes own the arm it crosses. The root is now the tests-only `emery-adapters` package over `crates/*` + `sources/*`; the per-adapter `tests/operations.rs` is `tests/extract.rs`, trimmed to the adapter's own behavior. The graded live eval (`examples/eval`, `make eval`) is removed from the tree pending its recreation as a root example.

### Requires

- Engine revision that exports the extract-only `emery-adapter` / `emery-prose` / `emery-sdk` crates under those import names (the git pin in the root `Cargo.toml`, currently emery `main`). The first adapter train publish waits on an engine release tag.

---

Release notes for previous releases can be found on the respective release branches of the repository.

<!-- ARCHIVE_START -->
* [0.12.x](https://github.com/augentic/emery-adapters/blob/release-0.12.0/RELEASES.md)
* [0.11.x](https://github.com/augentic/emery-adapters/blob/release-0.11.0/RELEASES.md)
* [0.10.x](https://github.com/augentic/emery-adapters/blob/release-0.10.0/RELEASES.md)
* [0.9.x](https://github.com/augentic/emery-adapters/blob/release-0.9.0/RELEASES.md)
* [0.8.x](https://github.com/augentic/emery-adapters/blob/release-0.8.0/RELEASES.md)
* [0.7.x](https://github.com/augentic/emery-adapters/blob/release-0.7.0/RELEASES.md)
* [0.5.x](https://github.com/augentic/emery-adapters/blob/release-0.5.0/RELEASES.md)
