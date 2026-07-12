//! Embeds every markdown document under the adapter's `prose/` tree as
//! the sorted `DOCS` table `src/registry.rs` includes; symlinks resolve
//! at build time.
//!
//! `prose/rules/` rides along so the component carries its own rule
//! overlay pack, pinned to the adapter version.

fn main() {
    prose::emit();
}
