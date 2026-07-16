//! Shared primitives: lazy-compiled validators and the JSON-Pointer /
//! YAML helpers every per-mode handler reuses.
//!
//! The embedded schema sources themselves live in [`crate::schema_source`];
//! this module owns only the lazy `OnceLock` validators built from them.

use std::path::Path;
use std::sync::OnceLock;

use jsonschema::Validator;
use serde_json::Value;

use crate::schema_source::{ASSETS_SCHEMA_SOURCE, COMPOSITION_SCHEMA_SOURCE, TOKENS_SCHEMA_SOURCE};
use crate::validate::error::VectisError;

static TOKENS_VALIDATOR: OnceLock<Result<Validator, String>> = OnceLock::new();

static ASSETS_VALIDATOR: OnceLock<Result<Validator, String>> = OnceLock::new();

static COMPOSITION_VALIDATOR: OnceLock<Result<Validator, String>> = OnceLock::new();

/// Compile the embedded tokens schema once and re-use the validator.
///
/// # Errors
///
/// Returns [`VectisError::Internal`] if the embedded JSON is
/// unparseable or the schema fails to compile.
pub fn tokens_validator() -> Result<&'static Validator, VectisError> {
    lazy_validator(&TOKENS_VALIDATOR, TOKENS_SCHEMA_SOURCE, "tokens.schema.json")
}

/// Compile the embedded assets schema once and re-use the validator.
///
/// # Errors
///
/// Returns [`VectisError::Internal`] when the embedded JSON is
/// unparseable or the schema fails to compile.
pub fn assets_validator() -> Result<&'static Validator, VectisError> {
    lazy_validator(&ASSETS_VALIDATOR, ASSETS_SCHEMA_SOURCE, "assets.schema.json")
}

/// Compile the embedded composition schema once and re-use the
/// validator. Shared between `layout` mode and `composition` mode.
///
/// # Errors
///
/// Returns [`VectisError::Internal`] when the embedded JSON is
/// unparseable or the schema fails to compile.
pub fn composition_validator() -> Result<&'static Validator, VectisError> {
    lazy_validator(&COMPOSITION_VALIDATOR, COMPOSITION_SCHEMA_SOURCE, "composition.schema.json")
}

fn lazy_validator(
    cell: &'static OnceLock<Result<Validator, String>>, source: &'static str, name: &'static str,
) -> Result<&'static Validator, VectisError> {
    let entry = cell.get_or_init(|| {
        let schema: Value = serde_json::from_str(source)
            .map_err(|err| format!("embedded {name} is not JSON: {err}"))?;
        jsonschema::validator_for(&schema)
            .map_err(|err| format!("embedded {name} failed to compile: {err}"))
    });
    match entry {
        Ok(validator) => Ok(validator),
        Err(message) => Err(VectisError::Internal {
            message: message.clone(),
        }),
    }
}

/// Read `path` and parse it as YAML into a [`serde_json::Value`].
#[must_use]
pub fn parse_yaml_file(path: &Path) -> Option<Value> {
    let source = std::fs::read_to_string(path).ok()?;
    serde_saphyr::from_str::<Value>(&source).ok()
}

/// Escape a JSON Pointer reference token (`~` → `~0`, `/` → `~1`).
pub(super) fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}
