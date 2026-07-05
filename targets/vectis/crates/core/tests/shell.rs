//! Crux shell presence heuristics and launcher icon probes, moved from
//! the extension's `shell` unit tests (RFC-61 Step 5 Milestone A1).

use std::path::Path;

use specify_vectis_core::shell::{
    SUPPORTED_SHELL_PLATFORMS, shell_present, shell_resident_app_icon,
};
use tempfile::tempdir;

// Greenfield (empty) tree: every supported shell is absent. The `core`-absent
// branch is unit-only — `verify` fixtures always scaffold core. The positive
// and dir-without-source-file branches are covered end-to-end by
// `tests/engine/verify.rs`: `all_present_exits_clean` (all present),
// `missing_shell_exits_one` (ios absent), `web_desktop_emit_info_not_error`,
// `ios_dir_without_swift_files_is_not_present`,
// `android_dir_without_kt_files_is_not_present`.
#[test]
fn greenfield_all_supported_absent() {
    let tmp = tempdir().unwrap();
    assert!(!shell_present(tmp.path(), "core"));
    assert!(!shell_present(tmp.path(), "ios"));
    assert!(!shell_present(tmp.path(), "android"));
}

#[test]
fn supported_platforms_closed_set() {
    assert_eq!(SUPPORTED_SHELL_PLATFORMS, &["core", "ios", "android"]);
}

fn scaffold_ios_appiconset(root: &Path, contents_json: &str, png_bytes: Option<&[u8]>) {
    let appiconset = root.join("iOS/TestApp/Resources/Assets.xcassets/AppIcon.appiconset");
    std::fs::create_dir_all(&appiconset).expect("mkdir appiconset");
    std::fs::write(appiconset.join("Contents.json"), contents_json).expect("write Contents.json");
    if let Some(bytes) = png_bytes {
        std::fs::write(appiconset.join("AppIcon.png"), bytes).expect("write png");
    }
}

fn minimal_png() -> Vec<u8> {
    // 1×1 RGBA PNG — sufficient for presence probe (not dimension validation).
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

// Negative `shell_resident_app_icon` branches. The positive ios/android
// branches are covered end-to-end by the extension's `tests/engine/verify.rs`:
// `bootstrap_app_icon_shell_resident_escape_hatch` (ios appiconset + png),
// `bootstrap_app_icon_android_shell_resident_anydpi` (adaptive xml), and
// `bootstrap_app_icon_android_shell_resident_mipmap_png` (density-bucket png).
// This matrix pins the falses: an ios appiconset whose referenced PNG is
// absent, an ios Contents.json with no images, an android tree with no
// launcher, and `core` (never shell-resident).
#[test]
fn shell_resident_app_icon_matrix() {
    let png_ref = r#"{
  "images": [{ "filename": "AppIcon.png", "idiom": "universal", "platform": "ios", "size": "1024x1024" }],
  "info": { "author": "xcode", "version": 1 }
}"#;

    let skeleton = tempdir().unwrap();
    scaffold_ios_appiconset(skeleton.path(), png_ref, None);
    assert!(!shell_resident_app_icon(skeleton.path(), "ios"));

    let no_images = tempdir().unwrap();
    scaffold_ios_appiconset(no_images.path(), r#"{"info":{"version":1}}"#, Some(&minimal_png()));
    assert!(!shell_resident_app_icon(no_images.path(), "ios"));

    let android = tempdir().unwrap();
    let values = android.path().join("Android/app/src/main/res/values");
    std::fs::create_dir_all(&values).expect("mkdir values");
    std::fs::write(values.join("strings.xml"), "<resources/>").expect("write strings");
    assert!(!shell_resident_app_icon(android.path(), "android"));

    let core = tempdir().unwrap();
    assert!(!shell_resident_app_icon(core.path(), "core"));
}

// Positive ios probe through whitespace-tolerant Contents.json parsing:
// `"filename" : "AppIcon.png"` (spaced colon) must still resolve the PNG.
// Replaces the extension's two private-helper unit tests
// (`parses_filename_from_contents_json` / `parse_json_string_value`) by
// reaching the same parse kernel through the public probe.
#[test]
fn spaced_contents_json_resolves_icon() {
    let spaced = r#"{
  "images": [{ "filename" : "AppIcon.png", "idiom" : "universal" }],
  "info": { "author": "xcode", "version": 1 }
}"#;
    let tmp = tempdir().unwrap();
    scaffold_ios_appiconset(tmp.path(), spaced, Some(&minimal_png()));
    assert!(shell_resident_app_icon(tmp.path(), "ios"));
}
