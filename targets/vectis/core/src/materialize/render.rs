//! SVG rasterization via `resvg` for illustration exports.

use resvg::tiny_skia::{Pixmap, Transform};
use usvg::Tree;

/// Render a parsed SVG tree to PNG bytes at the given pixel dimensions.
///
/// # Errors
///
/// Returns a human-readable message when allocation or PNG encoding fails.
pub fn render_tree_to_png(tree: &Tree, out_width: u32, out_height: u32) -> Result<Vec<u8>, String> {
    if out_width == 0 || out_height == 0 {
        return Err("render dimensions must be non-zero".into());
    }

    let mut pixmap = Pixmap::new(out_width, out_height)
        .ok_or_else(|| "render buffer allocation failed".to_string())?;

    let svg_size = tree.size();
    if svg_size.width() <= 0.0 || svg_size.height() <= 0.0 {
        return Err("SVG canvas size must be non-zero".into());
    }
    let scale_x = f64::from(out_width) / f64::from(svg_size.width());
    let scale_y = f64::from(out_height) / f64::from(svg_size.height());
    #[expect(
        clippy::cast_possible_truncation,
        reason = "scale ratios for designer-scale SVGs are far below f32 max"
    )]
    let transform = Transform::from_scale(scale_x as f32, scale_y as f32);

    resvg::render(tree, transform, &mut pixmap.as_mut());

    pixmap.encode_png().map_err(|err| format!("PNG encode failed: {err}"))
}

/// Pixel dimensions for a 1× logical SVG canvas scaled by `factor`.
#[must_use]
pub fn scaled_dimensions(tree: &Tree, factor: f32) -> (u32, u32) {
    let size = tree.size();
    (pixel_dim(size.width(), factor), pixel_dim(size.height(), factor))
}

fn pixel_dim(logical: f32, factor: f32) -> u32 {
    let scaled = (logical * factor).round().max(1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "SVG logical dimensions are designer-scale; products fit comfortably in u32"
    )]
    {
        scaled as u32
    }
}
