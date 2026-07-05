//! Embedded prose registry: every brief and reference document the
//! screenshots adapter ships, compiled in by `build.rs` through the
//! shared `specify-prose-registry` codegen.
//!
//! Documents are keyed by adapter-relative path (`briefs/survey.md`,
//! `references/spec-runtime/reconciliation.md`, …). The
//! `references/spec-runtime` symlink is resolved at build time, so its
//! documents appear under their symlink-name paths with the shared runtime
//! content inlined. The guest's MCP shelf serves this registry as `doc://`
//! resources, and the operations read brief bodies from it for prompt
//! assembly.

pub use specify_guest_kit::registry::Doc;

include!(concat!(env!("OUT_DIR"), "/registry_docs.rs"));

/// Every embedded document, sorted by adapter-relative path.
#[must_use]
pub fn docs() -> &'static [Doc] {
    DOCS
}

/// Look up one document by its adapter-relative path.
#[must_use]
pub fn doc(path: &str) -> Option<&'static Doc> {
    specify_guest_kit::registry::find(DOCS, path)
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
    specify_guest_kit::registry::body(DOCS, path)
}
