//! Shared support for the native harness suites: a throw-away project
//! tree with a hermetic project cache.

use std::fs;
use std::path::PathBuf;

use eval::env::CacheGuard;
use tempfile::TempDir;

/// An initialised throw-away project bound to the `omnia` target, with
/// the adapter cache pinned inside the tempdir.
pub struct Project {
    _tmp: TempDir,
    _cache: CacheGuard,
    root: PathBuf,
}

impl Project {
    /// Scaffold the tree: `.specify/{slices,specs}`, `project.yaml`
    /// bound to the linked `omnia` adapter.
    pub fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let root = tmp.path().canonicalize().expect("canonical tempdir");
        let cache = eval::env::scoped_cache(&root);
        for sub in [".specify/slices", ".specify/specs"] {
            fs::create_dir_all(root.join(sub)).expect("mkdir");
        }
        fs::write(root.join(".specify/project.yaml"), "name: demo\nadapter: omnia\nrules: {}\n")
            .expect("write project.yaml");
        Self {
            _tmp: tmp,
            _cache: cache,
            root,
        }
    }

    /// The project root.
    #[must_use]
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
}
