//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; symlinks resolve at build time.
//!
//! The `rules/` tree rides along because the build brief's review flow cites
//! the Vectis rule overlay by path, so the shelf must serve it.

fn main() {
    specify_prose_registry::emit_core(&["briefs", "references", "rules"]);
}
