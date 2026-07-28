//! Deterministic base-repo scaffold for Omnia guest workspaces.
//!
//! Writes the standard tooling files (cargo-make, deny, cargo-vet,
//! GitHub workflows, toolchain, editor config) from the exemplar
//! `templates/guest/` contract fetched at build time into `OUT_DIR`.
//! Fill-only: an existing file is never overwritten, so consumer
//! customizations always stand.

mod templates;

use std::fs;
use std::path::Path;

use templates::registry;

/// Project-relative path of the scaffolded publish workflow.
pub const PUBLISH_WORKFLOW: &str = ".github/workflows/publish.yaml";

/// Project-relative path of the scaffolded cargo-vet config.
pub const VET_CONFIG: &str = "supply-chain/config.toml";

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
        write_atomic(&target, entry.contents)?;
        report.written.push(entry.target);
    }
    Ok(report)
}

/// Placeholder tokens in the embedded publish workflow template.
///
/// `<UPPER_SNAKE>` tokens in first-appearance order. The build prompts
/// name these tokens; deriving them here keeps the template the single
/// source of truth.
#[must_use]
pub fn publish_placeholders() -> Vec<&'static str> {
    registry::core::ENTRIES
        .iter()
        .find(|entry| entry.target == PUBLISH_WORKFLOW)
        .map(|entry| placeholders(entry.contents))
        .unwrap_or_default()
}

fn placeholders(contents: &'static str) -> Vec<&'static str> {
    let mut found = Vec::new();
    let mut rest = contents;
    while let Some(start) = rest.find('<') {
        rest = &rest[start..];
        let Some(end) = rest.find('>') else { break };
        let token = &rest[..=end];
        let inner = &token[1..token.len() - 1];
        if !inner.is_empty()
            && inner.chars().all(|c| c.is_ascii_uppercase() || c == '_')
            && !found.contains(&token)
        {
            found.push(token);
        }
        rest = &rest[end + 1..];
    }
    found
}

// A fill-only pass never revisits an existing path, so a write left
// half-complete by a crash would be kept forever; temp + rename keeps
// every scaffolded file whole.
fn write_atomic(target: &Path, contents: &str) -> std::io::Result<()> {
    let file_name = target.file_name().map(|name| name.to_string_lossy()).unwrap_or_default();
    let tmp = target.with_file_name(format!(".{file_name}.scaffold-tmp"));
    fs::write(&tmp, contents)?;
    fs::rename(&tmp, target).inspect_err(|_| {
        drop(fs::remove_file(&tmp));
    })
}
