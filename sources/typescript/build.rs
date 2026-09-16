//! Embeds every Markdown document under `prose/` as the table `include_prose!` includes.
//!
//! Symlinks resolve at build time, so the shared runtime references ride along.

fn main() {
    emery_prose::emit("prose");
}
