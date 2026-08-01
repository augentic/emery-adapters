## 0.10.0

Released 2026-08-01

### Compatibility

```text
engine 0.36.x  ↔  adapters 0.10.x  (WIT emery:adapter@0.1.0, floor ≥ 0.36.0)
```

Requires engine **v0.36.0**.

### Changed

* Engine pin advances to **`tag = "v0.36.0"`**; every adapter `emery_floor` rises to **0.36.0**.
* **Vectis:** greenfield scaffolding is host-side via `references/template-materialize.md` — `vectis::scaffold::materialize` / `sync` CLIs and exit-code helpers are gone; scaffold drift messages point at manual re-copy from `$TEMPLATE_DIR`.
* Omnia / contracts prose and tests drop RFC ticket archaeology; workspace deps refresh (`serde-saphyr` 1.0.0, `jsonschema` 0.49.2, `toml` 1.1.4).

**Full Changelog**: https://github.com/augentic/emery-adapters/compare/v0.9.0...v0.10.0

---

Release notes for previous releases can be found on the respective release branches of the repository.

<!-- ARCHIVE_START -->
* [0.10.x](https://github.com/augentic/emery-adapters/blob/release-0.10.0/RELEASES.md)
* [0.9.x](https://github.com/augentic/emery-adapters/blob/release-0.9.0/RELEASES.md)
* [0.8.x](https://github.com/augentic/emery-adapters/blob/release-0.8.0/RELEASES.md)
* [0.7.x](https://github.com/augentic/emery-adapters/blob/release-0.7.0/RELEASES.md)
* [0.5.x](https://github.com/augentic/emery-adapters/blob/release-0.5.0/RELEASES.md)
