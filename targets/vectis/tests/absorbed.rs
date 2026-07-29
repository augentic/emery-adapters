//! Kernel tests for the materialize and validate libraries.

use image::{ImageFormat, Rgba, RgbaImage};
use serde_json::{Map, Value, json};
use tempfile::tempdir;
use vectis::VectisError;
use vectis::materialize::app_icon::android::{resolve_launcher_background, write_android_export};
use vectis::materialize::app_icon::decode_to_launcher_canvas;
use vectis::materialize::paths::{
    ANDROID_DENSITIES, Platform, android_density_factor, export_layout, ios_raster_filename,
    ios_scale_factor, kebab_to_snake,
};
use vectis::materialize::render::{render_tree_to_png, scaled_dimensions};
use vectis::materialize::svg::{collect_paths, parse_vector_svg, path_data_string};
use vectis::materialize::yaml_pins::{AutoPin, apply_auto_pins, collect_auto_pins};
use vectis::materialize::{AssetsArgs, MaterializeCommand, run as materialize_run};
use vectis::validate::engine::composition::{build_group_skeleton, fingerprint, skeleton_to_json};
use vectis::validate::engine::{conventional_export_exists, imageset_has_materialized_content};

const TRIANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path fill="#010203" d="M12 2L2 22h20z"/>
</svg>"##;

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

// `1x` omits the `@` suffix.
#[test]
fn scale_and_filename_conventions() {
    assert_eq!(kebab_to_snake("onboarding-hero"), "onboarding_hero");
    assert_eq!(kebab_to_snake("settings"), "settings");

    assert_eq!(ios_scale_factor("2x"), Some(2.0_f32));
    assert_eq!(ios_scale_factor("3x"), Some(3.0_f32));
    for (density, factor) in
        [("mdpi", 1.0_f32), ("hdpi", 1.5), ("xhdpi", 2.0), ("xxhdpi", 3.0), ("xxxhdpi", 4.0)]
    {
        assert_eq!(android_density_factor(density), Some(factor), "{density}");
    }

    assert_eq!(ios_raster_filename("hero", "1x"), "hero.png");
    assert_eq!(ios_raster_filename("hero", "2x"), "hero@2x.png");
}

#[test]
fn export_layout_matrix() {
    struct Case {
        role: &'static str,
        kind: &'static str,
        platform: Platform,
        asset_id: &'static str,
        pin: &'static str,
        artifacts: &'static [&'static str],
    }

    let cases = [
        Case {
            role: "icon",
            kind: "vector",
            platform: Platform::Ios,
            asset_id: "settings",
            pin: "assets/exports/ios/settings.imageset/settings.pdf",
            artifacts: &[
                "assets/exports/ios/settings.imageset/settings.pdf",
                "assets/exports/ios/settings.imageset/Contents.json",
            ],
        },
        Case {
            role: "icon",
            kind: "vector",
            platform: Platform::Android,
            asset_id: "chevron-right",
            pin: "assets/exports/android/drawable/chevron_right.xml",
            artifacts: &["assets/exports/android/drawable/chevron_right.xml"],
        },
        Case {
            role: "illustration",
            kind: "vector",
            platform: Platform::Ios,
            asset_id: "onboarding-hero",
            pin: "assets/exports/ios/onboarding-hero.imageset/onboarding-hero@3x.png",
            artifacts: &[
                "assets/exports/ios/onboarding-hero.imageset/onboarding-hero@2x.png",
                "assets/exports/ios/onboarding-hero.imageset/onboarding-hero@3x.png",
                "assets/exports/ios/onboarding-hero.imageset/Contents.json",
            ],
        },
        Case {
            role: "illustration",
            kind: "vector",
            platform: Platform::Android,
            asset_id: "onboarding-hero",
            pin: "assets/exports/android/drawable-xxxhdpi/onboarding_hero.png",
            artifacts: &[
                "assets/exports/android/drawable-mdpi/onboarding_hero.png",
                "assets/exports/android/drawable-hdpi/onboarding_hero.png",
                "assets/exports/android/drawable-xhdpi/onboarding_hero.png",
                "assets/exports/android/drawable-xxhdpi/onboarding_hero.png",
                "assets/exports/android/drawable-xxxhdpi/onboarding_hero.png",
            ],
        },
        Case {
            role: "app-icon",
            kind: "vector",
            platform: Platform::Ios,
            asset_id: "app-icon",
            pin: "assets/exports/ios/app-icon/AppIcon.appiconset",
            artifacts: &[
                "assets/exports/ios/app-icon/AppIcon.appiconset/Contents.json",
                "assets/exports/ios/app-icon/AppIcon.appiconset/AppIcon.png",
            ],
        },
    ];

    for case in cases {
        let layout = export_layout(case.role, case.kind, case.platform, case.asset_id)
            .expect("declared layout case resolves");
        assert_eq!(layout.pin, case.pin, "{}/{}", case.role, case.kind);
        assert_eq!(
            layout.artifacts,
            case.artifacts.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "{}/{}",
            case.role,
            case.kind
        );
    }

    // `decorative/vector` aliases `icon/vector` exactly.
    assert_eq!(
        export_layout("decorative", "vector", Platform::Ios, "sparkle"),
        export_layout("icon", "vector", Platform::Ios, "sparkle"),
    );

    // The android app-icon tree fans out to 13 artifacts (anydpi xml +
    // background + per-density foreground/launcher pngs).
    let android = export_layout("app-icon", "raster", Platform::Android, "app-icon")
        .expect("app-icon android");
    assert_eq!(android.pin, "assets/exports/android/app-icon");
    assert_eq!(android.artifacts.len(), 13);
    assert!(android.artifacts.contains(
        &"assets/exports/android/app-icon/mipmap-anydpi-v26/ic_launcher.xml".to_string()
    ));
    assert!(android.artifacts.iter().any(|path| path.contains("drawable-mdpi")));
    assert!(android.artifacts.iter().any(|path| path.contains("mipmap-xxxhdpi")));
}

// Roles/kinds without a canonical master do not auto-convert.
#[test]
fn unsupported_roles() {
    assert!(export_layout("photo", "raster", Platform::Ios, "hero").is_none());
    assert!(export_layout("icon", "symbol", Platform::Ios, "close").is_none());
    assert!(export_layout("icon", "raster", Platform::Android, "badge").is_none());
}

// Artifact entry wins over sidecar Contents.json; auto-pins fill absent slots only.
#[test]
fn yaml_pins_matrix() {
    let assets = Map::from_iter([(
        "settings".to_string(),
        json!({ "kind": "vector", "role": "icon", "source": "assets/settings.svg" }),
    )]);
    let materialized = vec![
        json!({ "asset_id": "settings", "platform": "ios", "path": "assets/exports/ios/settings.imageset/settings.pdf" }),
        json!({ "asset_id": "settings", "platform": "ios", "path": "assets/exports/ios/settings.imageset/Contents.json" }),
    ];
    let pins = collect_auto_pins(&materialized, &assets);
    assert_eq!(pins.len(), 1);
    assert_eq!(pins[0].path, "assets/exports/ios/settings.imageset/settings.pdf");

    let mut instance = json!({
        "version": 1,
        "assets": {
            "settings": {
                "kind": "vector",
                "role": "icon",
                "source": "assets/settings.svg",
                "sources": { "ios": "assets/exports/ios/settings.imageset/settings.pdf" }
            }
        }
    });
    let slots = vec![
        AutoPin {
            asset_id: "settings".into(),
            platform: "ios".into(),
            path: "assets/exports/ios/settings.imageset/settings.pdf".into(),
        },
        AutoPin {
            asset_id: "settings".into(),
            platform: "android".into(),
            path: "assets/exports/android/drawable/settings.xml".into(),
        },
    ];
    apply_auto_pins(&mut instance, &slots);
    let sources = &instance["assets"]["settings"]["sources"];
    assert_eq!(sources["ios"], "assets/exports/ios/settings.imageset/settings.pdf");
    assert_eq!(sources["android"], "assets/exports/android/drawable/settings.xml");
}

#[test]
fn svg_parse_matrix() {
    let parsed = parse_vector_svg(TRIANGLE.as_bytes(), "tri").expect("parse");
    assert!(parsed.tree.size().width() > 0.0);
    let mut paths = Vec::new();
    collect_paths(parsed.tree.root(), &mut paths);
    assert_eq!(path_data_string(&paths[0].geometry), "M12 2 L2 22 L22 22 Z");
    assert!(paths[0].fill.is_some());
    assert!(paths[0].stroke.is_none());

    let filtered = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <filter id="blur"><feGaussianBlur stdDeviation="2"/></filter>
  <rect width="24" height="24" filter="url(#blur)"/>
</svg>"#;
    let err = parse_vector_svg(filtered.as_bytes(), "bad").unwrap_err();
    assert!(err.contains("bad"));
    assert!(err.contains("filters"));
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
    assert!(!pdf_text.contains(" ca\n") && !pdf_text.contains(" CA\n"), "invalid opacity ops: {pdf_text}");

    let xml = std::fs::read_to_string(design.join("assets/exports/android/drawable/check.xml"))
        .expect("read android");
    assert!(xml.contains("android:strokeColor=\"#1F2937\""), "{xml}");
    assert!(xml.contains("android:strokeWidth=\"2\""), "{xml}");
    assert!(xml.contains("android:fillColor=\"#00000000\""), "{xml}");
    assert!(!xml.contains("android:fillColor=\"#1F2937\""), "{xml}");

    let filled = parse_vector_svg(TRIANGLE.as_bytes(), "tri").expect("parse fill");
    let mut filled_paths = Vec::new();
    collect_paths(filled.tree.root(), &mut filled_paths);
    assert!(filled_paths[0].fill.is_some());
    assert!(filled_paths[0].stroke.is_none());
}

#[test]
fn stroke_normalize_preserves_stroke() {
    let parsed = parse_vector_svg(CHECKMARK.as_bytes(), "check").expect("parse");
    let report = parsed.normalization.expect("noop clip should normalize");
    assert!(report.transforms.contains(&"stripped-noop-clip"));
    let mut paths = Vec::new();
    collect_paths(parsed.tree.root(), &mut paths);
    assert_eq!(paths.len(), 1);
    assert!(paths[0].fill.is_none());
    let stroke = paths[0].stroke.expect("stroke");
    assert_eq!(stroke.color, (0x1F, 0x29, 0x37));
    assert!((stroke.width - 2.0).abs() < f32::EPSILON, "width={}", stroke.width);
}

#[test]
fn stroke_width_scales_with_transform() {
    let scaled = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">
  <g transform="scale(2)">
    <path d="M5 12L10 17L20 7" stroke="#112233" stroke-width="2" fill="none"/>
  </g>
</svg>"##;
    let parsed = parse_vector_svg(scaled.as_bytes(), "scaled").expect("parse");
    let mut paths = Vec::new();
    collect_paths(parsed.tree.root(), &mut paths);
    assert_eq!(paths.len(), 1);
    let stroke = paths[0].stroke.expect("stroke");
    assert!(
        (stroke.width - 4.0).abs() < 0.01,
        "expected canvas width 4, got {}",
        stroke.width
    );
}

#[test]
fn render_tree_matrix() {
    let parsed = parse_vector_svg(TRIANGLE.as_bytes(), "tri").expect("parse");
    let (w, h) = scaled_dimensions(&parsed.tree, 2.0);
    assert_eq!((w, h), (48, 48));
    let png = render_tree_to_png(&parsed.tree, w, h).expect("render");
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(png.len() > 64);

    let (w3, h3) = scaled_dimensions(&parsed.tree, 3.0);
    let first = render_tree_to_png(&parsed.tree, w3, h3).expect("first");
    let second = render_tree_to_png(&parsed.tree, w3, h3).expect("second");
    assert_eq!(first, second);
}

#[test]
fn launcher_background_matrix() {
    let tmp = tempdir().expect("tempdir");
    let design = tmp.path().join("design-system");
    std::fs::create_dir_all(&design).expect("mkdir");
    std::fs::write(
        design.join("tokens.yaml"),
        "version: 1\ncolors:\n  brand:\n    light: \"#AABBCC\"\n    dark: \"#001122\"\n",
    )
    .expect("tokens");
    assert_eq!(resolve_launcher_background(&json!({ "tint": "brand" }), &design), "#AABBCC");
    assert_eq!(resolve_launcher_background(&json!({}), tmp.path()), "#FFFFFF");
}

#[test]
fn android_export_tree() {
    let tmp = tempdir().expect("tempdir");
    let root = tmp.path().join("app-icon");
    let canvas = RgbaImage::from_pixel(1024, 1024, Rgba([20, 40, 60, 255]));

    write_android_export(&canvas, "#112233", &root).expect("write");

    for rel in [
        "mipmap-anydpi-v26/ic_launcher.xml",
        "mipmap-anydpi-v26/ic_launcher_round.xml",
        "values/ic_launcher_background.xml",
    ] {
        assert!(root.join(rel).is_file(), "missing {rel}");
    }

    for density in ANDROID_DENSITIES {
        assert!(
            root.join(format!("drawable-{density}/ic_launcher_foreground.png")).is_file(),
            "missing foreground {density}"
        );
        assert!(
            root.join(format!("mipmap-{density}/ic_launcher.png")).is_file(),
            "missing legacy {density}"
        );
    }

    let bg = std::fs::read_to_string(root.join("values/ic_launcher_background.xml")).expect("read");
    assert!(bg.contains("ic_launcher_background"));
    assert!(bg.contains("#112233"));

    // The embedded adaptive templates ride along verbatim.
    let launcher = std::fs::read_to_string(root.join("mipmap-anydpi-v26/ic_launcher.xml"))
        .expect("read launcher xml");
    assert!(launcher.contains("adaptive-icon"));
    assert!(launcher.contains("@drawable/ic_launcher_foreground"));
    let round = std::fs::read_to_string(root.join("mipmap-anydpi-v26/ic_launcher_round.xml"))
        .expect("read round xml");
    assert!(round.contains("adaptive-icon"));
}

#[test]
fn launcher_canvas_matrix() {
    let tmp = tempdir().expect("tempdir");

    let ok = tmp.path().join("app-icon.png");
    RgbaImage::from_pixel(1024, 1024, Rgba([4, 5, 6, 255]))
        .save_with_format(&ok, ImageFormat::Png)
        .expect("write png");
    let canvas = decode_to_launcher_canvas(&ok, "assets/app-icon.png", "app-icon").expect("decode");
    assert_eq!(canvas.image.dimensions(), (1024, 1024));
    assert_eq!(canvas.image.get_pixel(0, 0).0, [4, 5, 6, 255]);
    assert!(!canvas.has_transparency);

    let small = tmp.path().join("small.png");
    RgbaImage::from_pixel(512, 512, Rgba([1, 2, 3, 255]))
        .save_with_format(&small, ImageFormat::Png)
        .expect("write png");
    let err = decode_to_launcher_canvas(&small, "assets/small.png", "app-icon").unwrap_err();
    assert!(err.contains("assets-app-icon-source-invalid"));
    assert!(err.contains("512"));

    let alpha = tmp.path().join("alpha.png");
    RgbaImage::from_pixel(1024, 1024, Rgba([1, 2, 3, 128]))
        .save_with_format(&alpha, ImageFormat::Png)
        .expect("write png");
    let canvas = decode_to_launcher_canvas(&alpha, "assets/alpha.png", "app-icon").expect("decode");
    assert!(canvas.has_transparency);
    assert_eq!(canvas.image.get_pixel(0, 0)[3], 128);
}

// iOS raster imageset needs a materialized file beyond Contents.json.
#[test]
fn conventional_export_matrix() {
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

fn group(items: Value) -> Value {
    let mut map = Map::new();
    map.insert("items".to_string(), items);
    Value::Object(map)
}

// Bind/event wiring is ignored by fingerprinting.
#[test]
fn fingerprint_and_skeleton_matrix() {
    let skeleton = build_group_skeleton(&group(json!([
        { "icon-button": { "bind": "home", "event": "Navigate(Home)" } },
        { "icon-button": { "bind": "search", "event": "Navigate(Search)" } },
    ])));
    assert_eq!(fingerprint(&skeleton), fingerprint(&skeleton));

    let rewired = build_group_skeleton(&group(json!([
        { "icon-button": { "bind": "profile", "event": "Navigate(Profile)" } },
        { "icon-button": { "bind": "inbox", "event": "Navigate(Inbox)" } },
    ])));
    assert_eq!(fingerprint(&skeleton), fingerprint(&rewired));

    let two = build_group_skeleton(&group(json!([ { "icon-button": {} }, { "icon-button": {} } ])));
    let three = build_group_skeleton(&group(json!([
        { "icon-button": {} },
        { "icon-button": {} },
        { "icon-button": {} },
    ])));
    assert_ne!(fingerprint(&two), fingerprint(&three));

    let bare = build_group_skeleton(&json!({ "items": [ { "text": {} } ] }));
    let conditional =
        build_group_skeleton(&json!({ "active-when": "$x", "items": [ { "text": {} } ] }));
    assert_ne!(fingerprint(&bare), fingerprint(&conditional));

    let nested = build_group_skeleton(&json!({
        "active-when": "$route",
        "items": [
            { "icon-button": {} },
            { "group": { "items": [ { "text": {} } ] } },
        ],
    }));
    let projected = skeleton_to_json(&nested);
    assert_eq!(projected["group"]["when_keys"], json!(["active-when"]));
    assert_eq!(projected["group"]["items"][0], json!({ "item": "icon-button" }));
    assert_eq!(
        projected["group"]["items"][1],
        json!({ "group": { "when_keys": [], "items": [ { "item": "text" } ] } })
    );
}
