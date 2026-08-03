## 0.12.0

Unreleased

### Added

- Vectis: open-GAP inventiveness contract — default stub-faithful for unspecified scenarios; same-build B′ closure of build-editable surfaces (`spec.md` scenario body, `design.md` TBD, composition `# GAP`) only when a grounded destination exists; LOG-010 review check (`important`, default `code-fix`).
- Eval: `vectis-open-gap-fab` build case — My Lists–shaped FAB with unspecified activation + grounded `Page::NewList` pressure; pass criteria and consumer Wasm desk-check in the [case README](examples/eval/cases/vectis-open-gap-fab/README.md).

### Changed

- Vectis: LOG-007 scoped to input-validation / adversarial gaps on otherwise specified actions (not navigation inventiveness); core review Logic specialist and non-mechanical set cover LOG-001..010; UNI-004 dedupe range bumped accordingly.

### Desk-check (unreleased Wasm)

```bash
cargo make adapter vectis   # or: cargo build -p vectis --target wasm32-wasip2 --release
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
