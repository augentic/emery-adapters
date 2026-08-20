//! Embeds every markdown document under `prose/` (rules included) as
//! the sorted `DOCS` table `src/registry.rs` includes; symlinks
//! resolve at build time.

fn main() {
    emery_prose::emit("prose");
}
