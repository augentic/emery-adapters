# Structural update checklist

Use when types, modules, or file layout change without (yet) changing behaviour. Compiling module shapes: exemplar adapters under `crates/` — see [`exemplar.md`](../../../exemplar.md). Strategy detail: [`update-patterns.md`](../../../update-patterns.md).

1. Inventory renames / moves from the artifact-vs-code diff.
2. Apply renames with semantics-preserving rewrites (types, modules, `pub use`, imports).
3. Update `$GUEST_PATH/src/lib.rs` imports and router type parameters to match.
4. Re-scan the crate inventory before subtractive / modifying / additive work.
5. `cargo check` must pass before leaving this category — do not interleave behaviour edits.
