//! The embedded prose-registry vocabulary and lookup helpers.
//!
//! Each adapter core's `build.rs` (via `specify-prose-registry`) emits a
//! sorted `DOCS` table of [`Doc`] entries into `$OUT_DIR/registry_docs.rs`;
//! the core `include!`s that table next to a `pub use` of this [`Doc`]
//! type and wraps the lookup helpers below. Documents are keyed by
//! adapter-relative path (`prompts/build.md`,
//! `references/openapi/verifier.md`, …).

/// One embedded reference document.
#[derive(Clone, Copy, Debug)]
pub struct Doc {
    /// Adapter-relative path, e.g. `prompts/build.md`.
    pub path: &'static str,
    /// Full markdown body.
    pub body: &'static str,
}

/// Look up one document by its adapter-relative path. `docs` must be
/// sorted by path (the generated table is).
#[must_use]
pub fn find<'d>(docs: &'d [Doc], path: &str) -> Option<&'d Doc> {
    docs.binary_search_by(|doc| doc.path.cmp(path)).ok().map(|idx| &docs[idx])
}

/// The body of a document the registry is guaranteed to embed.
///
/// # Panics
///
/// Panics when `path` is not in `docs` — a build-time invariant (the
/// codegen embeds the adapter's full prose trees), so a miss means the
/// adapter tree and its core disagree and the crate must not limp on.
#[must_use]
pub fn body(docs: &[Doc], path: &str) -> &'static str {
    find(docs, path)
        .unwrap_or_else(|| panic!("document `{path}` is not embedded in the registry"))
        .body
}

/// Generate an adapter core's `registry` module body over the `DOCS`
/// table its `build.rs` emitted (via `specify_prose_registry::emit_core`).
///
/// Invoke once inside the core's `registry` module:
///
/// ```ignore
/// pub mod registry {
///     specify_guest_kit::embed_registry!();
/// }
/// ```
#[macro_export]
macro_rules! embed_registry {
    () => {
        pub use $crate::registry::Doc;

        include!(concat!(env!("OUT_DIR"), "/registry_docs.rs"));

        /// Every embedded document, sorted by adapter-relative path.
        #[must_use]
        pub fn docs() -> &'static [Doc] {
            DOCS
        }

        /// Look up one document by its adapter-relative path.
        #[must_use]
        pub fn doc(path: &str) -> Option<&'static Doc> {
            $crate::registry::find(DOCS, path)
        }

        /// The body of a document the registry is guaranteed to embed.
        ///
        /// # Panics
        ///
        /// Panics when `path` is not embedded — a build-time invariant, so a
        /// miss means the adapter tree and its core disagree and the crate
        /// must not limp on.
        #[must_use]
        pub fn body(path: &str) -> &'static str {
            $crate::registry::body(DOCS, path)
        }
    };
}
