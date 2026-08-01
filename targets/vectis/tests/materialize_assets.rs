//! `materialize assets` operation-surface tests over the public
//! [`vectis::materialize::run`] entry.

use std::path::Path;

use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Value, json};
use tempfile::tempdir;
use vectis::VectisError;
use vectis::materialize::{AssetsArgs, MaterializeCommand, run as materialize_run};

const CHECKMARK: &str = r##"<svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
<g clip-path="url(#clip0)">
<path d="M5 12L10 17L20 7" stroke="#1F2937" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
</g>
<defs>
<clipPath id="clip0">
<rect width="24" height="24" fill="white"/>
</clipPath>
</defs>
</svg>"##;

#[test]
fn platform_filter_matrix() {
    let tmp = tempdir().expect("tempdir");
    let assets = tmp.path().join("assets.yaml");
    std::fs::write(&assets, "version: 1\nassets: {}\n").expect("write assets");

    let command = |platform: Option<Vec<String>>| {
        MaterializeCommand::Assets(AssetsArgs {
            path: Some(assets.clone()),
            platform,
            dry_run: true,
            only: None,
        })
    };

    let default = materialize_run(&command(None)).expect("default run");
    assert_eq!(default["platforms"], json!(["ios", "android"]));

    let deduped =
        materialize_run(&command(Some(vec!["ios".into(), "ios".into()]))).expect("dedupe run");
    assert_eq!(deduped["platforms"], json!(["ios"]));

    let err = materialize_run(&command(Some(vec!["web".into()]))).unwrap_err();
    assert!(matches!(err, VectisError::InvalidProject { .. }));
}

#[test]
fn stroke_icon_export_matrix() {
    let tmp = tempdir().expect("tempdir");
    let design = tmp.path();
    let assets_dir = design.join("assets");
    std::fs::create_dir_all(&assets_dir).expect("mkdir assets");
    std::fs::write(assets_dir.join("check.svg"), CHECKMARK).expect("write svg");
    let assets_yaml = design.join("assets.yaml");
    std::fs::write(
        &assets_yaml,
        r"version: 1
assets:
  check:
    alt: Check
    kind: vector
    role: icon
    source: assets/check.svg
",
    )
    .expect("write assets.yaml");

    let summary = materialize_run(&MaterializeCommand::Assets(AssetsArgs {
        path: Some(assets_yaml),
        platform: None,
        dry_run: false,
        only: Some(vec!["check".into()]),
    }))
    .expect("materialize");
    assert!(summary["errors"].as_array().is_some_and(Vec::is_empty), "{summary}");

    let pdf_path = design.join("assets/exports/ios/check.imageset/check.pdf");
    let pdf = std::fs::read(&pdf_path).expect("read pdf");
    assert!(pdf.starts_with(b"%PDF-"), "pdf magic missing");
    let pdf_text = String::from_utf8_lossy(&pdf);
    assert!(pdf_text.contains(" w\n"), "stroke width op missing: {pdf_text}");
    assert!(pdf_text.contains(" RG\n"), "stroke colour op missing: {pdf_text}");
    assert!(pdf_text.contains("S\nQ\n"), "stroke paint op missing: {pdf_text}");
    assert!(!pdf_text.contains("\nf\n"), "stroke-only path must not fill: {pdf_text}");
    assert!(
        !pdf_text.contains(" ca\n") && !pdf_text.contains(" CA\n"),
        "invalid opacity ops: {pdf_text}"
    );

    let xml = std::fs::read_to_string(design.join("assets/exports/android/drawable/check.xml"))
        .expect("read android");
    assert!(xml.contains("android:strokeColor=\"#1F2937\""), "{xml}");
    assert!(xml.contains("android:strokeWidth=\"2\""), "{xml}");
    assert!(xml.contains("android:fillColor=\"#00000000\""), "{xml}");
    assert!(!xml.contains("android:fillColor=\"#1F2937\""), "{xml}");
}

const SQUARE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path fill="#336699" d="M2 2h20v20H2z"/>
</svg>"##;

fn run_assets(assets_yaml: &Path) -> Value {
    materialize_run(&MaterializeCommand::Assets(AssetsArgs {
        path: Some(assets_yaml.to_path_buf()),
        platform: None,
        dry_run: false,
        only: None,
    }))
    .expect("materialize run")
}

fn decoded_dimensions(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "png magic missing at {}", path.display());
    let decoded = image::load_from_memory(&bytes)
        .unwrap_or_else(|err| panic!("decode {}: {err}", path.display()));
    (decoded.width(), decoded.height())
}

// App-icon materialize contract over the public run entry: a vector
// master fans out to the full iOS appiconset and Android adaptive +
// legacy trees (canvas decode, per-density render math, and launcher
// background resolution are asserted on the written exports), and an
// undersized raster master surfaces the public
// `assets-app-icon-source-invalid` error.
#[test]
fn app_icon_export_matrix() {
    let tmp = tempdir().expect("tempdir");
    let design = tmp.path();
    std::fs::create_dir_all(design.join("assets")).expect("mkdir assets");
    std::fs::write(design.join("assets/launcher.svg"), SQUARE).expect("write svg");
    std::fs::write(
        design.join("tokens.yaml"),
        "version: 1\ncolors:\n  brand:\n    light: \"#112233\"\n    dark: \"#001122\"\n",
    )
    .expect("write tokens");
    let assets_yaml = design.join("assets.yaml");
    std::fs::write(
        &assets_yaml,
        r"version: 1
app-icon: launcher
assets:
  launcher:
    alt: Launcher icon
    kind: vector
    role: app-icon
    source: assets/launcher.svg
    tint: brand
",
    )
    .expect("write assets.yaml");

    let summary = run_assets(&assets_yaml);
    assert!(summary["errors"].as_array().is_some_and(Vec::is_empty), "{summary}");

    // iOS: single-size appiconset with a decodable 1024 canvas.
    let appiconset = design.join("assets/exports/ios/app-icon/AppIcon.appiconset");
    assert!(appiconset.join("Contents.json").is_file(), "Contents.json missing");
    assert_eq!(decoded_dimensions(&appiconset.join("AppIcon.png")), (1024, 1024));

    // Android: adaptive XML pair, tinted background, and per-density
    // foreground / legacy PNGs at the documented dp scales.
    let android = design.join("assets/exports/android/app-icon");
    for rel in ["mipmap-anydpi-v26/ic_launcher.xml", "mipmap-anydpi-v26/ic_launcher_round.xml"] {
        let body = std::fs::read_to_string(android.join(rel)).expect("read adaptive xml");
        assert!(body.contains("adaptive-icon"), "{rel}: {body}");
    }
    let background = std::fs::read_to_string(android.join("values/ic_launcher_background.xml"))
        .expect("read background");
    assert!(background.contains("#112233"), "tint token not resolved: {background}");
    for density in ["mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"] {
        assert!(
            android.join(format!("drawable-{density}/ic_launcher_foreground.png")).is_file(),
            "missing foreground {density}"
        );
        assert!(
            android.join(format!("mipmap-{density}/ic_launcher.png")).is_file(),
            "missing legacy {density}"
        );
    }
    // 108dp adaptive canvas and 48dp legacy launcher at xhdpi (2.0×).
    assert_eq!(
        decoded_dimensions(&android.join("drawable-xhdpi/ic_launcher_foreground.png")),
        (216, 216)
    );
    assert_eq!(decoded_dimensions(&android.join("mipmap-xhdpi/ic_launcher.png")), (96, 96));

    // Without a `tint`, the launcher background falls back to white.
    let plain = tempdir().expect("tempdir");
    std::fs::create_dir_all(plain.path().join("assets")).expect("mkdir assets");
    std::fs::write(plain.path().join("assets/launcher.svg"), SQUARE).expect("write svg");
    let plain_yaml = plain.path().join("assets.yaml");
    std::fs::write(
        &plain_yaml,
        r"version: 1
assets:
  launcher:
    alt: Launcher icon
    kind: vector
    role: app-icon
    source: assets/launcher.svg
",
    )
    .expect("write assets.yaml");
    let summary = run_assets(&plain_yaml);
    assert!(summary["errors"].as_array().is_some_and(Vec::is_empty), "{summary}");
    let background = std::fs::read_to_string(
        plain.path().join("assets/exports/android/app-icon/values/ic_launcher_background.xml"),
    )
    .expect("read default background");
    assert!(background.contains("#FFFFFF"), "default background expected: {background}");

    // Undersized raster master: the public error names the code and size.
    let bad = tempdir().expect("tempdir");
    std::fs::create_dir_all(bad.path().join("assets")).expect("mkdir assets");
    RgbaImage::from_pixel(512, 512, Rgba([1, 2, 3, 255]))
        .save_with_format(bad.path().join("assets/launcher.png"), ImageFormat::Png)
        .expect("write small png");
    let bad_yaml = bad.path().join("assets.yaml");
    std::fs::write(
        &bad_yaml,
        r"version: 1
assets:
  launcher:
    kind: raster
    role: app-icon
    source: assets/launcher.png
",
    )
    .expect("write assets.yaml");

    let summary = run_assets(&bad_yaml);
    let errors = serde_json::to_string(&summary["errors"]).expect("errors json");
    assert!(errors.contains("assets-app-icon-source-invalid"), "{errors}");
    assert!(errors.contains("512"), "{errors}");
}

// Illustration-vector materialize over the public run entry: the 1×
// logical SVG canvas fans out to per-density Android PNGs and iOS
// @2x/@3x imageset PNGs at the documented scale factors.
#[test]
fn illustration_export_matrix() {
    let tmp = tempdir().expect("tempdir");
    let design = tmp.path();
    std::fs::create_dir_all(design.join("assets")).expect("mkdir assets");
    std::fs::write(design.join("assets/onboarding-hero.svg"), SQUARE).expect("write svg");
    let assets_yaml = design.join("assets.yaml");
    std::fs::write(
        &assets_yaml,
        r"version: 1
assets:
  onboarding-hero:
    alt: Onboarding hero
    kind: vector
    role: illustration
    source: assets/onboarding-hero.svg
",
    )
    .expect("write assets.yaml");

    let summary = run_assets(&assets_yaml);
    assert!(summary["errors"].as_array().is_some_and(Vec::is_empty), "{summary}");

    // The 24-unit viewBox scales by density factor (mdpi 1.0, xhdpi 2.0,
    // xxxhdpi 4.0) and iOS scale (@2x, @3x).
    for (rel, edge) in [
        ("assets/exports/android/drawable-mdpi/onboarding_hero.png", 24),
        ("assets/exports/android/drawable-xhdpi/onboarding_hero.png", 48),
        ("assets/exports/android/drawable-xxxhdpi/onboarding_hero.png", 96),
        ("assets/exports/ios/onboarding-hero.imageset/onboarding-hero@2x.png", 48),
        ("assets/exports/ios/onboarding-hero.imageset/onboarding-hero@3x.png", 72),
    ] {
        assert_eq!(decoded_dimensions(&design.join(rel)), (edge, edge), "{rel}");
    }
    assert!(
        design.join("assets/exports/ios/onboarding-hero.imageset/Contents.json").is_file(),
        "Contents.json missing"
    );
}

// Auto-pin contract over the public run entry: a materialized icon
// fills absent `sources.<platform>` slots on disk with the canonical
// artifact (the iOS pdf, never Contents.json), and a re-run treats the
// written pins as operator pins (fill-only, skipped — not rewritten).
#[test]
fn auto_pin_matrix() {
    let tmp = tempdir().expect("tempdir");
    let design = tmp.path();
    std::fs::create_dir_all(design.join("assets")).expect("mkdir assets");
    std::fs::write(design.join("assets/settings.svg"), SQUARE).expect("write svg");
    let assets_yaml = design.join("assets.yaml");
    std::fs::write(
        &assets_yaml,
        r"version: 1
assets:
  settings:
    alt: Settings
    kind: vector
    role: icon
    source: assets/settings.svg
",
    )
    .expect("write assets.yaml");

    let summary = run_assets(&assets_yaml);
    assert!(summary["errors"].as_array().is_some_and(Vec::is_empty), "{summary}");

    let written = std::fs::read_to_string(&assets_yaml).expect("re-read assets.yaml");
    let doc: Value = serde_saphyr::from_str(&written).expect("parse assets.yaml");
    let sources = &doc["assets"]["settings"]["sources"];
    assert_eq!(sources["ios"], "assets/exports/ios/settings.imageset/settings.pdf", "{written}");
    assert_eq!(sources["android"], "assets/exports/android/drawable/settings.xml", "{written}");

    let rerun = run_assets(&assets_yaml);
    assert!(rerun["errors"].as_array().is_some_and(Vec::is_empty), "{rerun}");
    let skipped = rerun["skipped_pins"].as_array().expect("skipped_pins");
    assert_eq!(skipped.len(), 2, "{rerun}");
    let unchanged = std::fs::read_to_string(&assets_yaml).expect("re-read assets.yaml");
    assert_eq!(unchanged, written, "pins must be fill-only");
}
