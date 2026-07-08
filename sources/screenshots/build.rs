//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; symlinks resolve at build time.

fn main() {
    prose::emit_adapter(&["prompts", "references"]);
}
