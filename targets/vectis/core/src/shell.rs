//! Crux shell presence heuristics for Vectis-bound projects.
//!
//! On-disk shell detection and shell-resident launcher icon probes
//! (RFC-46 §6.3) for [`crate::verify`]. `project.yaml.platforms`
//! is the authority for platform *intent*; these heuristics report what
//! is present on disk so build-time scaffolding and the bootstrap
//! `app-icon` gate can decide what work remains.

use std::path::Path;

mod launcher;

pub use launcher::shell_resident_app_icon;

/// Platform strings with on-disk shell interpretations today.
pub const SUPPORTED_SHELL_PLATFORMS: &[&str] = &["core", "ios", "android"];

/// Returns whether a declared platform's shell tree is present under
/// `project_dir`.
///
/// `web`, `desktop`, and unknown platform strings are treated as
/// present (no on-disk interpretation yet).
#[must_use]
pub fn shell_present(project_dir: &Path, platform: &str) -> bool {
    match platform {
        "core" => project_dir.join("shared/src/app.rs").is_file(),
        "ios" => {
            let ios_dir = project_dir.join("iOS");
            ios_dir.is_dir() && has_files_with_extension(&ios_dir, "swift")
        }
        "android" => {
            let android_dir = project_dir.join("Android");
            android_dir.is_dir() && has_files_with_extension(&android_dir, "kt")
        }
        _ => true,
    }
}

fn has_files_with_extension(dir: &Path, ext: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if has_files_with_extension(&path, ext) {
                return true;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some(ext) {
            return true;
        }
    }
    false
}
