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
        return layout.artifacts.iter().any(|rel| export_artifact_counts(&assets_dir.join(rel)));
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
    entries.filter_map(Result::ok).any(|entry| export_artifact_counts(&entry.path()))
}

fn export_artifact_counts(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if path.file_name().is_some_and(|name| name == "Contents.json") {
        return false;
    }
    if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        return pdf_has_magic(path);
    }
    true
}

fn pdf_has_magic(path: &Path) -> bool {
    use std::io::Read;

    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0_u8; 5];
    matches!(file.read_exact(&mut magic), Ok(())) && magic == *b"%PDF-"
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    // iOS raster imageset needs a materialized file beyond Contents.json.
    #[test]
    fn conventional_matrix() {
        let tmp = tempdir().expect("tempdir");
        let design = tmp.path();

        let xml = design.join("assets/exports/android/drawable/chevron_right.xml");
        std::fs::create_dir_all(xml.parent().expect("parent")).expect("mkdir");
        std::fs::write(&xml, "<vector/>").expect("write");
        let icon = json!({ "role": "icon", "kind": "vector" });
        assert!(conventional_export_exists(design, "chevron-right", "vector", "android", &icon));

        let imageset = design.join("assets/exports/ios/hero.imageset");
        std::fs::create_dir_all(&imageset).expect("mkdir");
        std::fs::write(imageset.join("Contents.json"), "{\"images\":[]}").expect("write json");
        assert!(!imageset_has_materialized_content(&imageset));
        let raster = json!({ "role": "illustration", "kind": "raster" });
        assert!(!conventional_export_exists(design, "hero", "raster", "ios", &raster));
        std::fs::write(imageset.join("hero@3x.png"), b"PNG").expect("write png");
        assert!(conventional_export_exists(design, "hero", "raster", "ios", &raster));

        let checkset = design.join("assets/exports/ios/check.imageset");
        std::fs::create_dir_all(&checkset).expect("mkdir check");
        std::fs::write(checkset.join("Contents.json"), "{\"images\":[]}").expect("write json");
        std::fs::write(checkset.join("check.pdf"), b"1 0 obj\n<< /Type /Catalog >>\nendobj\n")
            .expect("write junk pdf");
        assert!(!imageset_has_materialized_content(&checkset));
        let icon = json!({ "role": "icon", "kind": "vector" });
        assert!(!conventional_export_exists(design, "check", "vector", "ios", &icon));
        std::fs::write(checkset.join("check.pdf"), b"%PDF-1.4\n1 0 obj\n<< >>\nendobj\n")
            .expect("write real pdf");
        assert!(imageset_has_materialized_content(&checkset));
        assert!(conventional_export_exists(design, "check", "vector", "ios", &icon));

        let png = design.join("assets/exports/android/drawable-mdpi/hero.png");
        std::fs::create_dir_all(png.parent().expect("parent")).expect("mkdir");
        std::fs::write(&png, b"PNG").expect("write");
        assert!(conventional_export_exists(design, "hero", "raster", "android", &raster));
    }
}
