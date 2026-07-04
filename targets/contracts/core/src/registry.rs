//! Embedded prose registry: every brief and reference document the
//! contracts adapter ships, compiled in by `build.rs`.
//!
//! Documents are keyed by adapter-relative path (`briefs/build.md`,
//! `references/openapi/verifier.md`, …). The `references/spec-runtime`
//! symlink is resolved at build time, so its documents appear under their
//! symlink-name paths with the shared runtime content inlined. The guest's
//! MCP shelf serves this registry as `doc://` resources, and the operation
//! template reads brief bodies from it for prompt assembly.

/// One embedded reference document.
#[derive(Clone, Copy, Debug)]
pub struct Doc {
    /// Adapter-relative path, e.g. `briefs/build.md`.
    pub path: &'static str,
    /// Full markdown body.
    pub body: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/registry_docs.rs"));

/// Every embedded document, sorted by adapter-relative path.
#[must_use]
pub fn docs() -> &'static [Doc] {
    DOCS
}

/// Look up one document by its adapter-relative path.
#[must_use]
pub fn doc(path: &str) -> Option<&'static Doc> {
    DOCS.binary_search_by(|doc| doc.path.cmp(path)).ok().map(|idx| &DOCS[idx])
}

/// The body of a document the registry is guaranteed to embed.
///
/// # Panics
///
/// Panics when `path` is not in the registry — a build-time invariant
/// (`build.rs` embeds the full `briefs/` and `references/` trees), so a
/// miss means the adapter tree and the core disagree and the crate must
/// not limp on.
#[must_use]
pub fn body(path: &str) -> &'static str {
    doc(path).unwrap_or_else(|| panic!("document `{path}` is not embedded in the registry")).body
}
