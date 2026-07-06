//! iOS `AppIcon.appiconset` export for auto-converted app icons (RFC-46 §4.2).

use std::io::Cursor;
use std::path::Path;

use image::RgbaImage;
use serde_json::json;

use crate::materialize::rgba::flatten_to_opaque_white;

pub const APPICON_PNG_NAME: &str = "AppIcon.png";

/// Write a single-size iOS 11+ `AppIcon.appiconset` from a 1024×1024 canvas.
///
/// # Errors
///
/// Returns a human-readable message when directory creation, PNG encoding, or
/// JSON serialization fails.
pub fn write_appiconset(canvas: &RgbaImage, appiconset_dir: &Path) -> Result<(), String> {
    if canvas.dimensions() != (1024, 1024) {
        return Err(format!(
            "internal: app-icon canvas must be 1024×1024 (got {}×{})",
            canvas.width(),
            canvas.height()
        ));
    }

    std::fs::create_dir_all(appiconset_dir).map_err(|err| {
        format!("AppIcon.appiconset write failed at {}: {err}", appiconset_dir.display())
    })?;

    let (opaque, _) = flatten_to_opaque_white(canvas.clone());

    let png_path = appiconset_dir.join(APPICON_PNG_NAME);
    let mut png_bytes = Vec::new();
    opaque
        .write_to(&mut Cursor::new(&mut png_bytes), image::ImageFormat::Png)
        .map_err(|err| format!("AppIcon.png encode failed: {err}"))?;
    std::fs::write(&png_path, png_bytes)
        .map_err(|err| format!("AppIcon.png write failed at {}: {err}", png_path.display()))?;

    let contents = json!({
        "images": [
            {
                "filename": APPICON_PNG_NAME,
                "idiom": "universal",
                "platform": "ios",
                "size": "1024x1024"
            }
        ],
        "info": {
            "author": "xcode",
            "version": 1
        }
    });
    let contents_path = appiconset_dir.join("Contents.json");
    std::fs::write(&contents_path, serde_json::to_vec_pretty(&contents).expect("contents json"))
        .map_err(|err| {
            format!("Contents.json write failed at {}: {err}", contents_path.display())
        })?;

    Ok(())
}

// `write_appiconset` lives in a private module (CLI-only reachable). Its
// `AppIcon.png` + `Contents.json` layout — single universal 1024×1024 ios
// image entry — is asserted end-to-end through the CLI by
// `tests/engine/materialize_app_icon.rs::ios_exports_exist`. Transparent-canvas
// flattening is covered by
// `tests/engine/materialize_app_icon.rs::transparent_*`.
