//! Deterministic base-repo scaffold for Omnia guest workspaces.
//!
//! Writes the standard tooling files (cargo-make, deny, cargo-vet,
//! GitHub workflows, toolchain, editor config) from the templates
//! embedded via `templates/manifest.yaml`. Fill-only: an existing file
//! is never overwritten, so consumer customizations always stand.

mod templates;

use std::fs;
use std::path::Path;

use templates::registry;

/// Outcome of one [`ensure_missing`] pass.
#[derive(Debug, Default)]
pub struct EnsureReport {
    /// Relative paths written this pass, in manifest order.
    pub written: Vec<&'static str>,
    /// Relative paths already present and left untouched, in manifest order.
    pub skipped: Vec<&'static str>,
}

/// Write every base-repo tooling file absent from `project_root`.
///
/// # Errors
///
/// Returns the first I/O error creating a parent directory or writing a
/// template; files written before the failure stay in place.
pub fn ensure_missing(project_root: &Path) -> std::io::Result<EnsureReport> {
    let mut report = EnsureReport::default();
    for entry in registry::core::ENTRIES {
        let target = project_root.join(entry.target);
        if target.exists() {
            report.skipped.push(entry.target);
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, entry.contents)?;
        report.written.push(entry.target);
    }
    Ok(report)
}
