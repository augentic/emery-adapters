//! iOS asset-catalog root discovery (exemplar layout).

use std::path::{Path, PathBuf};

/// Returns every `iOS/<App>/Assets.xcassets` directory present under `project_dir`.
///
/// Matches the vectis-exemplar / `XcodeGen` layout: the catalog lives directly under
/// the app target folder (not under a filesystem `Resources/` directory).
#[must_use]
pub fn ios_xcassets_roots(project_dir: &Path) -> Vec<PathBuf> {
    let ios_root = project_dir.join("iOS");
    let Ok(entries) = std::fs::read_dir(&ios_root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path().join("Assets.xcassets"))
        .filter(|path| path.is_dir())
        .collect()
}
