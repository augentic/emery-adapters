//! Conventional committed-export presence for materialization checks.

use std::path::Path;

use serde_json::Value;

use crate::materialize::paths::{Platform, export_layout, kebab_to_snake};

/// Whether a composition-referenced asset has a committed export on
/// disk for `platform` without a `sources.<platform>` pin.
pub fn conventional_export_exists(
    assets_dir: &Path, id: &str, kind: &str, platform: &str, entry: &Value,
) -> bool {
    let role = entry.get("role").and_then(Value::as_str).unwrap_or("");
    if let Some(plat) = Platform::parse(platform)
        && let Some(layout) = export_layout(role, kind, plat, id)
    {
        return layout.artifacts.iter().any(|rel| assets_dir.join(rel).is_file());
    }
    conventional_raster_export_exists(assets_dir, id, kind, platform)
}

fn conventional_raster_export_exists(
    assets_dir: &Path, id: &str, kind: &str, platform: &str,
) -> bool {
    let exports_root = assets_dir.join("assets/exports").join(platform);
    if !exports_root.is_dir() {
        return false;
    }
    match (platform, kind) {
        ("ios", "raster") => {
            let imageset = exports_root.join(format!("{id}.imageset"));
            imageset.is_dir() && imageset_has_materialized_content(&imageset)
        }
        ("android", "raster") => {
            let snake = kebab_to_snake(id);
            for density in ["mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"] {
                if exports_root
                    .join(format!("drawable-{density}"))
                    .join(format!("{snake}.png"))
                    .is_file()
                {
                    return true;
                }
                if exports_root
                    .join(format!("mipmap-{density}"))
                    .join(format!("{snake}.png"))
                    .is_file()
                {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}

/// Path A for vector inventory: canonical `source:` exists on disk.
pub(super) fn vector_source_materializable(assets_dir: &Path, entry: &Value) -> bool {
    entry
        .get("source")
        .and_then(Value::as_str)
        .is_some_and(|source| assets_dir.join(source).is_file())
}

/// Whether an operator-pinned `sources.<platform>` path exists on disk.
pub fn platform_pin_active(entry: &Value, platform: &str, assets_dir: &Path) -> bool {
    let Some(pin) = entry.get("sources").and_then(|s| s.get(platform)).and_then(Value::as_str)
    else {
        return false;
    };
    assets_dir.join(pin).exists()
}

/// Whether a committed launcher `app-icon` export tree exists for `platform`.
#[must_use]
pub fn app_icon_export_exists(assets_dir: &Path, platform: &str) -> bool {
    let root = assets_dir.join(format!("assets/exports/{platform}/app-icon"));
    match platform {
        "ios" => {
            let appiconset = root.join("AppIcon.appiconset");
            appiconset.is_dir()
                && appiconset.join("Contents.json").is_file()
                && directory_has_extension(&appiconset, "png")
        }
        "android" => root.join("mipmap-anydpi-v26/ic_launcher.xml").is_file(),
        _ => false,
    }
}

fn directory_has_extension(dir: &Path, ext: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case(ext))
    })
}

/// Whether an iOS imageset directory carries materialized content.
pub fn imageset_has_materialized_content(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path = entry.path();
        path.is_file() && entry.file_name() != "Contents.json"
    })
}
