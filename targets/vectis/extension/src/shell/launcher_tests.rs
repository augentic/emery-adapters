//! Unit tests for shell-resident launcher icon probes.

use std::path::Path;

use tempfile::tempdir;

use super::shell_resident_app_icon;

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
// branches are covered end-to-end by `tests/engine/verify.rs`:
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
