//! Copy-only raster materialization for `role: photo` per-density masters.

use std::path::Path;

use serde_json::{Value, json};

use crate::materialize::icons::{asset_error, materialized_entry};
use crate::materialize::paths::{
    ANDROID_DENSITIES, IOS_RASTER_SCALES, Platform, android_raster_artifact_rel, ios_imageset_dir,
    ios_raster_artifact_rel, ios_raster_filename, resolve_under_assets_dir,
};
use crate::materialize::{MaterializeFilter, matches_only};

/// Copy per-density raster sources into conventional export paths for `role: photo`.
pub fn materialize_photo_rasters(
    assets_dir: &Path, assets: &serde_json::Map<String, Value>, platforms: &[String],
    filter: &MaterializeFilter<'_>, materialized: &mut Vec<Value>, errors: &mut Vec<Value>,
) {
    for (asset_id, entry) in assets {
        if !matches_only(asset_id, filter.only) {
            continue;
        }
        if entry.get("role").and_then(Value::as_str) != Some("photo")
            || entry.get("kind").and_then(Value::as_str) != Some("raster")
        {
            continue;
        }

        for platform_name in platforms {
            let Some(platform) = Platform::parse(platform_name) else {
                continue;
            };
            let Some(density_map) = entry
                .get("sources")
                .and_then(Value::as_object)
                .and_then(|sources| sources.get(platform_name))
                .and_then(Value::as_object)
            else {
                continue;
            };

            match copy_platform_densities(
                asset_id,
                platform,
                assets_dir,
                density_map,
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

fn copy_platform_densities(
    asset_id: &str, platform: Platform, assets_dir: &Path,
    density_map: &serde_json::Map<String, Value>, dry_run: bool,
) -> Result<Vec<String>, String> {
    let mut written = Vec::new();

    match platform {
        Platform::Ios => {
            let mut images = Vec::new();
            for scale in IOS_RASTER_SCALES {
                let Some(source_rel) = density_map.get(*scale).and_then(Value::as_str) else {
                    continue;
                };
                let export_rel = ios_raster_artifact_rel(asset_id, scale);
                written.push(export_rel.clone());
                images.push(json!({
                    "filename": ios_raster_filename(asset_id, scale),
                    "idiom": "universal",
                    "scale": scale,
                }));

                if dry_run {
                    continue;
                }

                copy_file(assets_dir, source_rel, &export_rel)?;
            }

            if images.is_empty() {
                return Ok(written);
            }

            let contents_rel = format!("{}/Contents.json", ios_imageset_dir(asset_id));
            written.push(contents_rel.clone());

            if !dry_run {
                let imageset_dir =
                    resolve_under_assets_dir(assets_dir, &ios_imageset_dir(asset_id));
                std::fs::create_dir_all(&imageset_dir).map_err(|err| err.to_string())?;
                let contents = json!({
                    "images": images,
                    "info": {
                        "author": "vectis",
                        "version": 1
                    }
                });
                let contents_path = resolve_under_assets_dir(assets_dir, &contents_rel);
                std::fs::write(
                    contents_path,
                    serde_json::to_vec_pretty(&contents).expect("contents json"),
                )
                .map_err(|err| err.to_string())?;
            }
        }
        Platform::Android => {
            for density in ANDROID_DENSITIES {
                let Some(source_rel) = density_map.get(*density).and_then(Value::as_str) else {
                    continue;
                };
                let export_rel = android_raster_artifact_rel(asset_id, density);
                written.push(export_rel.clone());

                if dry_run {
                    continue;
                }

                copy_file(assets_dir, source_rel, &export_rel)?;
            }
        }
    }

    Ok(written)
}

fn copy_file(assets_dir: &Path, source_rel: &str, export_rel: &str) -> Result<(), String> {
    let source_path = assets_dir.join(source_rel);
    let dest_path = resolve_under_assets_dir(assets_dir, export_rel);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    std::fs::copy(&source_path, &dest_path)
        .map_err(|err| format!("copy `{source_rel}` → `{export_rel}` failed: {err}"))?;
    Ok(())
}

// `materialize_photo_rasters` (the copy-only `role: photo` funnel) is exercised
// end-to-end through the CLI by
// `tests/engine/materialize_illustrations.rs::materialize_photo_copies_density_slots`,
// which materializes both the ios imageset (`@2x`) and the android
// drawable-density (`mdpi`) slots and asserts byte-identical copies, so the
// `src` unit was re-homed.
