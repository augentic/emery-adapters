## 0.8.0

Unreleased

### Compatibility

```text
engine 0.34.x  ↔  adapters 0.8.x  (WIT emery:adapter@0.1.0, floor ≥ 0.34.0)
```

Requires engine **v0.34.0**. Aligns operator docs and eval/wasm scripts with Gate 1 = first `emery plan execute` (no separate approve).

### Changed

* Engine pin advances to **`tag = "v0.34.0"`**; every adapter `emery_floor` rises to **0.34.0**.
* Docs, eval/wasm examples, and contracts merge prose follow the 0.34 operator surface: Gate 1 is `emery plan execute`, plan entry undo is `emery plan transition --undo`, and conflict detection is `emery slice merge run --conflict-check`.
* Omnia / omnia-cursor git pins refresh (`omnia-cursor` from `augentic/backends` to `augentic/omnia-backends`); supply-chain allow-git and cargo-vet imports follow.

**Full Changelog**: https://github.com/augentic/emery-adapters/compare/v0.7.0...v0.8.0

---

Release notes for previous releases can be found on the respective release branches of the repository.

<!-- ARCHIVE_START -->
* [0.8.x](https://github.com/augentic/emery-adapters/blob/release-0.8.0/RELEASES.md)
* [0.7.x](https://github.com/augentic/emery-adapters/blob/release-0.7.0/RELEASES.md)
* [0.5.x](https://github.com/augentic/emery-adapters/blob/release-0.5.0/RELEASES.md)
