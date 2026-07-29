//! Android Vector Drawable export for icon vectors.

use std::fmt::Write;

use usvg::Tree;

use crate::materialize::svg::{
    StrokeCap, StrokeJoin, StrokePaint, collect_paths, path_data_string,
};

/// Write a `drawable/<id>.xml` Vector Drawable for an icon.
///
/// # Errors
/// Returns I/O errors from the underlying write.
pub fn write_vector_drawable(
    tree: &Tree, _drawable_name: &str, out_path: &std::path::Path,
) -> std::io::Result<()> {
    let width = tree.size().width();
    let height = tree.size().height();
    let mut paths = Vec::new();
    collect_paths(tree.root(), &mut paths);

    let mut body = String::new();
    body.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    let _ = write!(
        body,
        "<vector xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    android:width=\"{width}dp\"\n    android:height=\"{height}dp\"\n    android:viewportWidth=\"{width}\"\n    android:viewportHeight=\"{height}\">\n"
    );

    for drawable in paths {
        let path_data = path_data_string(&drawable.geometry);
        if path_data.is_empty() {
            continue;
        }
        body.push_str("    <path\n");
        match drawable.fill {
            Some((r, g, b, opacity)) => {
                let _ = writeln!(body, "        android:fillColor=\"{}\"", android_color(r, g, b));
                let _ = writeln!(body, "        android:fillAlpha=\"{}\"", trim_num(opacity));
            }
            None => {
                body.push_str("        android:fillColor=\"#00000000\"\n");
            }
        }
        if let Some(stroke) = drawable.stroke {
            append_stroke_attrs(&mut body, &stroke);
        }
        let _ = writeln!(body, "        android:pathData=\"{path_data}\"/>");
    }

    body.push_str("</vector>\n");

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, body)
}

fn append_stroke_attrs(body: &mut String, stroke: &StrokePaint) {
    let (r, g, b) = stroke.color;
    let _ = writeln!(body, "        android:strokeColor=\"{}\"", android_color(r, g, b));
    let _ = writeln!(body, "        android:strokeWidth=\"{}\"", trim_num(stroke.width));
    let _ = writeln!(body, "        android:strokeAlpha=\"{}\"", trim_num(stroke.opacity));
    if let Some(cap) = android_linecap(stroke.linecap) {
        let _ = writeln!(body, "        android:strokeLineCap=\"{cap}\"");
    }
    if let Some(join) = android_linejoin(stroke.linejoin) {
        let _ = writeln!(body, "        android:strokeLineJoin=\"{join}\"");
    }
}

const fn android_linecap(cap: StrokeCap) -> Option<&'static str> {
    match cap {
        StrokeCap::Butt => None,
        StrokeCap::Round => Some("round"),
        StrokeCap::Square => Some("square"),
    }
}

const fn android_linejoin(join: StrokeJoin) -> Option<&'static str> {
    match join {
        StrokeJoin::Miter => None,
        StrokeJoin::Round => Some("round"),
        StrokeJoin::Bevel => Some("bevel"),
    }
}

fn android_color(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn trim_num(value: f32) -> String {
    format!("{value:.4}").trim_end_matches('0').trim_end_matches('.').to_string()
}
