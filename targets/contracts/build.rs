//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; trees live under `<adapter>/prose/` and
//! symlinks resolve at build time.
//!
//! The `rules/` tree rides along so the component carries its own rule
//! overlay pack, pinned to the adapter version (DECISIONS.md §"Codex
//! ownership flip: shared packs live in the engine").

fn main() {
    prose::emit_adapter(&["prompts", "references", "rules"]);
}
