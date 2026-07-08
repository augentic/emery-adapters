//! The embedded prose vocabulary and lookup helpers.
//!
//! Each adapter core's `build.rs` (via `prose`) emits a
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

/// The body of a document the embedded table declares, or `None` when
/// `path` is not in `docs`.
///
/// The single body-resolution seam shared by [`body`] and the MCP
/// references server's read path: under the `prose-overlay` feature an on-disk
/// overlay body wins, but the doc *set* is always the embedded table's —
/// the overlay overrides bodies, never entries. Public as the
/// `Option`-returning sibling of the panicking [`body`], for callers
/// that treat a miss as recoverable (the MCP references).
#[must_use]
pub fn resolve(docs: &[Doc], path: &str) -> Option<&'static str> {
    let doc = find(docs, path)?;
    #[cfg(feature = "prose-overlay")]
    if let Some(body) = overlay(path) {
        return Some(body);
    }
    Some(doc.body)
}

/// The overlay body for `path` from `.eval/prose/<path>`, resolved
/// against the process cwd (the shared `"."` preopen in a deployed
/// guest); `None` when the file is absent. An empty overlay file is
/// served as-is by design — `read_to_string` reads to EOF, so a partial
/// read cannot happen. The read body is leaked to preserve the
/// registry's `&'static str` contract — acceptable for this dev-only
/// affordance because the leak is per body-read, bounded by the number
/// of body reads in one operation of a per-call-instantiated guest.
///
/// # Panics
///
/// Panics when the overlay file exists but cannot be read — the overlay
/// must never silently fall back to a body the author is not editing.
#[cfg(feature = "prose-overlay")]
fn overlay(path: &str) -> Option<&'static str> {
    let file = std::path::Path::new(".eval/prose").join(path);
    match std::fs::read_to_string(&file) {
        Ok(body) => Some(body.leak()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => panic!("prose overlay `{}` is unreadable: {err}", file.display()),
    }
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
    resolve(docs, path)
        .unwrap_or_else(|| panic!("document `{path}` is not embedded in the registry"))
}

/// Generate an adapter core's `registry` module body over the `DOCS`
/// table its `build.rs` emitted (via `prose::emit_core`).
///
/// Invoke once inside the core's `registry` module:
///
/// ```ignore
/// pub mod registry {
///     adapter::embed_registry!();
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
