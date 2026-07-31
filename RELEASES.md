## 0.7.0

Released 2026-07-31

### Compatibility

```text
engine 0.33.x  ↔  adapters 0.7.x  (WIT emery:adapter@0.1.0, floor ≥ 0.33.0)
```

Hard cut with engine 0.33: this train speaks the RFC-78 path-first seam. Pin engine git dependencies to `v0.33.0`; earlier hosts and adapter trains are not compatible.

### Added

* Vectis **final-core-verify** build leg after review: re-runs fmt / check / clippy / test under `-D warnings` on `shared/`, then stamps `shared/.vectis/verify.ok` (digest of `shared/src/**/*.rs`). Shell verify / report gates block on a missing or stale stamp.
* Operator wasm examples split into `cargo make wasm-contracts` and `cargo make wasm-omnia-r9k` (replacing the single `wasm-run` path).

### Changed

* **RFC-78 prompt budget.** Target builds take path-first `Payload::Path` inputs and `BuildContext` (bound source names). Omnia drops generation-time `guidance.md` refresh, skips capture replay unless `captures` is bound, and closes with an in-guest standards-review report (no separate report leg). Contracts / omnia / vectis operation tests lock per-leg system-prompt byte budgets.
* Engine pin advances from the deleted `rfc-78` branch to **`tag = "v0.33.0"`**; every adapter `emery_floor` rises to **0.33.0**.
* Vectis vector-icon export keeps fill and stroke separate (stroke-only icons render correctly on iOS PDF and Android vector drawables); iOS asset catalogs resolve under `iOS/<App>/Assets.xcassets/` (exemplar / XcodeGen layout), not `Resources/`.
* Omnia verify-repair and merge preflight use `cargo clippy --all-targets -- -D warnings`; merge prompt is an explicit fail-fast gate.
* Eval / wasm examples rename `EVAL_*` Cursor env vars to `CURSOR_MODEL` / `CURSOR_TIMEOUT_SECS`; adapter publish CI hardens checkout credentials and publish permissions.

**Full Changelog**: https://github.com/augentic/emery-adapters/compare/v0.6.0...v0.7.0

---

Release notes for previous releases can be found on the respective release branches of the repository.

<!-- ARCHIVE_START -->
* [0.7.x](https://github.com/augentic/emery-adapters/blob/release-0.7.0/RELEASES.md)
* [0.5.x](https://github.com/augentic/emery-adapters/blob/release-0.5.0/RELEASES.md)
