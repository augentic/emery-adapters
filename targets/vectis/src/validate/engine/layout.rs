//! `validate layout` — schema validation plus unwired-subset
//! enforcement (forbidden wiring keys + `delta`-shape rejection) plus
//! shared structural-identity checks.

use std::path::Path;

use serde_json::{Value, json};

use super::composition::{Finding, check_structural_identity};
use super::paths::resolve_default_path;
use super::shared::{composition_validator, escape_pointer_token};
use crate::validate::ValidateMode;
use crate::validate::error::VectisError;

pub(super) fn validate(path: Option<&Path>) -> Result<Value, VectisError> {
    let target = path.map_or_else(|| resolve_default_path(ValidateMode::Layout), Path::to_path_buf);

    let source = std::fs::read_to_string(&target).map_err(|err| VectisError::InvalidProject {
        message: format!("layout.yaml not readable at {}: {err}", target.display()),
    })?;

    let mut errors: Vec<Value> = Vec::new();
    let warnings: Vec<Value> = Vec::new();

    match serde_saphyr::from_str::<Value>(&source) {
        Ok(instance) => {
            let validator = composition_validator()?;
            for err in validator.iter_errors(&instance) {
                errors.push(json!({
                    "path": err.instance_path().to_string(),
                    "message": err.to_string(),
                }));
            }

            if instance.get("delta").is_some() {
                errors.push(json!({
                    "path": "/delta",
                    "message": "layout.yaml MUST NOT use the `delta` shape (unwired-subset rule); only `screens` documents are permitted. Use composition.yaml for change-local delta artifacts.",
                }));
            }

            // Both walks are scoped to `screens`: other top-level keys
            // never carry wiring per the schema, and skipping `delta:`
            // avoids redundant wiring-key errors after the shape
            // itself was rejected above.
            if let Some(screens) = instance.get("screens") {
                walk_unwired(screens, "/screens", &mut errors);
                let mut identity: Vec<Finding> = Vec::new();
                check_structural_identity(screens, "/screens", &mut identity);
                errors.extend(identity.into_iter().map(Value::from));
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
        "mode": ValidateMode::Layout.as_str(),
        "path": target.display().to_string(),
        "errors": errors,
        "warnings": warnings,
    }))
}

fn walk_unwired(node: &Value, json_path: &str, errors: &mut Vec<Value>) {
    match node {
        Value::Object(map) => {
            for (key, val) in map {
                let child_path = format!("{json_path}/{}", escape_pointer_token(key));
                if let Some(reason) = forbidden_wiring_key(key) {
                    errors.push(json!({
                        "path": child_path,
                        "message": format!(
                            "{reason} -- remove this key from layout.yaml (unwired-subset rule); wiring is added by /spec:define when it produces composition.yaml"
                        ),
                    }));
                }
                walk_unwired(val, &child_path, errors);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                walk_unwired(v, &format!("{json_path}/{i}"), errors);
            }
        }
        _ => {}
    }
}

fn forbidden_wiring_key(key: &str) -> Option<&'static str> {
    match key {
        "maps_to" => Some("`maps_to` is define-owned screen-to-route wiring"),
        "bind" => Some("`bind` is define-owned field binding"),
        "event" => Some("`event` is define-owned event wiring"),
        "error" => Some("`error` is define-owned validation-error wiring"),
        "trigger" => Some("overlay `trigger` is define-owned"),
        _ if key.ends_with("-when") && key.len() > 5 => {
            Some("conditional visual `*-when` keys are define-owned wiring")
        }
        _ => None,
    }
}
