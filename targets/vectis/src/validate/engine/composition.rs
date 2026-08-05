//! `validate composition` — schema validation, structural-identity,
//! sibling auto-invoke (tokens / assets), and cross-artifact reference
//! resolution.
//!
//! [`structural_identity`] is shared with `validate layout` and the
//! `infer` verb; `refs` resolves references, `catalog` owns the
//! component catalog contract, `finding` the typed finding they emit.

mod catalog;
mod finding;
mod refs;
pub mod structural_identity;
mod test_ids;

use std::path::Path;

use serde_json::{Value, json};

pub use self::finding::Finding;
pub use self::structural_identity::{
    Skeleton, build_group_skeleton, check_structural_identity, fingerprint, skeleton_to_json,
};
pub use self::test_ids::{
    check_test_ids, collect_test_id_values, is_kebab_test_id, kebab_to_maestro_key,
};
use super::paths::{discover_artifact, discover_catalog, resolve_default_path};
use super::run_inner;
use super::shared::{composition_validator, parse_yaml_file};
use crate::validate::ValidateMode;
use crate::validate::error::VectisError;

pub(super) fn validate(path: Option<&Path>) -> Result<Value, VectisError> {
    let target =
        path.map_or_else(|| resolve_default_path(ValidateMode::Composition), Path::to_path_buf);

    if path.is_none() && !target.exists() {
        return Ok(json!({
            "mode": "composition",
            "status": "skipped",
            "reason": format!("no composition.yaml discoverable (looked at {}); core-only projects carry none", target.display()),
            "errors": [],
            "warnings": [],
        }));
    }

    let source = std::fs::read_to_string(&target).map_err(|err| VectisError::InvalidProject {
        message: format!("composition.yaml not readable at {}: {err}", target.display()),
    })?;

    let mut errors: Vec<Finding> = Vec::new();
    let mut warnings: Vec<Finding> = Vec::new();
    let mut results: Vec<Value> = Vec::new();

    match serde_saphyr::from_str::<Value>(&source) {
        Ok(instance) => {
            let validator = composition_validator()?;
            for err in validator.iter_errors(&instance) {
                errors.push(Finding::new(err.instance_path().to_string(), err.to_string()));
            }

            // The schema's `oneOf` ensures only one of `screens` /
            // `delta` is present at a time.
            if let Some(screens) = instance.get("screens") {
                check_structural_identity(screens, "/screens", &mut errors);
                check_test_ids(screens, "/screens", &mut errors);
            }
            if let Some(delta) = instance.get("delta") {
                check_structural_identity(delta, "/delta", &mut errors);
                check_test_ids(delta, "/delta", &mut errors);
            }

            // `tokens` runs before `assets` so `results` matches the
            // `validate all` dispatch order.
            let tokens_sibling = discover_artifact(&target, ValidateMode::Tokens);
            let assets_sibling = discover_artifact(&target, ValidateMode::Assets);

            if let Some(tokens_path) = &tokens_sibling {
                let report = run_inner(ValidateMode::Tokens, tokens_path)?;
                results.push(json!({
                    "mode": ValidateMode::Tokens.as_str(),
                    "report": report,
                }));
            }
            if let Some(assets_path) = &assets_sibling {
                let report = run_inner(ValidateMode::Assets, assets_path)?;
                results.push(json!({
                    "mode": ValidateMode::Assets.as_str(),
                    "report": report,
                }));
            }

            // Reference resolution catches "composition references a
            // name absent from tokens.yaml / assets.yaml"; the
            // auto-invoke above catches "the sibling manifest is
            // itself structurally broken".
            if let Some(tokens_path) = &tokens_sibling
                && let Some(tokens_value) = parse_yaml_file(tokens_path)
            {
                refs::resolve_token_references(&instance, &tokens_value, &mut errors);
            }
            if let Some(assets_path) = &assets_sibling
                && let Some(assets_value) = parse_yaml_file(assets_path)
            {
                refs::resolve_asset_references(&instance, &assets_value, &mut errors);
            }

            // Unlike tokens/assets, the catalog has no sibling
            // validator — report read/parse failures explicitly.
            if let Some(catalog_path) = &discover_catalog(&target) {
                match catalog::parse_catalog_file(catalog_path) {
                    Ok(catalog_value) => {
                        catalog::check_catalog_cross_references(
                            &instance,
                            &catalog_value,
                            &mut errors,
                            &mut warnings,
                        );
                    }
                    Err(message) => {
                        errors.push(Finding::new("", message));
                    }
                }
            }
        }
        Err(err) => {
            errors.push(Finding::new("", format!("invalid YAML: {err}")));
        }
    }

    let mut envelope = json!({
        "mode": ValidateMode::Composition.as_str(),
        "path": target.display().to_string(),
        "errors": finding::to_values(errors),
        "warnings": finding::to_values(warnings),
    });
    // Only emit `results` when we actually folded something in.
    if !results.is_empty()
        && let Value::Object(ref mut map) = envelope
    {
        map.insert("results".to_string(), Value::Array(results));
    }

    Ok(envelope)
}
