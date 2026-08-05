## 0.12.0

Unreleased

### Added

- Vectis: canonical UI bindings — composition inline `test_id` projects to `ui-contract/test-ids.yaml` during `emery build`; display strings / fixed errors / seed live under `ui-contract/`; deterministic in-guest verify gates (`canonical-ui-literal-hardcoded`, `canonical-test-id-raw`, `canonical-test-tag-resource-id`, `canonical-test-id-projection-stale`, `canonical-seed-version`). See [`targets/vectis/prose/references/canonical-ui-bindings.md`](targets/vectis/prose/references/canonical-ui-bindings.md).
- Vectis: platform-scoped template materialize — `iOS/` / `Android/` copy only when listed in `project.yaml.platforms`; allowlist also covers `ui-contract/`, `tools/` (`cursor-guard`), and `.cursor/hooks.json`.
- Vectis: open-GAP inventiveness contract — default stub-faithful for unspecified scenarios; same-build B′ closure of build-editable surfaces (`spec.md` scenario body, `design.md` TBD, composition `# GAP`) only when a grounded destination exists; LOG-010 review check (`important`, default `code-fix`).
- Eval: `vectis-open-gap-fab` build case — My Lists–shaped FAB with unspecified activation + grounded `Page::NewList` pressure; pass criteria and consumer Wasm desk-check in the [case README](examples/eval/cases/vectis-open-gap-fab/README.md).

### Changed

- Vectis: LOG-007 scoped to input-validation / adversarial gaps on otherwise specified actions (not navigation inventiveness); core review Logic specialist and non-mechanical set cover LOG-001..010; UNI-004 dedupe range bumped accordingly.

### Requires

- Matching [`augentic/vectis-exemplar`](https://github.com/augentic/vectis-exemplar) checkout that includes `ui-contract/`, `tools/`, and `.cursor/hooks.json`. An older exemplar (`main` before those landed) fails materialize with an actionable missing-shape error — update the sibling clone (or `VECTIS_EXEMPLAR_DIR`) before greenfield builds.
- Sibling [`augentic/emery`](https://github.com/augentic/emery) checkout at `../emery` that includes the local `emery-composition` crate (path workspace dep until emery tags `v0.38.0` and this repo switches the pin to that tag).

### Desk-check (unreleased Wasm)

```bash
cargo build -p vectis --target wasm32-wasip2 --release   # or: cargo make release
emery adapter add target/wasm32-wasip2/release/vectis.wasm
# then: emery slice build my-lists-platform  (todo-app)  and/or
#       cargo make eval vectis-open-gap-fab --restart
```

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
