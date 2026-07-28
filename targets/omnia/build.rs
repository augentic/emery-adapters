//! Embeds every markdown document under the adapter's `prose/` tree as
//! the sorted `DOCS` table `src/registry.rs` includes (symlinks resolve
//! at build time).
//!
//! Nothing else happens at build time: the Omnia guest-template contract
//! lives in [`augentic/omnia-exemplar`](https://github.com/augentic/omnia-exemplar)
//! and is read at consumer-build time from the checkout the build's
//! preparation leg places at `target/omnia-exemplar/` (see
//! `src/scaffold.rs`). Adapter compilation is network-free.

fn main() {
    prose::emit("prose");
}
