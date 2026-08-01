//! `materialize assets` operation-surface tests over the public
//! [`vectis::materialize::run`] entry.

use serde_json::json;
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
