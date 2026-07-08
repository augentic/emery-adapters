//! `validate tokens` — schema validation against the embedded tokens
//! schema. No cross-artifact checks; tokens.yaml is leaf-shaped.

use std::path::Path;

use serde_json::{Value, json};

use super::paths::resolve_default_path;
use super::shared::tokens_validator;
use crate::validate::ValidateMode;
use crate::validate::error::VectisError;

/// Validate `tokens.yaml` against the embedded tokens schema.
///
/// Path resolution: the explicit `[path]` positional, else the
/// `artifacts.tokens` cascade (see [`super::paths`]).
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the resolved file is
/// unreadable, and [`VectisError::Internal`] if the embedded schema
/// fails to compile.
pub(super) fn validate(path: Option<&Path>) -> Result<Value, VectisError> {
    let target = path.map_or_else(|| resolve_default_path(ValidateMode::Tokens), Path::to_path_buf);

    let source = std::fs::read_to_string(&target).map_err(|err| VectisError::InvalidProject {
        message: format!("tokens.yaml not readable at {}: {err}", target.display()),
    })?;

    let mut errors: Vec<Value> = Vec::new();
    match serde_saphyr::from_str::<Value>(&source) {
        Ok(instance) => {
            let validator = tokens_validator()?;
            for err in validator.iter_errors(&instance) {
                errors.push(json!({
                    "path": err.instance_path().to_string(),
                    "message": err.to_string(),
                }));
            }
        }
        Err(err) => {
            errors.push(json!({
                "path": "",
                "message": format!("invalid YAML: {err}"),
            }));
        }
    }

    Ok(json!({
        "mode": ValidateMode::Tokens.as_str(),
        "path": target.display().to_string(),
        "errors": errors,
        "warnings": Vec::<Value>::new(),
    }))
}
