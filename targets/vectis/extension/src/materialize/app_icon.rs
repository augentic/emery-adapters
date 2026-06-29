//! App-icon materialization — shared launcher canvas and per-platform exports.

mod android;
mod canvas;
mod ios;

use std::path::Path;

pub use canvas::{LAUNCHER_CANVAS_SIZE, LauncherCanvas, decode_to_launcher_canvas};
use serde_json::{Value, json};

use crate::materialize::icons::{active_platform_pin, asset_error, materialized_entry};
use crate::materialize::paths::{Platform, export_layout, resolve_under_assets_dir};
use crate::materialize::{MaterializeFilter, MaterializeSink, matches_only};

/// Materialize `role: app-icon` entries with a canonical `source:` master.
pub fn materialize_app_icons(
    assets_dir: &Path, assets: &serde_json::Map<String, Value>, platforms: &[String],
    filter: &MaterializeFilter<'_>, sink: &mut MaterializeSink<'_>,
) {
    for (asset_id, entry) in assets {
        if !matches_only(asset_id, filter.only) {
            continue;
        }
        if entry.get("role").and_then(Value::as_str) != Some("app-icon") {
            continue;
        }
        let Some(source_rel) = entry.get("source").and_then(Value::as_str) else {
            continue;
        };
        let source_path = assets_dir.join(source_rel);

        let launcher = match decode_to_launcher_canvas(&source_path, source_rel, asset_id) {
            Ok(canvas) => canvas,
            Err(message) => {
                sink.errors.push(asset_error(asset_id, &message));
                continue;
            }
        };

        record_app_icon_normalization(sink.normalized, asset_id, &launcher);

        let kind = entry.get("kind").and_then(Value::as_str).unwrap_or("vector");

        for platform_name in platforms {
            let Some(platform) = Platform::parse(platform_name) else {
                continue;
            };
            if let Some(pin) = active_platform_pin(entry, platform_name, assets_dir) {
                sink.skipped_pins.push(json!({
                    "asset_id": asset_id,
                    "platform": platform_name,
                    "pin": pin,
                }));
                continue;
            }

            let Some(layout) = export_layout("app-icon", kind, platform, asset_id) else {
                continue;
            };

            let result = match platform {
                Platform::Ios => {
                    materialize_ios(asset_id, assets_dir, &layout, &launcher.image, filter.dry_run)
                }
                Platform::Android => materialize_android(
                    asset_id,
                    entry,
                    assets_dir,
                    &layout,
                    &launcher.image,
                    filter.dry_run,
                ),
            };
            match result {
                Ok(written) => sink.materialized.extend(written),
                Err(message) => sink.errors.push(asset_error(asset_id, &message)),
            }
        }
    }
}

fn materialize_ios(
    asset_id: &str, assets_dir: &Path, layout: &crate::materialize::paths::ExportLayout,
    canvas: &image::RgbaImage, dry_run: bool,
) -> Result<Vec<Value>, String> {
    if dry_run {
        return Ok(layout
            .artifacts
            .iter()
            .map(|path| materialized_entry(asset_id, Platform::Ios, path))
            .collect());
    }

    let appiconset_dir = resolve_under_assets_dir(assets_dir, &layout.pin);
    ios::write_appiconset(canvas, &appiconset_dir)
        .map_err(|err| format!("asset `{asset_id}`: iOS app-icon export failed: {err}"))?;

    Ok(layout
        .artifacts
        .iter()
        .map(|path| materialized_entry(asset_id, Platform::Ios, path))
        .collect())
}

fn materialize_android(
    asset_id: &str, entry: &Value, assets_dir: &Path,
    layout: &crate::materialize::paths::ExportLayout, canvas: &image::RgbaImage, dry_run: bool,
) -> Result<Vec<Value>, String> {
    if dry_run {
        return Ok(layout
            .artifacts
            .iter()
            .map(|path| materialized_entry(asset_id, Platform::Android, path))
            .collect());
    }

    let export_root = resolve_under_assets_dir(assets_dir, &layout.pin);
    let background = android::resolve_launcher_background(entry, assets_dir);
    android::write_android_export(canvas, &background, &export_root)
        .map_err(|err| format!("asset `{asset_id}`: Android app-icon export failed: {err}"))?;

    Ok(layout
        .artifacts
        .iter()
        .map(|path| materialized_entry(asset_id, Platform::Android, path))
        .collect())
}

fn record_app_icon_normalization(
    normalized: &mut Vec<Value>, asset_id: &str, launcher: &LauncherCanvas,
) {
    let mut transforms = Vec::new();
    if let Some(report) = &launcher.normalization {
        transforms.extend(report.transforms.iter().copied());
    }
    if launcher.has_transparency {
        transforms.push("composited-transparent-background");
    }
    crate::materialize::push_normalization_entry(normalized, asset_id, &transforms);
}

// The public `materialize_app_icons` funnel is exercised end-to-end through the
// CLI by `tests/engine/materialize_app_icon.rs`: ios + android SVG exports
// (`materialize_app_icon_{ios,android}_exports_exist`), the small-raster
// rejection (`materialize_app_icon_{ios,android}_rejects_small_raster`), and the
// pinned-export skip (`materialize_app_icon_skips_pinned_{ios,android}_export`).
// Tint-token background resolution is unit-covered by
// `app_icon::android`'s `resolve_launcher_background_matrix`, and the
// actool-friendly `Contents.json` layout by `app_icon::ios`'s appiconset test.
