//! Illustration and photo materialize integration tests (RFC-46 R46-S18).

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

const TRIANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path fill="#010203" d="M12 2L2 22h20z"/>
</svg>"##;

#[test]
fn vector_exports_exist() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();
    fs::write(design.join("assets/onboarding-hero.svg"), TRIANGLE).unwrap();

    let yaml = r#"version: 1
assets:
  onboarding-hero:
    kind: vector
    role: illustration
    alt: "Hero"
    source: assets/onboarding-hero.svg
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize().args(["assets"]).arg(&assets_path).assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));

    let ios_2x = design.join("assets/exports/ios/onboarding-hero.imageset/onboarding-hero@2x.png");
    let ios_3x = design.join("assets/exports/ios/onboarding-hero.imageset/onboarding-hero@3x.png");
    let android_mdpi = design.join("assets/exports/android/drawable-mdpi/onboarding_hero.png");
    let android_xxxhdpi =
        design.join("assets/exports/android/drawable-xxxhdpi/onboarding_hero.png");
    assert!(ios_2x.is_file() && ios_3x.is_file());
    assert!(android_mdpi.is_file() && android_xxxhdpi.is_file());

    // Re-homed from `src/materialize/illustrations.rs`: per-scale @2x/@3x and
    // per-density mdpi/xxxhdpi raster dimensions.
    let img_2x = ImageReader::open(&ios_2x).unwrap().decode().unwrap();
    assert_eq!((img_2x.width(), img_2x.height()), (48, 48));
    let img_3x = ImageReader::open(&ios_3x).unwrap().decode().unwrap();
    assert_eq!((img_3x.width(), img_3x.height()), (72, 72));
    let img_mdpi = ImageReader::open(&android_mdpi).unwrap().decode().unwrap();
    assert_eq!((img_mdpi.width(), img_mdpi.height()), (24, 24));
    let img_xxxhdpi = ImageReader::open(&android_xxxhdpi).unwrap().decode().unwrap();
    assert_eq!((img_xxxhdpi.width(), img_xxxhdpi.height()), (96, 96));
}

const FIGMA_CLIP_ILLUSTRATION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 160" width="240" height="160">
  <defs>
    <clipPath id="clip"><path d="M0 0h240v160H0z"/></clipPath>
  </defs>
  <g clip-path="url(#clip)">
    <rect width="240" height="160" fill="#AABBCC"/>
  </g>
</svg>"##;

#[test]
fn figma_style_exports_exist() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets")).unwrap();
    fs::write(design.join("assets/onboarding-hero.svg"), FIGMA_CLIP_ILLUSTRATION).unwrap();

    let yaml = r#"version: 1
assets:
  onboarding-hero:
    kind: vector
    role: illustration
    alt: "Hero"
    source: assets/onboarding-hero.svg
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize().args(["assets"]).arg(&assets_path).assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));

    let ios_2x = design.join("assets/exports/ios/onboarding-hero.imageset/onboarding-hero@2x.png");
    let ios_3x = design.join("assets/exports/ios/onboarding-hero.imageset/onboarding-hero@3x.png");
    let android_mdpi = design.join("assets/exports/android/drawable-mdpi/onboarding_hero.png");
    let android_xxxhdpi =
        design.join("assets/exports/android/drawable-xxxhdpi/onboarding_hero.png");
    assert!(ios_2x.is_file() && ios_3x.is_file());
    assert!(android_mdpi.is_file() && android_xxxhdpi.is_file());

    let normalized = value["normalized"].as_array().expect("normalized");
    assert!(normalized.iter().any(|entry| entry["asset_id"] == "onboarding-hero"));
}

// Re-homed from `src/materialize/raster_copy.rs`: copy-only `role: photo`
// per-density masters for both the ios imageset (`@2x`) and the android
// drawable-density (`mdpi`) branches, asserting byte-identical copies. Running
// without `--platform` materializes both platform slots.
#[test]
fn photo_copies_density_slots() {
    let tmp = tempdir().unwrap();
    let design = tmp.path().join("design-system");
    fs::create_dir_all(design.join("assets/android")).unwrap();

    let ios_src = design.join("assets/hero@2x.png");
    image::RgbaImage::from_pixel(48, 48, image::Rgba([9, 8, 7, 255])).save(&ios_src).unwrap();
    let android_src = design.join("assets/android/hero-mdpi.png");
    image::RgbaImage::from_pixel(24, 24, image::Rgba([9, 8, 7, 255])).save(&android_src).unwrap();

    let yaml = r#"version: 1
assets:
  hero:
    kind: raster
    role: photo
    alt: "Photo"
    sources:
      ios:
        2x: assets/hero@2x.png
      android:
        mdpi: assets/android/hero-mdpi.png
"#;
    let assets_path = design.join("assets.yaml");
    fs::write(&assets_path, yaml).unwrap();

    let assert = vectis_materialize().arg("assets").arg(&assets_path).assert().success();
    let value = parse_json(&assert.get_output().stdout);
    assert_eq!(value["errors"].as_array().map(Vec::len), Some(0));

    let ios_export = design.join("assets/exports/ios/hero.imageset/hero@2x.png");
    assert!(ios_export.is_file());
    assert_eq!(fs::read(&ios_export).unwrap(), fs::read(&ios_src).unwrap());

    let android_export = design.join("assets/exports/android/drawable-mdpi/hero.png");
    assert!(android_export.is_file(), "android density slot copied");
    assert_eq!(fs::read(&android_export).unwrap(), fs::read(&android_src).unwrap());
}
