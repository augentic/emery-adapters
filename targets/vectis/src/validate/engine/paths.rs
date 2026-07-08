//! Default-path resolver ([`resolve_default_path`]) and cross-artifact
//! discovery ([`discover_artifact`]).
//!
//! Both walk up from a starting path looking for `.specify/` and
//! resolve against the embedded defaults at
//! [`EMBEDDED_ARTIFACT_PATHS`].

use std::path::{Path, PathBuf};

use crate::validate::ValidateMode;

/// Embedded default paths for the four `vectis validate` modes — the
/// canonical cascade: slice-local files first, then project-level
/// inputs or the merged composition baseline.
///
/// Inner-array order is resolution order; the first existing file
/// wins. The role label exists for parity with the schema YAML; only
/// the templates are consumed. `<name>` expands against
/// `.specify/slices/<dir>/` (alphabetical first match).
const EMBEDDED_ARTIFACT_PATHS: &[(&str, &[(&str, &str)])] = &[
    (
        "layout",
        &[
            ("change_local", ".specify/slices/<name>/layout.yaml"),
            ("project", "design-system/layout.yaml"),
        ],
    ),
    (
        "tokens",
        &[
            ("change_local", ".specify/slices/<name>/tokens.yaml"),
            ("project", "design-system/tokens.yaml"),
        ],
    ),
    (
        "assets",
        &[
            ("change_local", ".specify/slices/<name>/assets.yaml"),
            ("project", "design-system/assets.yaml"),
        ],
    ),
    (
        "composition",
        &[
            ("change_local", ".specify/slices/<name>/composition.yaml"),
            ("baseline", ".specify/specs/composition.yaml"),
        ],
    ),
];

/// Resolve the default path for `validate <mode>` when no `[path]`
/// positional was supplied. Falls through to a fixed canonical path
/// when nothing exists, so the caller's read error names the most
/// operator-friendly path.
pub(super) fn resolve_default_path(mode: ValidateMode) -> PathBuf {
    resolve_default_path_with_root(mode, &default_project_root())
}

/// Default project root for omitted `[path]` positionals: the host's
/// `PROJECT_DIR` when set (WASI invocations), else the `.specify/`
/// root above CWD, else CWD.
pub(super) fn default_project_root() -> PathBuf {
    if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(project_dir);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_project_root(&cwd).unwrap_or(cwd)
}

/// Resolve a per-mode default path against an explicit project root.
///
/// When no candidate exists, returns the *last* candidate considered;
/// with an empty candidate list, falls back to the embedded canonical
/// name under `<root>/`.
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
/// Returns `Some(path)` only when an existing file is found.
///
/// Resolution order: (1) same directory as `start` — the change-local
/// case, plus standalone usage without a Specify project layout;
/// (2) the embedded canonical cascade against the project root.
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

/// Filename half of the canonical-default template; in lock-step with
/// [`canonical_default_template`].
fn canonical_filename_for_key(key: &str) -> &'static str {
    match key {
        "layout" => "layout.yaml",
        "tokens" => "tokens.yaml",
        "assets" => "assets.yaml",
        _ => "composition.yaml",
    }
}

/// Walk up from `start` (or its parent when `start` is a file) to the
/// directory containing `.specify/` — the project root, *not*
/// `.specify/` itself.
#[must_use]
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut cursor =
        if start.is_dir() { start.to_path_buf() } else { start.parent()?.to_path_buf() };
    loop {
        if cursor.join(".specify").is_dir() {
            return Some(cursor);
        }
        if !cursor.pop() {
            return None;
        }
    }
}

/// Locate the operator-curated component catalog at
/// `.specify/design-system/components.yaml` under the project root.
/// `None` when absent (the catalog is opt-in).
#[must_use]
pub fn discover_catalog(start: &Path) -> Option<PathBuf> {
    let project_root = find_project_root(start)?;
    let path = project_root.join(".specify/design-system/components.yaml");
    path.is_file().then_some(path)
}

/// Map a [`ValidateMode`] to its `artifacts:` map key.
/// `ValidateMode::All` has no per-mode key and returns `None`.
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

/// Last-resort fallback template: the project / baseline location,
/// *not* the change-local one, so error messages stay
/// operator-friendly.
fn canonical_default_template(key: &str) -> &'static str {
    match key {
        "layout" => "design-system/layout.yaml",
        "tokens" => "design-system/tokens.yaml",
        "assets" => "design-system/assets.yaml",
        _ => ".specify/specs/composition.yaml",
    }
}

/// Expand a `paths.<role>` template against `project_root`.
///
/// `<name>` is substituted with each directory under
/// `.specify/slices/` (sorted alphabetically). Templates without
/// `<name>` resolve to a single absolute path.
pub fn expand_path_template(template: &str, project_root: &Path) -> Vec<PathBuf> {
    if !template.contains("<name>") {
        return vec![project_root.join(template)];
    }
    let slices_dir = project_root.join(".specify/slices");
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
