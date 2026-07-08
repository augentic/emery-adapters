//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; symlinks resolve at build time.
//!
//! The `rules/` tree rides along so the component carries its own rule
//! overlay pack, pinned to the adapter version.

fn main() {
    prose::emit_adapter(&["prompts", "references", "rules"]);
}
