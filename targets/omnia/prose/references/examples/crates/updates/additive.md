# Additive update checklist

Use when the slice adds a new operation to an existing crate. Compiling shapes: exemplar `crates/tally-connector` (minimal) and `crates/gtfs-adapter` (multi-handler) — see [`exemplar.md`](../../../exemplar.md). Strategy detail: [`update-patterns.md`](../../../update-patterns.md).

1. Confirm structural / subtractive / modifying categories are already applied.
2. Add request/response types and a zero-sized `Operation<P>` with the narrowest capability bounds.
3. Register the operation on the root guest's Axum router and/or exact messaging topic match in `src/lib.rs` (or the consumer's existing guest package if it is not root-packaged).
4. Extend workspace / crate `Cargo.toml` only for new dependencies.
5. Add `tests/` coverage mapped to the new `REQ-*` IDs; extend the mock provider traits if needed.
6. Document new config keys in the guest `.env.example`.
7. `cargo check` / `cargo test` in `$CRATE_PATH` before leaving the category.
