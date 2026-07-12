//! Lightweight scaffold repair without prepare side effects.

use std::path::Path;

use serde_json::{Value, json};

use crate::VectisError;
use crate::android_scaffold::{scaffold_sync_android_json, sync_android_scaffold_files};
use crate::ios_scaffold::{scaffold_sync_ios_json, sync_ios_scaffold_files};

/// Re-render agent-immutable `iOS/Makefile`, `iOS/project.yml`, and the
/// `.vectis/` scripts from the embedded templates.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the app name cannot be
/// resolved or a file write fails.
pub fn ios(project_root: &Path) -> Result<Value, VectisError> {
    let report = sync_ios_scaffold_files(project_root)?;
    Ok(json!({
        "command": "sync ios-scaffold",
        "project-root": project_root.display().to_string(),
        "scaffold_sync": scaffold_sync_ios_json(&report),
    }))
}

/// Re-render agent-immutable Android assembly Gradle files and Makefile
/// from the embedded templates.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the app name or package
/// cannot be resolved or a file write fails.
pub fn android(project_root: &Path) -> Result<Value, VectisError> {
    let report = sync_android_scaffold_files(project_root)?;
    Ok(json!({
        "command": "sync android-scaffold",
        "project-root": project_root.display().to_string(),
        "scaffold_sync": scaffold_sync_android_json(&report),
    }))
}
