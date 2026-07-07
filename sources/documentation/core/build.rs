//! Embeds the adapter's prose trees as the sorted `DOCS` table
//! `src/registry.rs` includes; symlinks resolve at build time.
//!
//! The `rules/` tree rides along so the component carries its own rule
//! overlay pack, pinned to the adapter version (RFC-66 §"Codex
//! ownership becomes real").

fn main() {
    prose::emit_core(&["prompts", "references", "rules"]);
}
