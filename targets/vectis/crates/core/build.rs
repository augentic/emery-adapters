//! Codegen for the embedded prose registry — the shared
//! `specify-prose-registry` walk over this adapter's `briefs/`,
//! `references/`, and `rules/` trees (resolving the
//! `references/spec-runtime` and `references/agent-teams.md` symlinks
//! inline), emitting the sorted `DOCS` table `src/registry.rs` includes.
//! The `rules/` tree rides along because the build brief's review flow
//! cites the Vectis rule overlay (`VECTIS-006` render-by-kind,
//! `VECTIS-007` scaffold immutability, `VECTIS-009` lint suppression)
//! by path, so the shelf must be able to serve them.

use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let adapter_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("core crate sits at <adapter>/crates/core under the adapter root");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    if let Err(err) =
        specify_prose_registry::emit(adapter_root, &["briefs", "references", "rules"], &out_dir)
    {
        panic!("vectis prose registry codegen failed: {err}");
    }
}
