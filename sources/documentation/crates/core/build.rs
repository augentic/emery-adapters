//! Codegen for the embedded prose registry — the shared
//! `specify-prose-registry` walk over this adapter's `briefs/` and
//! `references/` trees (resolving the `references/spec-runtime` symlink
//! inline), emitting the sorted `DOCS` table `src/registry.rs` includes.

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
        specify_prose_registry::emit(adapter_root, &["briefs", "references"], &out_dir)
    {
        panic!("documentation prose registry codegen failed: {err}");
    }
}
