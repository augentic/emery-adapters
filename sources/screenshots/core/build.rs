//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; symlinks resolve at build time.

fn main() {
    prose::emit_core(&["prompts", "references"]);
}
