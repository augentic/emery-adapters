//! Flatten Figma-export noise (no-op clips, group opacity) into export-clean SVG.

use std::fmt::Write;

use usvg::tiny_skia_path::Rect;
use usvg::{BlendMode, ClipPath, Group, LineCap, LineJoin, Node, Paint, Path, Tree};

use super::{absolute_path, path_data_string, trim_num};

const CLIP_BOUNDS_TOLERANCE: f32 = 0.5;

/// Transforms applied during vector SVG normalization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NormalizeReport {
    /// Tags of the normalization transforms applied, in order.
    pub transforms: Vec<&'static str>,
}

impl NormalizeReport {
    fn record(&mut self, tag: &'static str) {
        if !self.transforms.contains(&tag) {
            self.transforms.push(tag);
        }
    }
}

/// Flatten no-op clips and bake group opacity into path-level alpha.
///
/// # Errors
///
/// Returns a human-readable message when the SVG uses unsupported constructs
/// that cannot be normalized away.
pub fn normalize_for_export(
    tree: &Tree, asset_id: &str,
) -> Result<Option<(Vec<u8>, NormalizeReport)>, String> {
    let canvas = artboard_rect(tree);
    let mut report = NormalizeReport::default();
    let mut paths = Vec::new();
    collect_flat_paths(tree.root(), 1.0, &canvas, asset_id, &mut report, &mut paths)?;

    if report.transforms.is_empty() {
        return Ok(None);
    }

    if paths.is_empty() {
        return Err(format!("asset `{asset_id}`: SVG contains no drawable paths"));
    }

    let svg = emit_minimal_svg(tree.size().width(), tree.size().height(), &paths);
    Ok(Some((svg.into_bytes(), report)))
}

struct FlatStroke {
    color: (u8, u8, u8),
    opacity: f32,
    width: f32,
    linecap: LineCap,
    linejoin: LineJoin,
}

struct FlatPath {
    d: String,
    fill: Option<(u8, u8, u8, f32)>,
    stroke: Option<FlatStroke>,
}

fn collect_flat_paths(
    group: &Group, opacity_stack: f32, canvas: &Rect, asset_id: &str, report: &mut NormalizeReport,
    out: &mut Vec<FlatPath>,
) -> Result<(), String> {
    if group.blend_mode() != BlendMode::Normal {
        return Err(format!("asset `{asset_id}`: non-normal blend modes are not supported"));
    }
    if group.mask().is_some() || !group.filters().is_empty() {
        return Err(format!(
            "asset `{asset_id}`: clip paths, masks, and filters are not supported"
        ));
    }

    let mut group_stack = opacity_stack;
    if group.opacity().get() < 1.0 {
        group_stack *= group.opacity().get();
        report.record("baked-group-opacity");
    }

    if let Some(clip) = group.clip_path() {
        if is_noop_clip(clip, canvas) {
            report.record("stripped-noop-clip");
        } else {
            return Err(format!(
                "asset `{asset_id}`: clip paths, masks, and filters are not supported"
            ));
        }
    }

    for child in group.children() {
        match child {
            Node::Group(nested) => {
                collect_flat_paths(nested, group_stack, canvas, asset_id, report, out)?;
            }
            Node::Path(path) => {
                push_flat_path(path, group_stack, asset_id, out)?;
            }
            Node::Image(_) => {
                return Err(format!(
                    "asset `{asset_id}`: embedded raster images are not supported"
                ));
            }
            Node::Text(_) => {
                return Err(format!("asset `{asset_id}`: text nodes are not supported"));
            }
        }
    }
    Ok(())
}

fn push_flat_path(
    path: &Path, opacity_stack: f32, asset_id: &str, out: &mut Vec<FlatPath>,
) -> Result<(), String> {
    if !path.is_visible() {
        return Ok(());
    }

    let Some(geometry) = absolute_path(path) else {
        return Ok(());
    };
    let d = path_data_string(&geometry);
    if d.is_empty() {
        return Ok(());
    }

    let mut flat = FlatPath {
        d,
        fill: None,
        stroke: None,
    };

    if let Some(fill) = path.fill() {
        let alpha = fill.opacity().get() * opacity_stack;
        match fill.paint() {
            Paint::Color(color) => {
                flat.fill = Some((color.red, color.green, color.blue, alpha));
            }
            Paint::LinearGradient(_) | Paint::RadialGradient(_) | Paint::Pattern(_) => {
                return Err(format!(
                    "asset `{asset_id}`: fill gradients and patterns are not supported"
                ));
            }
        }
    }

    if let Some(stroke) = path.stroke() {
        let alpha = stroke.opacity().get() * opacity_stack;
        match stroke.paint() {
            Paint::Color(color) => {
                flat.stroke = Some(FlatStroke {
                    color: (color.red, color.green, color.blue),
                    opacity: alpha,
                    width: stroke.width().get(),
                    linecap: stroke.linecap(),
                    linejoin: stroke.linejoin(),
                });
            }
            Paint::LinearGradient(_) | Paint::RadialGradient(_) | Paint::Pattern(_) => {
                return Err(format!(
                    "asset `{asset_id}`: stroke gradients and patterns are not supported"
                ));
            }
        }
    }

    if flat.fill.is_none() && flat.stroke.is_none() {
        return Err(format!("asset `{asset_id}`: path has no fill or stroke"));
    }

    out.push(flat);
    Ok(())
}

fn is_noop_clip(clip: &ClipPath, canvas: &Rect) -> bool {
    let Some(bounds) = clip_geometry_bounds(clip) else {
        return false;
    };
    rects_approx_equal(bounds, *canvas, CLIP_BOUNDS_TOLERANCE)
}

fn clip_geometry_bounds(clip: &ClipPath) -> Option<Rect> {
    let mut bounds: Option<Rect> = None;
    union_clip_group(clip.root(), &mut bounds);
    bounds
}

fn union_clip_group(group: &Group, bounds: &mut Option<Rect>) {
    for child in group.children() {
        match child {
            Node::Group(nested) => union_clip_group(nested, bounds),
            Node::Path(path) => merge_bounds(bounds, path.abs_bounding_box()),
            Node::Image(image) => merge_bounds(bounds, image.abs_bounding_box()),
            Node::Text(text) => merge_bounds(bounds, text.abs_bounding_box()),
        }
    }
}

fn merge_bounds(bounds: &mut Option<Rect>, next: Rect) {
    *bounds = Some(bounds.take().map_or(next, |existing| union_rects(existing, next)));
}

fn union_rects(a: Rect, b: Rect) -> Rect {
    Rect::from_ltrb(
        a.left().min(b.left()),
        a.top().min(b.top()),
        a.right().max(b.right()),
        a.bottom().max(b.bottom()),
    )
    .expect("union of finite clip bounds")
}

fn artboard_rect(tree: &Tree) -> Rect {
    Rect::from_xywh(0.0, 0.0, tree.size().width(), tree.size().height())
        .expect("tree size is positive after parse")
}

fn rects_approx_equal(a: Rect, b: Rect, tolerance: f32) -> bool {
    (a.left() - b.left()).abs() <= tolerance
        && (a.top() - b.top()).abs() <= tolerance
        && (a.right() - b.right()).abs() <= tolerance
        && (a.bottom() - b.bottom()).abs() <= tolerance
}

fn emit_minimal_svg(width: f32, height: f32, paths: &[FlatPath]) -> String {
    let width_s = trim_num(width);
    let height_s = trim_num(height);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width_s} {height_s}" width="{width_s}" height="{height_s}">"#
    );

    for path in paths {
        let fill = path.fill.filter(|(_, _, _, alpha)| *alpha > 0.0);
        let stroke = path.stroke.as_ref().filter(|s| s.opacity > 0.0);
        if fill.is_none() && stroke.is_none() {
            continue;
        }

        svg.push_str("<path ");
        match fill {
            Some((red, green, blue, alpha)) => {
                let _ = write!(svg, r#"fill="{}" "#, rgb_hex(red, green, blue));
                if alpha < 1.0 {
                    let _ = write!(svg, r#"fill-opacity="{}" "#, trim_num(alpha));
                }
            }
            None => svg.push_str(r#"fill="none" "#),
        }
        if let Some(stroke) = stroke {
            let (red, green, blue) = stroke.color;
            let _ = write!(svg, r#"stroke="{}" "#, rgb_hex(red, green, blue));
            let _ = write!(svg, r#"stroke-width="{}" "#, trim_num(stroke.width));
            if stroke.opacity < 1.0 {
                let _ = write!(svg, r#"stroke-opacity="{}" "#, trim_num(stroke.opacity));
            }
            if let Some(cap) = linecap_attr(stroke.linecap) {
                let _ = write!(svg, r#"stroke-linecap="{cap}" "#);
            }
            if let Some(join) = linejoin_attr(stroke.linejoin) {
                let _ = write!(svg, r#"stroke-linejoin="{join}" "#);
            }
        }
        let _ = write!(svg, r#"d="{}"/>"#, path.d);
    }

    svg.push_str("</svg>");
    svg
}

const fn linecap_attr(cap: LineCap) -> Option<&'static str> {
    match cap {
        LineCap::Butt => None,
        LineCap::Round => Some("round"),
        LineCap::Square => Some("square"),
    }
}

const fn linejoin_attr(join: LineJoin) -> Option<&'static str> {
    match join {
        LineJoin::Miter => None,
        LineJoin::MiterClip => Some("miter-clip"),
        LineJoin::Round => Some("round"),
        LineJoin::Bevel => Some("bevel"),
    }
}

fn rgb_hex(red: u8, green: u8, blue: u8) -> String {
    format!("#{red:02X}{green:02X}{blue:02X}")
}
