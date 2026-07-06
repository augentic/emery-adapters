//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; trees live under `<adapter>/prose/` and
//! symlinks resolve at build time.

fn main() {
    prose::emit_core(&["prompts", "references"]);
}
