# Core-Writer Preservation Rules

**When to read this**: open this file at the start of every Update Mode run, and again before final diff review (step U8). These ten rules govern what the writer is allowed to touch when editing an existing Crux core, and they are the contract behind Update Mode's "minimum collateral change" promise.

In update mode, minimize collateral changes. Follow these rules:

1. **Never regenerate a file from scratch.** Always make targeted edits to existing files. The only exception is creating an entirely new file (e.g., a new custom capability module that did not exist before).
2. **Preserve helper functions** that serve unchanged spec requirements. Do not rename, refactor, or move them unless the spec change requires it.
3. **Preserve test utilities** -- factory functions (e.g., `make_item`), setup helpers (e.g., `seeded_model`), and test infrastructure. Update them only if the types they construct changed.
4. **Preserve code organization** -- section header comments (e.g., `// ── Domain types ──`), module structure, and blank-line grouping.
5. **Preserve `ffi.rs`** unless the App type name changed (which changes the `Bridge<AppType>` generic parameter).
6. **Preserve custom capability modules** unless the spec changes their operation types or API contract.
7. **Preserve `clippy.toml` and `rust-toolchain.toml`** unless a newly added capability introduces duplicate transitive crates or requires new build targets.
8. **Preserve `Cargo.lock`** -- do not delete or manually edit it. Let `cargo` update it when dependencies change.
9. **Preserve doc comments and code comments** on unchanged items.
10. **Never add or preserve inline lint suppressions.** Do not introduce `#[allow(...)]` or `#[expect(...)]` in `shared/src/**/*.rs`, and remove any stale suppressions encountered during repair. Fix the underlying structure (extract helpers, split match arms, use const anchors, narrow types) so `cargo clippy --all-targets -- -D warnings` passes without inline suppressions. Crate-level `[workspace.lints.clippy]` allows in `Cargo.toml` are scaffold-owned and out of scope for writers.
