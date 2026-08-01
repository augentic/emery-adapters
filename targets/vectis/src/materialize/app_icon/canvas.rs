//! Shared 1024×1024 launcher canvas decode for `role: app-icon`.

use std::path::Path;

use image::{ImageReader, RgbaImage};

use crate::materialize::render::render_tree_to_png;
use crate::materialize::rgba::image_has_transparency;
use crate::materialize::svg::{NormalizeReport, parse_vector_svg};

/// Fixed launcher canvas edge length (iOS path A and Android path A share this).
pub const LAUNCHER_CANVAS_SIZE: u32 = 1024;

/// Decoded launcher canvas plus optional SVG normalization metadata.
#[derive(Debug)]
pub struct LauncherCanvas {
    /// 1024×1024 RGBA pixels (alpha retained for Android adaptive composition).
    pub image: RgbaImage,
    /// SVG normalization applied during vector decode, when any.
    pub normalization: Option<NormalizeReport>,
    /// Whether any decoded pixel carries α < 255.
    pub has_transparency: bool,
}

/// Decode an app-icon `source:` master into a normalized 1024×1024 RGBA canvas.
///
/// # Errors
///
/// Returns `assets-app-icon-source-invalid: …` when the master cannot be decoded
/// or violates path-A constraints.
pub fn decode_to_launcher_canvas(
    source_path: &Path, source_rel: &str, asset_id: &str,
) -> Result<LauncherCanvas, String> {
    let ext = source_path.extension().and_then(|value| value.to_str()).map(str::to_ascii_lowercase);

    match ext.as_deref() {
        Some("svg") => decode_svg_canvas(source_path, source_rel, asset_id),
        Some("png" | "jpg" | "jpeg" | "webp") => {
            decode_raster_canvas(source_path, source_rel, asset_id)
        }
        _ => Err(format!(
            "assets-app-icon-source-invalid: app-icon `{asset_id}` `source:` `{source_rel}` has no recognised master extension"
        )),
    }
}

fn decode_svg_canvas(
    source_path: &Path, source_rel: &str, asset_id: &str,
) -> Result<LauncherCanvas, String> {
    let bytes = std::fs::read(source_path).map_err(|err| {
        format!(
            "assets-app-icon-source-invalid: app-icon `{asset_id}` `source:` `{source_rel}` not readable: {err}"
        )
    })?;
    let parsed = parse_vector_svg(&bytes, asset_id).map_err(|err| {
        format!("assets-app-icon-source-invalid: app-icon `{asset_id}` SVG decode failed: {err}")
    })?;
    let png = render_tree_to_png(&parsed.tree, LAUNCHER_CANVAS_SIZE, LAUNCHER_CANVAS_SIZE)
        .map_err(|err| {
            format!(
                "assets-app-icon-source-invalid: app-icon `{asset_id}` SVG rasterize failed: {err}"
            )
        })?;
    let image = image::load_from_memory(&png).map_err(|err| {
        format!("assets-app-icon-source-invalid: app-icon `{asset_id}` SVG rasterize failed: {err}")
    })?;
    let rgba = image.to_rgba8();
    Ok(LauncherCanvas {
        has_transparency: image_has_transparency(&rgba),
        normalization: parsed.normalization,
        image: rgba,
    })
}

fn decode_raster_canvas(
    source_path: &Path, source_rel: &str, asset_id: &str,
) -> Result<LauncherCanvas, String> {
    let image = ImageReader::open(source_path)
        .map_err(|err| {
            format!(
                "assets-app-icon-source-invalid: app-icon `{asset_id}` `source:` `{source_rel}` not readable: {err}"
            )
        })?
        .with_guessed_format()
        .map_err(|err| {
            format!(
                "assets-app-icon-source-invalid: app-icon `{asset_id}` raster decode failed: {err}"
            )
        })?
        .decode()
        .map_err(|err| {
            format!(
                "assets-app-icon-source-invalid: app-icon `{asset_id}` raster decode failed: {err}"
            )
        })?;

    let (width, height) = (image.width(), image.height());
    if width != height {
        return Err(format!(
            "assets-app-icon-source-invalid: raster app-icon `{asset_id}` master must be square (got {width}×{height})"
        ));
    }
    if width < LAUNCHER_CANVAS_SIZE {
        return Err(format!(
            "assets-app-icon-source-invalid: raster app-icon `{asset_id}` master must be at least 1024×1024 (got {width}×{height})"
        ));
    }

    let rgba = if width > LAUNCHER_CANVAS_SIZE {
        image
            .resize_exact(
                LAUNCHER_CANVAS_SIZE,
                LAUNCHER_CANVAS_SIZE,
                image::imageops::FilterType::Lanczos3,
            )
            .to_rgba8()
    } else {
        image.to_rgba8()
    };
    Ok(LauncherCanvas {
        has_transparency: image_has_transparency(&rgba),
        normalization: None,
        image: rgba,
    })
}
