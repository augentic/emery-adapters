//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; symlinks resolve at build time.

fn main() {
    specify_prose_registry::emit_core(&["briefs", "references"]);
}
