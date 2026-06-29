//! Illustration vector materialization — SVG to per-density PNG exports.

mod android;
mod ios;

use std::path::Path;

use serde_json::{Value, json};

use crate::materialize::icons::{active_platform_pin, asset_error, materialized_entry};
use crate::materialize::paths::{
    Platform, export_layout, ios_imageset_dir, resolve_under_assets_dir,
};
use crate::materialize::svg::parse_vector_svg;
use crate::materialize::{MaterializeFilter, matches_only};

/// Materialize every in-scope `role: illustration` vector entry from `source:`.
pub fn materialize_illustration_vectors(
    assets_dir: &Path, assets: &serde_json::Map<String, Value>, platforms: &[String],
    filter: &MaterializeFilter<'_>, materialized: &mut Vec<Value>, skipped_pins: &mut Vec<Value>,
    errors: &mut Vec<Value>, normalized: &mut Vec<Value>,
) {
    for (asset_id, entry) in assets {
        if !matches_only(asset_id, filter.only) {
            continue;
        }
        if !is_illustration_vector_entry(entry) {
            continue;
        }
        let Some(source_rel) = entry.get("source").and_then(Value::as_str) else {
            continue;
        };
        let source_path = assets_dir.join(source_rel);
        let svg_bytes = match std::fs::read(&source_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                errors.push(asset_error(
                    asset_id,
                    &format!("source not readable at {source_rel}: {err}"),
                ));
                continue;
            }
        };

        let parsed = match parse_vector_svg(&svg_bytes, asset_id) {
            Ok(parsed) => parsed,
            Err(message) => {
                errors.push(asset_error(asset_id, &message));
                continue;
            }
        };

        if let Some(report) = &parsed.normalization {
            crate::materialize::push_normalization_entry(
                normalized,
                asset_id,
                report.transforms.clone(),
            );
        }

        for platform_name in platforms {
            if let Some(pin) = active_platform_pin(entry, platform_name, assets_dir) {
                skipped_pins.push(json!({
                    "asset_id": asset_id,
                    "platform": platform_name,
                    "pin": pin,
                }));
                continue;
            }

            let Some(platform) = Platform::parse(platform_name) else {
                continue;
            };
            let Some(layout) = export_layout("illustration", "vector", platform, asset_id) else {
                continue;
            };

            match materialize_for_platform(
                &parsed.tree,
                asset_id,
                platform,
                assets_dir,
                &layout,
                filter.dry_run,
            ) {
                Ok(written) => {
                    for path in written {
                        materialized.push(materialized_entry(asset_id, platform, &path));
                    }
                }
                Err(message) => errors.push(asset_error(asset_id, &message)),
            }
        }
    }
}

fn materialize_for_platform(
    tree: &usvg::Tree, asset_id: &str, platform: Platform, assets_dir: &Path,
    layout: &crate::materialize::paths::ExportLayout, dry_run: bool,
) -> Result<Vec<String>, String> {
    match platform {
        Platform::Ios => {
            let imageset_dir = resolve_under_assets_dir(assets_dir, &ios_imageset_dir(asset_id));
            if dry_run {
                return Ok(layout.artifacts.clone());
            }
            ios::write_imageset(tree, asset_id, assets_dir, &imageset_dir, dry_run)
        }
        Platform::Android => {
            if dry_run {
                return Ok(layout.artifacts.clone());
            }
            android::write_density_pngs(tree, asset_id, assets_dir, dry_run)
        }
    }
}

fn is_illustration_vector_entry(entry: &Value) -> bool {
    entry.get("role").and_then(Value::as_str) == Some("illustration")
        && entry.get("kind").and_then(Value::as_str) == Some("vector")
}

// `materialize_illustration_vectors` (public) writes the per-scale ios
// imageset PNGs (@2x/@3x) and per-density android drawables (mdpi…xxxhdpi); it
// is exercised end-to-end through the CLI, with the exact rendered dimensions,
// by `tests/engine/materialize_illustrations.rs::materialize_illustration_vector_exports_exist`.
