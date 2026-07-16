//! Environment guard pinning the out-of-tree project cache root
//! inside a sandbox, mirroring the engine testkit's `env` module —
//! part of the shared eval-core seam.
//!
//! The trial and scenario runners hold one guard per run so adapter
//! cache reads and writes stay inside the run's own tree: the trial
//! must not read or write the operator's normal cache, and its result
//! must not depend on prior local state.

use std::path::Path;

const CACHE_ENV: &str = "SPECIFY_PROJECT_CACHE";

/// Restores the previous `SPECIFY_PROJECT_CACHE` value on drop.
#[derive(Debug)]
pub struct CacheGuard(Option<std::ffi::OsString>);

impl Drop for CacheGuard {
    #[expect(unsafe_code, reason = "restore the cache-root env var pinned for the run")]
    fn drop(&mut self) {
        // SAFETY: the guard lives in single-purpose processes — one
        // nextest test process, or one sequential `specify-dev eval`
        // phase — so no other thread observes the env mutation for the
        // guard's lifetime.
        unsafe {
            match self.0.take() {
                Some(prev) => std::env::set_var(CACHE_ENV, prev),
                None => std::env::remove_var(CACHE_ENV),
            }
        }
    }
}

/// Pin the out-of-tree project cache root inside `dir` so adapter
/// cache writes are hermetic and cleaned up with the sandbox.
#[must_use]
#[expect(unsafe_code, reason = "pin the cache-root env var into the sandbox")]
pub fn scoped_cache(dir: &Path) -> CacheGuard {
    let prev = std::env::var_os(CACHE_ENV);
    // SAFETY: see `CacheGuard::drop` — single-process isolation.
    unsafe { std::env::set_var(CACHE_ENV, dir.join("project-cache")) };
    CacheGuard(prev)
}
