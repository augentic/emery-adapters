## 0.9.0

Unreleased

### Compatibility

```text
engine 0.35.x  ↔  adapters 0.9.x  (WIT emery:adapter@0.1.0, floor ≥ 0.35.0)
```

Requires engine **v0.35.0**. Aligns with bare local-first adapter resolution (`emery adapter update` / pull-latest); no embedded train auto-pin.

### Changed

* Engine pin advances to **`tag = "v0.35.0"`**; every adapter `emery_floor` rises to **0.35.0**.
* Eval / wasm operator scripts use `emery --debug` (and eval's `--debug` argv) instead of a fixed `RUST_LOG` env filter; wasm README documents `--debug` / `--quiet` over ambient `RUST_LOG`.
* Omnia git pins and supply-chain imports refresh with the engine bump.

**Full Changelog**: https://github.com/augentic/emery-adapters/compare/v0.8.0...v0.9.0

---

Release notes for previous releases can be found on the respective release branches of the repository.

<!-- ARCHIVE_START -->
* [0.9.x](https://github.com/augentic/emery-adapters/blob/release-0.9.0/RELEASES.md)
* [0.8.x](https://github.com/augentic/emery-adapters/blob/release-0.8.0/RELEASES.md)
* [0.7.x](https://github.com/augentic/emery-adapters/blob/release-0.7.0/RELEASES.md)
* [0.5.x](https://github.com/augentic/emery-adapters/blob/release-0.5.0/RELEASES.md)
