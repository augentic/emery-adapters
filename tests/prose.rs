//! The embedded corpus of every shipped adapter stays under the 800
//! non-blank-line cap — the one corpus rule nothing else enforces: the
//! embed-time walker already fails the build on a dangling reference link.
//!
//! `foreach_adapter!` is the orphan guard for the adapter set: a new
//! `sources/<name>` component fails to compile here until a test of that
//! name exists.

#![cfg(not(target_arch = "wasm32"))]

use emery_prose::registry::Doc;
use emery_sdk::SourceAdapter as _;

// Every `sources/*` component `crates/test-programs` builds must have a
// matching test here; a new adapter without one fails to compile.
test_programs::foreach_adapter!();

/// Every document of `docs` carries at most 800 non-blank lines.
fn capped(docs: &[Doc]) {
    for doc in docs {
        let lines = doc.body.lines().filter(|line| !line.trim().is_empty()).count();
        assert!(lines <= 800, "{} carries {lines} non-blank lines (cap 800)", doc.path);
    }
}

#[test]
fn intent() {
    capped(intent::Adapter::docs());
}

#[test]
fn documentation() {
    capped(documentation::Adapter::docs());
}

#[test]
fn typescript() {
    capped(typescript::Adapter::docs());
}
