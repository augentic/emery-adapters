## 0.13.0

Unreleased

### Changed

- Extract-only re-seam (ADR-0008 / ADR-0009): each source adapter exports `extract` + `metadata` only. Survey prompts, the target axis (`vectis`, `omnia`, `contracts`), and the extra extract sources (`captures`, `screenshots`) are deleted from the live tree — retrieve them at tag `v1`.
- Engine crates import under their package names (`emery_adapter::`, `emery_prose::`). Until an engine release tag carries the extract-only SDK, `[patch.crates-io]` fetches `augentic/emery` by git (path patches stay commented for sibling co-development).
- The graded live eval is a public-contract client of the shipped `emery` binary: one `specify` per case over the built components, `emery show spec` for grading, mechanical CC-05 / CC-06 grading, dated scorecard. Operator-invoked, never CI. Catalog: `orders-docs`, `omnia-r9k`.

### Requires

- Engine revision that exports the extract-only `emery-adapter` / `emery-prose` crates under those import names (the git pin in the root `Cargo.toml`, currently emery `main`). The first adapter train publish waits on an engine release tag.

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
