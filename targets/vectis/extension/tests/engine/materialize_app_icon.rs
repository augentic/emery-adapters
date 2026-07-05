//! App-icon materialize integration tests (RFC-46 R46-S19 / R46-S20).

use std::fs;

use assert_cmd::Command;
use image::ImageReader;
use serde_json::Value;
use tempfile::tempdir;

fn vectis_materialize() -> Command {
    let mut cmd = Command::cargo_bin("vectis").expect("vectis binary");
    cmd.arg("materialize");
    cmd
}

fn parse_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("json output")
}

const SQUARE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <rect width="1024" height="1024" fill="#445566"/>
</svg>"##;

#[test]
fn ios_exports_exist() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();
    fs::write(design.join("assets/app-icon.svg"), SQUARE_SVG).unwrap();

    let yaml = r#"version: 1
app-icon: app-icon
assets:
  app-icon:
    kind: vector
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.svg
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize()
        .args(["assets", "--platform", "ios"])
        .arg(&assets_path)
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));

    let appiconset = design.join("assets/exports/ios/app-icon/AppIcon.appiconset");
    let png = appiconset.join("AppIcon.png");
    let contents = appiconset.join("Contents.json");
    assert!(png.is_file() && contents.is_file());

    let parsed: Value = serde_json::from_slice(&fs::read(&contents).unwrap()).unwrap();
    let images = parsed["images"].as_array().expect("images array");
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["filename"], "AppIcon.png");
    assert_eq!(images[0]["idiom"], "universal");
    assert_eq!(images[0]["platform"], "ios");
    assert_eq!(images[0]["size"], "1024x1024");

    let img = ImageReader::open(&png).unwrap().decode().unwrap();
    assert_eq!(img.width(), 1024);
    assert_eq!(img.height(), 1024);

    let updated = fs::read_to_string(&assets_path).unwrap();
    assert!(updated.contains("sources:"));
    assert!(updated.contains("ios: assets/exports/ios/app-icon/AppIcon.appiconset"));
}

#[test]
fn android_exports_exist() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();
    fs::write(design.join("assets/app-icon.svg"), SQUARE_SVG).unwrap();

    let yaml = r#"version: 1
app-icon: app-icon
assets:
  app-icon:
    kind: vector
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.svg
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize()
        .args(["assets", "--platform", "android"])
        .arg(&assets_path)
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));

    let root = design.join("assets/exports/android/app-icon");
    assert!(root.join("mipmap-anydpi-v26/ic_launcher.xml").is_file());
    assert!(root.join("mipmap-anydpi-v26/ic_launcher_round.xml").is_file());
    assert!(root.join("values/ic_launcher_background.xml").is_file());
    assert!(root.join("drawable-xxxhdpi/ic_launcher_foreground.png").is_file());
    assert!(root.join("mipmap-xxxhdpi/ic_launcher.png").is_file());

    let launcher = fs::read_to_string(root.join("mipmap-anydpi-v26/ic_launcher.xml")).unwrap();
    assert!(launcher.contains("adaptive-icon"));
    assert!(launcher.contains("ic_launcher_foreground"));

    let background = fs::read_to_string(root.join("values/ic_launcher_background.xml")).unwrap();
    assert!(background.contains("ic_launcher_background"));
}

const TRANSPARENT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <circle cx="512" cy="512" r="400" fill="#445566"/>
</svg>"##;

#[test]
fn transparent_raster_exports_exist() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();

    let png_path = design.join("assets/app-icon.png");
    let mut img = image::RgbaImage::from_pixel(1024, 1024, image::Rgba([0, 0, 0, 0]));
    img.put_pixel(512, 512, image::Rgba([40, 50, 60, 200]));
    img.save(&png_path).unwrap();

    let yaml = r#"version: 1
app-icon: app-icon
assets:
  app-icon:
    kind: raster
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.png
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize().args(["assets"]).arg(&assets_path).assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));

    let png = design.join("assets/exports/ios/app-icon/AppIcon.appiconset/AppIcon.png");
    assert!(png.is_file());
    let exported = image::open(&png).unwrap().to_rgba8();
    assert!(exported.pixels().all(|pixel| pixel[3] == 255));

    let normalized = value["normalized"].as_array().expect("normalized");
    let entry = normalized.iter().find(|e| e["asset_id"] == "app-icon").expect("entry");
    let transforms = entry["transforms"].as_array().expect("transforms");
    assert!(transforms.iter().any(|t| t == "composited-transparent-background"));
}

#[test]
fn transparent_svg_exports_exist() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();
    fs::write(design.join("assets/app-icon.svg"), TRANSPARENT_SVG).unwrap();

    let yaml = r#"version: 1
app-icon: app-icon
assets:
  app-icon:
    kind: vector
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.svg
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize().args(["assets"]).arg(&assets_path).assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));

    let png = design.join("assets/exports/ios/app-icon/AppIcon.appiconset/AppIcon.png");
    assert!(png.is_file());
    let exported = image::open(&png).unwrap().to_rgba8();
    assert!(exported.pixels().all(|pixel| pixel[3] == 255));

    assert!(
        design.join("assets/exports/android/app-icon/mipmap-xxxhdpi/ic_launcher.png").is_file()
    );

    let normalized = value["normalized"].as_array().expect("normalized");
    let entry = normalized.iter().find(|e| e["asset_id"] == "app-icon").expect("entry");
    let transforms = entry["transforms"].as_array().expect("transforms");
    assert!(transforms.iter().any(|t| t == "composited-transparent-background"));
}

#[test]
fn ios_rejects_small_raster() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();

    let png_path = design.join("assets/app-icon.png");
    let img = image::RgbaImage::from_pixel(512, 512, image::Rgba([1, 2, 3, 255]));
    img.save(&png_path).unwrap();

    let yaml = r#"version: 1
assets:
  app-icon:
    kind: raster
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.png
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize()
        .args(["assets", "--platform", "ios"])
        .arg(&assets_path)
        .assert()
        .failure();
    let value = parse_json(&assert.get_output().stdout);
    let errors = value["errors"].as_array().expect("errors");
    assert!(errors.iter().any(|entry| {
        entry["message"].as_str().unwrap_or("").contains("assets-app-icon-source-invalid")
    }));
}

#[test]
fn android_rejects_small_raster() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();

    let png_path = design.join("assets/app-icon.png");
    let img = image::RgbaImage::from_pixel(512, 512, image::Rgba([1, 2, 3, 255]));
    img.save(&png_path).unwrap();

    let yaml = r#"version: 1
assets:
  app-icon:
    kind: raster
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.png
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize()
        .args(["assets", "--platform", "android"])
        .arg(&assets_path)
        .assert()
        .failure();
    let value = parse_json(&assert.get_output().stdout);
    let errors = value["errors"].as_array().expect("errors");
    assert!(errors.iter().any(|entry| {
        entry["message"].as_str().unwrap_or("").contains("assets-app-icon-source-invalid")
    }));
}

// Re-homed from `src/materialize/app_icon.rs`: a pinned ios/android app-icon
// export already on disk is skipped (no re-materialize), reported via
// `skipped_pins`. The app-icon pin-skip branch is distinct from the
// icon-vector one in `materialize.rs::materialize_skips_pinned_platform…`.
#[test]
fn skips_pinned_ios_export() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    let appiconset = design.join("assets/exports/ios/app-icon/AppIcon.appiconset");
    fs::create_dir_all(&appiconset).unwrap();
    fs::write(
        appiconset.join("Contents.json"),
        r#"{"images":[{"filename":"AppIcon.png","idiom":"universal","platform":"ios","size":"1024x1024"}],"info":{"version":1,"author":"xcode"}}"#,
    )
    .unwrap();
    fs::write(design.join("assets/app-icon.svg"), SQUARE_SVG).unwrap();

    let yaml = r#"version: 1
assets:
  app-icon:
    kind: vector
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.svg
    sources:
      ios: assets/exports/ios/app-icon/AppIcon.appiconset
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize()
        .args(["assets", "--platform", "ios"])
        .arg(&assets_path)
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);
    assert!(value["materialized"].as_array().is_some_and(Vec::is_empty));
    assert!(value["skipped_pins"].as_array().is_some_and(|arr| !arr.is_empty()));
}

#[test]
fn skips_pinned_android_export() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    let export_root = design.join("assets/exports/android/app-icon");
    fs::create_dir_all(export_root.join("mipmap-anydpi-v26")).unwrap();
    fs::write(export_root.join("mipmap-anydpi-v26/ic_launcher.xml"), "<adaptive-icon/>").unwrap();
    fs::write(design.join("assets/app-icon.svg"), SQUARE_SVG).unwrap();

    let yaml = r#"version: 1
assets:
  app-icon:
    kind: vector
    role: app-icon
    alt: "App icon"
    source: assets/app-icon.svg
    sources:
      android: assets/exports/android/app-icon
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize()
        .args(["assets", "--platform", "android"])
        .arg(&assets_path)
        .assert()
        .success();
    let value = parse_json(&assert.get_output().stdout);
    assert!(value["materialized"].as_array().is_some_and(Vec::is_empty));
    assert!(value["skipped_pins"].as_array().is_some_and(|arr| !arr.is_empty()));
}
