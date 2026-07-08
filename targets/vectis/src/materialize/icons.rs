//! Icon vector materialization — SVG to iOS PDF imageset and Android VD XML.

mod android;
mod ios;
mod pdf;

use std::path::Path;

use serde_json::{Value, json};
use usvg::Tree;

use crate::materialize::paths::{
    Platform, export_layout, ios_imageset_dir, resolve_under_assets_dir,
};
use crate::materialize::svg::parse_vector_svg;
use crate::materialize::{MaterializeFilter, MaterializeSink, matches_only};

/// Materialize every in-scope `role: icon` / `role: decorative` vector entry.
pub fn materialize_icon_vectors(
    assets_dir: &Path, assets: &serde_json::Map<String, Value>, platforms: &[String],
    filter: &MaterializeFilter<'_>, sink: &mut MaterializeSink<'_>,
) {
    for (asset_id, entry) in assets {
        if !matches_only(asset_id, filter.only) {
            continue;
        }
        if !is_icon_vector_entry(entry) {
            continue;
        }
        let Some(source_rel) = entry.get("source").and_then(Value::as_str) else {
            continue;
        };
        let source_path = assets_dir.join(source_rel);
        let svg_bytes = match std::fs::read(&source_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                sink.errors.push(asset_error(
                    asset_id,
                    &format!("source not readable at {source_rel}: {err}"),
                ));
                continue;
            }
        };

        let parsed = match parse_vector_svg(&svg_bytes, asset_id) {
            Ok(parsed) => parsed,
            Err(message) => {
                sink.errors.push(asset_error(asset_id, &message));
                continue;
            }
        };

        if let Some(report) = &parsed.normalization {
            crate::materialize::push_normalization_entry(
                sink.normalized,
                asset_id,
                &report.transforms,
            );
        }

        for platform_name in platforms {
            if let Some(pin) = active_platform_pin(entry, platform_name, assets_dir) {
                sink.skipped_pins.push(json!({
                    "asset_id": asset_id,
                    "platform": platform_name,
                    "pin": pin,
                }));
                continue;
            }

            let Some(platform) = Platform::parse(platform_name) else {
                continue;
            };
            let Some(layout) = export_layout(
                entry.get("role").and_then(Value::as_str).unwrap_or("icon"),
                "vector",
                platform,
                asset_id,
            ) else {
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
                Ok(written) => sink.materialized.extend(written),
                Err(message) => sink.errors.push(asset_error(asset_id, &message)),
            }
        }
    }
}

fn materialize_for_platform(
    tree: &Tree, asset_id: &str, platform: Platform, assets_dir: &Path,
    layout: &crate::materialize::paths::ExportLayout, dry_run: bool,
) -> Result<Vec<Value>, String> {
    let mut written = Vec::new();
    match platform {
        Platform::Ios => {
            let imageset_dir = resolve_under_assets_dir(assets_dir, &ios_imageset_dir(asset_id));
            if dry_run {
                for artifact in &layout.artifacts {
                    written.push(materialized_entry(asset_id, platform, artifact));
                }
                return Ok(written);
            }
            ios::write_imageset(tree, asset_id, &imageset_dir, dry_run)
                .map_err(|err| format!("asset `{asset_id}`: iOS export failed: {err}"))?;
            for artifact in &layout.artifacts {
                written.push(materialized_entry(asset_id, platform, artifact));
            }
        }
        Platform::Android => {
            let xml_rel = layout.pin.as_str();
            let xml_path = resolve_under_assets_dir(assets_dir, xml_rel);
            let drawable_name = xml_path.file_stem().and_then(|s| s.to_str()).unwrap_or(asset_id);
            if dry_run {
                for artifact in &layout.artifacts {
                    written.push(materialized_entry(asset_id, platform, artifact));
                }
                return Ok(written);
            }
            android::write_vector_drawable(tree, drawable_name, &xml_path)
                .map_err(|err| format!("asset `{asset_id}`: Android export failed: {err}"))?;
            for artifact in &layout.artifacts {
                written.push(materialized_entry(asset_id, platform, artifact));
            }
        }
    }
    Ok(written)
}

fn is_icon_vector_entry(entry: &Value) -> bool {
    let role = entry.get("role").and_then(Value::as_str);
    let kind = entry.get("kind").and_then(Value::as_str);
    matches!(role, Some("icon" | "decorative")) && kind == Some("vector")
}

pub(crate) fn active_platform_pin(
    entry: &Value, platform: &str, assets_dir: &Path,
) -> Option<String> {
    let pin = entry.get("sources")?.get(platform)?.as_str()?;
    let path = assets_dir.join(pin);
    path.exists().then(|| pin.to_string())
}

pub(crate) fn materialized_entry(asset_id: &str, platform: Platform, path: &str) -> Value {
    json!({
        "asset_id": asset_id,
        "platform": platform.as_str(),
        "path": path,
    })
}

pub(crate) fn asset_error(asset_id: &str, message: &str) -> Value {
    json!({
        "path": format!("/assets/{asset_id}"),
        "message": message,
    })
}
