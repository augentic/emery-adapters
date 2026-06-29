//! Flatten Figma-export noise (no-op clips, group opacity) into export-clean SVG.

use std::fmt::Write;

use usvg::tiny_skia_path::Rect;
use usvg::{BlendMode, ClipPath, Group, Node, Paint, Path, Tree};

use super::{absolute_path, path_data_string, trim_num};

const CLIP_BOUNDS_TOLERANCE: f32 = 0.5;

/// Transforms applied during vector SVG normalization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NormalizeReport {
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
/// Returns `Ok(None)` when the tree is already export-clean.
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

struct FlatPath {
    d: String,
    fill: Option<(u8, u8, u8, f32)>,
    stroke: Option<(u8, u8, u8, f32)>,
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

    let mut flat = FlatPath { d, fill: None, stroke: None };

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
                flat.stroke = Some((color.red, color.green, color.blue, alpha));
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
    *bounds = Some(match bounds.take() {
        None => next,
        Some(existing) => union_rects(existing, next),
    });
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
    let w = trim_num(width);
    let h = trim_num(height);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">"#
    );

    for path in paths {
        if let Some((r, g, b, alpha)) = path.fill
            && alpha > 0.0
        {
            let _ = write!(svg, r#"<path fill="{}" "#, rgb_hex(r, g, b));
            if alpha < 1.0 {
                let _ = write!(svg, r#"fill-opacity="{}" "#, trim_num(alpha));
            }
            let _ = write!(svg, r#"d="{}"/>"#, path.d);
        }

        if let Some((r, g, b, alpha)) = path.stroke
            && alpha > 0.0
        {
            let _ = write!(svg, r#"<path fill="none" stroke="{}" "#, rgb_hex(r, g, b));
            if alpha < 1.0 {
                let _ = write!(svg, r#"stroke-opacity="{}" "#, trim_num(alpha));
            }
            let _ = write!(svg, r#"d="{}"/>"#, path.d);
        }
    }

    svg.push_str("</svg>");
    svg
}

fn rgb_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

#[cfg(test)]
mod tests {
    use usvg::Tree;

    use super::*;
    use crate::materialize::svg::parse_vector_svg;

    const TRIANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path fill="#010203" d="M12 2L2 22h20z"/>
</svg>"##;

    fn figma_clip_wrapper(width: f32, height: f32, inner: &str) -> String {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}">
  <defs>
    <clipPath id="clip"><path d="M0 0h{width}v{height}H0z"/></clipPath>
  </defs>
  <g clip-path="url(#clip)">{inner}</g>
</svg>"##
        )
    }

    #[test]
    fn noop_clip_icon_sized() {
        let svg = figma_clip_wrapper(
            24.0,
            24.0,
            r##"<path fill="#010203" d="M12 2L2 22h20z"/>"##,
        );
        let parsed = parse_vector_svg(svg.as_bytes(), "icon").expect("parse");
        assert!(!parsed.tree.has_defs_nodes());
        let report = parsed.normalization.expect("normalized");
        assert!(report.transforms.contains(&"stripped-noop-clip"));
    }

    #[test]
    fn noop_clip_illustration_sized() {
        let svg = figma_clip_wrapper(
            240.0,
            160.0,
            r##"<rect width="240" height="160" fill="#AABBCC"/>"##,
        );
        let parsed = parse_vector_svg(svg.as_bytes(), "illus").expect("parse");
        assert!(!parsed.tree.has_defs_nodes());
    }

    #[test]
    fn noop_clip_launcher_sized() {
        let svg = figma_clip_wrapper(
            1024.0,
            1024.0,
            r##"<rect width="1024" height="1024" fill="#445566"/>"##,
        );
        let parsed = parse_vector_svg(svg.as_bytes(), "launcher").expect("parse");
        assert!(!parsed.tree.has_defs_nodes());
    }

    #[test]
    fn opacity_bake_preserves_rgb() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <g opacity="0.12"><circle cx="12" cy="12" r="8" fill="#1A73E8"/></g>
</svg>"##;
        let parsed = parse_vector_svg(svg.as_bytes(), "fade").expect("parse");
        let report = parsed.normalization.expect("normalized");
        assert!(report.transforms.contains(&"baked-group-opacity"));

        let mut paths = Vec::new();
        crate::materialize::svg::collect_paths(parsed.tree.root(), &mut paths);
        assert_eq!(paths.len(), 1);
        let (r, g, b, alpha) = paths[0].color;
        assert_eq!((r, g, b), (26, 115, 232));
        assert!((alpha - 0.12).abs() < 0.01);
    }

    #[test]
    fn gradient_still_fails_after_clip_strip_attempt() {
        let svg = figma_clip_wrapper(
            24.0,
            24.0,
            r##"<defs><linearGradient id="g"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs><rect width="24" height="24" fill="url(#g)"/>"##,
        );
        let err = parse_vector_svg(svg.as_bytes(), "grad").unwrap_err();
        assert!(err.contains("grad"));
        assert!(err.contains("gradient"));
    }

    #[test]
    fn real_clip_still_fails() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <defs><clipPath id="c"><rect width="12" height="12"/></clipPath></defs>
  <g clip-path="url(#c)"><path fill="#000" d="M0 0h24v24z"/></g>
</svg>"##;
        let err = parse_vector_svg(svg.as_bytes(), "clip").unwrap_err();
        assert!(err.contains("clip"));
    }

    #[test]
    fn clean_tree_skips_normalization() {
        let opt = usvg::Options::default();
        let tree = Tree::from_data(TRIANGLE.as_bytes(), &opt).expect("parse");
        let result = normalize_for_export(&tree, "tri").expect("normalize");
        assert!(result.is_none());
    }
}
