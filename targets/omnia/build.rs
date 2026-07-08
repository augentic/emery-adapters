//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; symlinks resolve at build time.
//!
//! The `rules/` tree rides along because the build prompt's review phase
//! cites the Omnia rule overlay by path, so the references server must serve it.

fn main() {
    prose::emit_core(&["prompts", "references", "rules"]);
}
