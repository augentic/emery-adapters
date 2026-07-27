//! Default-path resolver ([`resolve_default_path`]) and cross-artifact
//! discovery ([`discover_artifact`]).
//!
//! Both walk up from a starting path looking for `.emery/` and
//! resolve against the embedded defaults at
//! [`EMBEDDED_ARTIFACT_PATHS`].

use std::path::{Path, PathBuf};

use crate::validate::ValidateMode;

const EMBEDDED_ARTIFACT_PATHS: &[(&str, &[(&str, &str)])] = &[
    (
        "layout",
        &[
            ("change_local", ".emery/slices/<name>/layout.yaml"),
            ("project", "design-system/layout.yaml"),
        ],
    ),
    (
        "tokens",
        &[
            ("change_local", ".emery/slices/<name>/tokens.yaml"),
            ("project", "design-system/tokens.yaml"),
        ],
    ),
    (
        "assets",
        &[
            ("change_local", ".emery/slices/<name>/assets.yaml"),
            ("project", "design-system/assets.yaml"),
        ],
    ),
    (
        "composition",
        &[
            ("change_local", ".emery/slices/<name>/composition.yaml"),
            ("baseline", ".emery/specs/composition.yaml"),
        ],
    ),
];

pub(super) fn resolve_default_path(mode: ValidateMode) -> PathBuf {
    resolve_default_path_with_root(mode, &default_project_root())
}

pub(super) fn default_project_root() -> PathBuf {
    if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(project_dir);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_project_root(&cwd).unwrap_or(cwd)
}

/// Resolve a per-mode default path against an explicit project root.
#[must_use]
pub fn resolve_default_path_with_root(mode: ValidateMode, project_root: &Path) -> PathBuf {
    let key = artifact_key_for_mode(mode).unwrap_or("composition");
    let templates = paths_for_key(key);

    let mut last_candidate: Option<PathBuf> = None;
    for template in &templates {
        for resolved in expand_path_template(template, project_root) {
            if resolved.is_file() {
                return resolved;
            }
            last_candidate = Some(resolved);
        }
    }
    last_candidate.unwrap_or_else(|| project_root.join(canonical_default_template(key)))
}

/// Locate a sibling artifact for a caller anchored at `start`.
#[must_use]
pub fn discover_artifact(start: &Path, mode: ValidateMode) -> Option<PathBuf> {
    let key = artifact_key_for_mode(mode)?;

    let filename = canonical_filename_for_key(key);
    if let Some(parent) = start.parent() {
        let local = parent.join(filename);
        if local.is_file() {
            return Some(local);
        }
    }

    let project_root = find_project_root(start)?;
    let templates = paths_for_key(key);

    for template in &templates {
        for resolved in expand_path_template(template, &project_root) {
            if resolved.is_file() {
                return Some(resolved);
            }
        }
    }
    None
}

fn canonical_filename_for_key(key: &str) -> &'static str {
    match key {
        "layout" => "layout.yaml",
        "tokens" => "tokens.yaml",
        "assets" => "assets.yaml",
        _ => "composition.yaml",
    }
}

/// Walk up from `start` (or its parent when `start` is a file) to the
/// directory containing `.emery/` — the project root, *not*
/// `.emery/` itself.
#[must_use]
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cursor =
        if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
    loop {
        if cursor.join(".emery").is_dir() {
            return Some(cursor);
        }
        if !cursor.pop() {
            return None;
        }
    }
}

/// Locate the operator-curated component catalog at
/// `.emery/design-system/components.yaml` under the project root.
/// `None` when absent (the catalog is opt-in).
#[must_use]
pub fn discover_catalog(start: &Path) -> Option<PathBuf> {
    let project_root = find_project_root(start)?;
    let path = project_root.join(".emery/design-system/components.yaml");
    path.is_file().then_some(path)
}

const fn artifact_key_for_mode(mode: ValidateMode) -> Option<&'static str> {
    match mode {
        ValidateMode::Layout => Some("layout"),
        ValidateMode::Composition => Some("composition"),
        ValidateMode::Tokens => Some("tokens"),
        ValidateMode::Assets => Some("assets"),
        ValidateMode::All => None,
    }
}

/// Ordered `paths.<role>` templates for the given artifact `key`,
/// in the embedded resolution order.
#[must_use]
pub fn paths_for_key(key: &str) -> Vec<String> {
    EMBEDDED_ARTIFACT_PATHS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, paths)| paths.iter().map(|(_, t)| (*t).to_string()).collect())
        .unwrap_or_default()
}

fn canonical_default_template(key: &str) -> &'static str {
    match key {
        "layout" => "design-system/layout.yaml",
        "tokens" => "design-system/tokens.yaml",
        "assets" => "design-system/assets.yaml",
        _ => ".emery/specs/composition.yaml",
    }
}

/// Expand a `paths.<role>` template against `project_root`.
pub fn expand_path_template(template: &str, project_root: &Path) -> Vec<PathBuf> {
    if !template.contains("<name>") {
        return vec![project_root.join(template)];
    }
    let slices_dir = project_root.join(".emery/slices");
    let Ok(entries) = std::fs::read_dir(&slices_dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    names.sort();
    names.into_iter().map(|name| project_root.join(template.replace("<name>", &name))).collect()
}
