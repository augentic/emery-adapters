//! Canonical-to-export asset conversion — the materialize library.

pub(crate) mod app_icon;
pub(crate) mod icons;
pub(crate) mod illustrations;
pub(crate) mod paths;
mod raster_copy;
pub(crate) mod render;
mod rgba;
pub(crate) mod svg;
pub(crate) mod yaml_pins;

use std::path::{Path, PathBuf};

use app_icon::materialize_app_icons;
use icons::materialize_icon_vectors;
use illustrations::materialize_illustration_vectors;
use raster_copy::materialize_photo_rasters;
use serde_json::{Value, json};
use yaml_pins::{apply_auto_pins, atomic_yaml_write, collect_auto_pins, serialise_yaml};

use crate::VectisError;
use crate::validate::engine::resolve_default_path_with_root;
use crate::validate::{ValidateMode, find_project_root};

/// Materialize targets, one per asset domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterializeCommand {
    /// Convert canonical asset masters into per-platform exports.
    Assets(AssetsArgs),
}

/// Arguments for one `materialize assets` run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetsArgs {
    /// Path to `assets.yaml`. Defaults to the design-system cascade.
    pub path: Option<PathBuf>,

    /// Platform filter (`ios`, `android`). Defaults to both.
    pub platform: Option<Vec<String>>,

    /// Report planned writes without creating files or auto-writing pins.
    pub dry_run: bool,

    /// Limit materialization to these asset ids.
    pub only: Option<Vec<String>>,
}

/// Dispatch a [`MaterializeCommand`].
///
/// # Errors
/// Returns [`VectisError::InvalidProject`] when the resolved `assets.yaml`
pub fn run(command: &MaterializeCommand) -> Result<Value, VectisError> {
    match command {
        MaterializeCommand::Assets(args) => run_assets(args),
    }
}

fn run_assets(args: &AssetsArgs) -> Result<Value, VectisError> {
    let path = resolve_assets_path(args.path.as_deref());
    if !path.is_file() {
        return Err(VectisError::InvalidProject {
            message: format!("assets.yaml not readable at {}", path.display()),
        });
    }

    let platforms = resolve_platform_filter(args.platform.as_deref())?;
    let source = std::fs::read_to_string(&path).map_err(VectisError::from)?;

    let mut materialized = Vec::new();
    let mut skipped_pins = Vec::new();
    let mut errors = Vec::new();
    let mut normalized = Vec::new();

    let mut instance = match serde_saphyr::from_str::<Value>(&source) {
        Ok(value) => value,
        Err(err) => {
            errors.push(json!({
                "path": "",
                "message": format!("invalid YAML: {err}"),
            }));
            return Ok(build_summary(
                &path,
                args.dry_run,
                &platforms,
                &materialized,
                &skipped_pins,
                &errors,
                &normalized,
            ));
        }
    };

    if let Some(assets) = instance.get("assets").and_then(Value::as_object) {
        let assets_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let filter = MaterializeFilter {
            dry_run: args.dry_run,
            only: args.only.as_deref(),
        };
        let mut sink = MaterializeSink {
            materialized: &mut materialized,
            skipped_pins: &mut skipped_pins,
            errors: &mut errors,
            normalized: &mut normalized,
        };
        materialize_icon_vectors(assets_dir, assets, &platforms, &filter, &mut sink);
        materialize_illustration_vectors(assets_dir, assets, &platforms, &filter, &mut sink);
        materialize_photo_rasters(
            assets_dir,
            assets,
            &platforms,
            &filter,
            sink.materialized,
            sink.errors,
        );
        materialize_app_icons(assets_dir, assets, &platforms, &filter, &mut sink);
    }

    if !args.dry_run
        && let Some(assets) = instance.get("assets").and_then(Value::as_object)
    {
        let pins = collect_auto_pins(&materialized, assets);
        if !pins.is_empty() {
            apply_auto_pins(&mut instance, &pins);
            let yaml = serialise_yaml(&instance)?;
            atomic_yaml_write(&path, &yaml)?;
        }
    }

    Ok(build_summary(
        &path,
        args.dry_run,
        &platforms,
        &materialized,
        &skipped_pins,
        &errors,
        &normalized,
    ))
}

fn resolve_assets_path(path: Option<&Path>) -> PathBuf {
    if let Some(p) = path {
        if p.is_absolute() {
            return p.to_path_buf();
        }
        // Host prepare and WASI invocations set PROJECT_DIR; anchor explicit
        // relative paths there. Native CLI without PROJECT_DIR stays cwd-relative
        // (matching `validate assets` positional handling).
        if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|value| !value.is_empty())
        {
            return PathBuf::from(project_dir).join(p);
        }
        return p.to_path_buf();
    }
    let root = materialize_project_root();
    resolve_default_path_with_root(ValidateMode::Assets, &root)
}

fn materialize_project_root() -> PathBuf {
    if let Some(project_dir) = std::env::var_os("PROJECT_DIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(project_dir);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_project_root(&cwd).unwrap_or(cwd)
}

fn resolve_platform_filter(platforms: Option<&[String]>) -> Result<Vec<String>, VectisError> {
    let Some(tokens) = platforms else {
        return Ok(vec!["ios".into(), "android".into()]);
    };

    if tokens.is_empty() {
        return Ok(vec!["ios".into(), "android".into()]);
    }

    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        let normalized = token.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if normalized != "ios" && normalized != "android" {
            return Err(VectisError::InvalidProject {
                message: format!("unknown platform filter {token:?} (expected ios and/or android)"),
            });
        }
        if !out.iter().any(|existing| existing == &normalized) {
            out.push(normalized);
        }
    }

    if out.is_empty() { Ok(vec!["ios".into(), "android".into()]) } else { Ok(out) }
}

/// Per-run filters shared by materialize funnels.
#[derive(Debug, Clone, Copy)]
pub struct MaterializeFilter<'a> {
    /// When true, report planned writes without creating files or pins.
    pub dry_run: bool,
    /// When set, limit materialization to these asset ids.
    pub only: Option<&'a [String]>,
}

/// Mutable materialize run outputs accumulated across funnels.
#[derive(Debug)]
pub struct MaterializeSink<'a> {
    /// Written export paths per asset and platform.
    pub materialized: &'a mut Vec<Value>,
    /// Platform pins skipped because an operator export already exists.
    pub skipped_pins: &'a mut Vec<Value>,
    /// Per-asset conversion failures.
    pub errors: &'a mut Vec<Value>,
    /// SVG normalization transforms applied during the run.
    pub normalized: &'a mut Vec<Value>,
}

/// When `only` is set, restrict materialization to the listed asset ids.
pub(crate) fn matches_only(asset_id: &str, only: Option<&[String]>) -> bool {
    only.is_none_or(|ids| ids.iter().any(|candidate| candidate == asset_id))
}

fn build_summary(
    path: &Path, dry_run: bool, platforms: &[String], materialized: &[Value],
    skipped_pins: &[Value], errors: &[Value], normalized: &[Value],
) -> Value {
    let mut summary = json!({
        "command": "materialize assets",
        "path": path.display().to_string(),
        "dry_run": dry_run,
        "platforms": platforms,
        "materialized": materialized,
        "skipped_pins": skipped_pins,
        "errors": errors,
    });
    if !normalized.is_empty()
        && let Value::Object(ref mut map) = summary
    {
        map.insert("normalized".to_string(), Value::Array(normalized.to_vec()));
    }
    summary
}

/// Append one `normalized[]` envelope entry when transforms were applied.
pub(crate) fn push_normalization_entry(
    normalized: &mut Vec<Value>, asset_id: &str, transforms: &[&str],
) {
    if transforms.is_empty() {
        return;
    }
    normalized.push(json!({
        "asset_id": asset_id,
        "transforms": transforms,
    }));
}
