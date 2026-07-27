//! Template-relative DX refresh helpers (no embedded re-render).
//!
//! Greenfield trees and pin faithfulness come from `$TEMPLATE_DIR` via
//! [`crate::scaffold::materialize`]. The target guest cannot see a sibling
//! checkout, so the build path does not call these helpers — the host-side
//! agent re-copies drifted DX paths from the template with identity
//! substitution. Embedded `include_str!` re-render is retired.

use std::path::Path;

use serde_json::{Value, json};

use crate::scaffold::materialize::Identity;
use crate::scaffold::{self, materialize};
use crate::{VectisError, android_scaffold, ios_scaffold};

/// Refresh agent-immutable iOS DX files from `$TEMPLATE_DIR`.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when `$TEMPLATE_DIR` cannot be
/// resolved, the consumer identity cannot be derived, or a copy fails.
pub fn ios(project_root: &Path) -> Result<Value, VectisError> {
    let report = refresh_from_template(project_root, materialize::IOS_DX_RELATIVE_PATHS)?;
    Ok(json!({
        "command": "sync ios-scaffold",
        "project-root": project_root.display().to_string(),
        "source": "template-dir",
        "scaffold_sync": {
            "ios": {
                "synced": report.synced,
                "unchanged": report.unchanged,
                "missing_in_template": report.missing_in_template,
            }
        },
    }))
}

/// Refresh agent-immutable Android DX files from `$TEMPLATE_DIR`.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when `$TEMPLATE_DIR` cannot be
/// resolved, the consumer identity cannot be derived, or a copy fails.
pub fn android(project_root: &Path) -> Result<Value, VectisError> {
    let report = refresh_from_template(project_root, materialize::ANDROID_DX_RELATIVE_PATHS)?;
    Ok(json!({
        "command": "sync android-scaffold",
        "project-root": project_root.display().to_string(),
        "source": "template-dir",
        "scaffold_sync": {
            "android": {
                "synced": report.synced,
                "unchanged": report.unchanged,
                "missing_in_template": report.missing_in_template,
            }
        },
    }))
}

struct RefreshReport {
    synced: Vec<String>,
    unchanged: Vec<String>,
    missing_in_template: Vec<String>,
}

fn refresh_from_template(
    project_root: &Path, relative_paths: &[&str],
) -> Result<RefreshReport, VectisError> {
    let template_dir =
        materialize::resolve_dir(project_root).ok_or_else(|| VectisError::InvalidProject {
            message: format!(
                "cannot refresh DX files: $TEMPLATE_DIR not found (clone \
                 https://github.com/augentic/vectis-template.git as {} or set {})",
                materialize::DEFAULT_RELATIVE_DIR,
                materialize::TEMPLATE_DIR_ENV
            ),
        })?;
    let identity = resolve_identity(project_root)?;
    let mut synced = Vec::new();
    let mut unchanged = Vec::new();
    let mut missing_in_template = Vec::new();

    for rel in relative_paths {
        let src = template_dir.join(rel);
        if !src.is_file() {
            missing_in_template.push((*rel).to_string());
            continue;
        }
        let mapped = materialize::map_relative_path(rel, &identity);
        copy_if_changed(
            &src,
            &project_root.join(&mapped),
            &identity,
            &mapped,
            &mut synced,
            &mut unchanged,
        )?;
    }

    Ok(RefreshReport {
        synced,
        unchanged,
        missing_in_template,
    })
}

fn copy_if_changed(
    src: &Path, dest: &Path, identity: &Identity, mapped: &str, synced: &mut Vec<String>,
    unchanged: &mut Vec<String>,
) -> Result<(), VectisError> {
    let bytes = std::fs::read(src).map_err(|err| VectisError::InvalidProject {
        message: format!("read {}: {err}", src.display()),
    })?;
    let expected = match String::from_utf8(bytes) {
        Ok(text) => materialize::substitute_identity(&text, identity).into_bytes(),
        Err(err) => err.into_bytes(),
    };
    if dest.is_file() && std::fs::read(dest).ok().as_deref() == Some(expected.as_slice()) {
        unchanged.push(mapped.to_string());
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| VectisError::InvalidProject {
            message: format!("create {}: {err}", parent.display()),
        })?;
    }
    std::fs::write(dest, expected).map_err(|err| VectisError::InvalidProject {
        message: format!("write {}: {err}", dest.display()),
    })?;
    synced.push(mapped.to_string());
    Ok(())
}

fn resolve_identity(project_root: &Path) -> Result<Identity, VectisError> {
    let app_name = ios_scaffold::resolve_ios_app_name(project_root)
        .or_else(|_| android_scaffold::resolve_android_app_name(project_root))
        .or_else(|_| app_name_from_project_yaml(project_root))?;
    let android_package = android_scaffold::resolve_android_package(project_root, &app_name)
        .unwrap_or_else(|_| scaffold::default_android_package(&app_name));
    Identity::new(app_name, android_package).map_err(|err| VectisError::InvalidProject {
        message: err.to_string(),
    })
}

fn app_name_from_project_yaml(project_root: &Path) -> Result<String, VectisError> {
    let source =
        std::fs::read_to_string(project_root.join(".emery/project.yaml")).map_err(|err| {
            VectisError::InvalidProject {
                message: format!("read project.yaml: {err}"),
            }
        })?;
    let doc: Value =
        serde_saphyr::from_str(&source).map_err(|err| VectisError::InvalidProject {
            message: format!("parse project.yaml: {err}"),
        })?;
    let raw =
        doc.get("name").and_then(Value::as_str).ok_or_else(|| VectisError::InvalidProject {
            message: "project.yaml missing name:".into(),
        })?;
    let pascal: String = raw
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                let mut word = first.to_ascii_uppercase().to_string();
                word.push_str(chars.as_str());
                word
            })
        })
        .collect();
    scaffold::validate_app_name(&pascal)?;
    Ok(pascal)
}
