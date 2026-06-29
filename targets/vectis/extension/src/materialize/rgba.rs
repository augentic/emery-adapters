//! Shared RGBA alpha compositing for materialize exports.

use image::RgbaImage;

/// Returns true when any pixel carries partial or full transparency.
#[must_use]
pub fn image_has_transparency(image: &RgbaImage) -> bool {
    image.pixels().any(|pixel| pixel[3] < 255)
}

/// Composite `foreground` onto an opaque `background` colour; returns whether any pixel had α < 255.
#[must_use]
pub fn flatten_to_opaque_on_background(
    mut foreground: RgbaImage, background: [u8; 3],
) -> (RgbaImage, bool) {
    let mut had_transparency = false;
    for pixel in foreground.pixels_mut() {
        if pixel[3] < 255 {
            had_transparency = true;
            let alpha = f32::from(pixel[3]) / 255.0;
            pixel[0] = blend_channel(pixel[0], alpha, background[0]);
            pixel[1] = blend_channel(pixel[1], alpha, background[1]);
            pixel[2] = blend_channel(pixel[2], alpha, background[2]);
            pixel[3] = 255;
        }
    }
    (foreground, had_transparency)
}

/// Composite any residual transparency onto an opaque white background.
#[must_use]
pub fn flatten_to_opaque_white(foreground: RgbaImage) -> (RgbaImage, bool) {
    flatten_to_opaque_on_background(foreground, [255, 255, 255])
}

fn blend_channel(foreground: u8, alpha: f32, background: u8) -> u8 {
    let blended = f32::from(background).mul_add(1.0 - alpha, f32::from(foreground) * alpha);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "blended channel is clamped to 0..=255 before narrowing"
    )]
    {
        blended.round().clamp(0.0, 255.0) as u8
    }
}
