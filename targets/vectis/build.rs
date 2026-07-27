//! Embeds every markdown document under the adapter's `prose/` tree as
//! the sorted `DOCS` table `src/registry.rs` includes (symlinks resolve
//! at build time).
//!
//! `prose/rules/` rides along because the build prompt's review flow cites
//! the Vectis rule overlay by path, so the references server must serve it.

fn main() {
    prose::emit("prose");
}
