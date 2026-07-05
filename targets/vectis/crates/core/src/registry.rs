//! Embedded prose registry: every brief, reference, and rule document
//! the vectis adapter ships, compiled in by `build.rs` through the
//! shared `specify-prose-registry` codegen.
//!
//! Documents are keyed by adapter-relative path (`briefs/build.md`,
//! `briefs/build/ios/write.md`, `references/hard-rules-core.md`,
//! `rules/VECTIS-006-asset-render-by-kind.md`, …) — the walk is
//! recursive, so the nested per-platform build sub-briefs embed under
//! their full paths. The `references/spec-runtime` and
//! `references/agent-teams.md` symlinks are resolved at build time, so
//! their documents appear under their symlink-name paths with the
//! shared runtime content inlined. The guest's MCP shelf serves this
//! registry as `doc://` resources, and the operation template reads
//! brief bodies from it for prompt assembly.

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
/// (`build.rs` embeds the full `briefs/`, `references/`, and `rules/`
/// trees), so a miss means the adapter tree and the core disagree and
/// the crate must not limp on.
#[must_use]
pub fn body(path: &str) -> &'static str {
    specify_guest_kit::registry::body(DOCS, path)
}
