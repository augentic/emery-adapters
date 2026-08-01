//! SVG load and lightweight profile checks for vector materialization.

mod normalize;

use std::fmt::Write;

pub use normalize::{NormalizeReport, normalize_for_export};
use usvg::tiny_skia_path::{Path, PathSegment};
use usvg::{LineCap, LineJoin, Node, Paint, Tree};

/// Parsed SVG ready for platform export.
#[derive(Debug)]
pub struct ParsedSvg {
    /// The parsed usvg render tree.
    pub tree: Tree,
    /// SVG normalization applied during parse, when any.
    pub normalization: Option<NormalizeReport>,
}

/// Load and validate an SVG master for vector export (icons, illustrations, app-icon SVG).
///
/// # Errors
///
/// Returns a human-readable message naming the asset when the SVG uses
/// unsupported features (gradients, text, filters, embedded images, …).
pub fn parse_vector_svg(svg_bytes: &[u8], asset_id: &str) -> Result<ParsedSvg, String> {
    let opt = usvg::Options::default();
    let tree = Tree::from_data(svg_bytes, &opt)
        .map_err(|err| format!("asset `{asset_id}`: SVG parse failed: {err}"))?;

    let (tree, normalization) = match normalize_for_export(&tree, asset_id)? {
        None => (tree, None),
        Some((bytes, report)) => {
            let tree = Tree::from_data(&bytes, &opt)
                .map_err(|err| format!("asset `{asset_id}`: SVG re-parse failed: {err}"))?;
            (tree, Some(report))
        }
    };

    validate_profile(&tree, asset_id)?;
    if !tree_has_drawable_paths(tree.root()) {
        return Err(format!("asset `{asset_id}`: SVG contains no drawable paths"));
    }

    Ok(ParsedSvg { tree, normalization })
}

fn validate_profile(tree: &Tree, asset_id: &str) -> Result<(), String> {
    if tree.has_text_nodes() {
        return Err(format!("asset `{asset_id}`: text nodes are not supported"));
    }
    if tree.has_defs_nodes() {
        return Err(format!(
            "asset `{asset_id}`: gradients, patterns, clip paths, masks, or filters are not supported"
        ));
    }
    walk_profile(tree.root(), asset_id)
}

fn walk_profile(group: &usvg::Group, asset_id: &str) -> Result<(), String> {
    if group.opacity().get() < 1.0 {
        return Err(format!("asset `{asset_id}`: group opacity is not supported"));
    }
    if group.blend_mode() != usvg::BlendMode::Normal {
        return Err(format!("asset `{asset_id}`: non-normal blend modes are not supported"));
    }
    if group.clip_path().is_some() || group.mask().is_some() || !group.filters().is_empty() {
        return Err(format!(
            "asset `{asset_id}`: clip paths, masks, and filters are not supported"
        ));
    }

    for child in group.children() {
        match child {
            Node::Group(nested) => walk_profile(nested, asset_id)?,
            Node::Path(path) => validate_path(path, asset_id)?,
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

fn validate_path(path: &usvg::Path, asset_id: &str) -> Result<(), String> {
    if !path.is_visible() {
        return Ok(());
    }
    if let Some(fill) = path.fill() {
        ensure_solid_paint(fill.paint(), asset_id, "fill")?;
    }
    if let Some(stroke) = path.stroke() {
        ensure_solid_paint(stroke.paint(), asset_id, "stroke")?;
    }
    if path.fill().is_none() && path.stroke().is_none() {
        return Err(format!("asset `{asset_id}`: path has no fill or stroke"));
    }
    Ok(())
}

fn ensure_solid_paint(paint: &Paint, asset_id: &str, kind: &str) -> Result<(), String> {
    match paint {
        Paint::Color(_) => Ok(()),
        Paint::LinearGradient(_) | Paint::RadialGradient(_) | Paint::Pattern(_) => {
            Err(format!("asset `{asset_id}`: {kind} gradients and patterns are not supported"))
        }
    }
}

fn tree_has_drawable_paths(group: &usvg::Group) -> bool {
    group.children().iter().any(|node| match node {
        Node::Group(nested) => tree_has_drawable_paths(nested),
        Node::Path(path) => path.is_visible(),
        Node::Image(_) | Node::Text(_) => false,
    })
}

/// Absolute canvas-space path data for a `usvg` path node.
#[must_use]
pub fn absolute_path(path: &usvg::Path) -> Option<Path> {
    path.data().clone().transform(path.abs_transform())
}

/// Format path segments as Android `pathData` (SVG `d` syntax).
#[must_use]
pub fn path_data_string(path: &Path) -> String {
    let mut out = String::new();
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(p) => {
                append_coord(&mut out, 'M', p.x, p.y);
            }
            PathSegment::LineTo(p) => {
                append_coord(&mut out, 'L', p.x, p.y);
            }
            PathSegment::QuadTo(p0, p1) => {
                let _ = write!(
                    out,
                    "Q{},{},{},{} ",
                    trim_num(p0.x),
                    trim_num(p0.y),
                    trim_num(p1.x),
                    trim_num(p1.y)
                );
            }
            PathSegment::CubicTo(p0, p1, p2) => {
                let _ = write!(
                    out,
                    "C{},{},{},{},{},{} ",
                    trim_num(p0.x),
                    trim_num(p0.y),
                    trim_num(p1.x),
                    trim_num(p1.y),
                    trim_num(p2.x),
                    trim_num(p2.y)
                );
            }
            PathSegment::Close => out.push('Z'),
        }
    }
    out.trim().to_string()
}

fn append_coord(out: &mut String, verb: char, x: f32, y: f32) {
    let _ = write!(out, "{verb}{} {} ", trim_num(x), trim_num(y));
}

pub(super) fn trim_num(value: f32) -> String {
    let rounded = format!("{value:.4}");
    rounded.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Stroke line cap for platform export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeCap {
    /// Flat ends.
    Butt,
    /// Round ends.
    Round,
    /// Square projecting ends.
    Square,
}

/// Stroke line join for platform export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeJoin {
    /// Mitered corners.
    Miter,
    /// Round corners.
    Round,
    /// Bevelled corners.
    Bevel,
}

/// Solid stroke paint plus geometry style.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokePaint {
    /// RGB colour.
    pub color: (u8, u8, u8),
    /// Opacity in `0.0..=1.0`.
    pub opacity: f32,
    /// Stroke width in canvas units.
    pub width: f32,
    /// Line cap.
    pub linecap: StrokeCap,
    /// Line join.
    pub linejoin: StrokeJoin,
}

/// Canvas-space path plus optional solid fill and stroke.
#[derive(Debug, Clone)]
pub struct DrawablePath {
    /// Path geometry in canvas coordinates.
    pub geometry: Path,
    /// Solid RGB fill plus opacity, when filled.
    pub fill: Option<(u8, u8, u8, f32)>,
    /// Solid stroke paint, when stroked.
    pub stroke: Option<StrokePaint>,
}

/// Collect drawable paths in paint order for export backends.
pub fn collect_paths(group: &usvg::Group, out: &mut Vec<DrawablePath>) {
    for child in group.children() {
        match child {
            Node::Group(nested) => collect_paths(nested, out),
            Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }
                if let Some(drawable) = drawable_from_path(path) {
                    out.push(drawable);
                }
            }
            Node::Image(_) | Node::Text(_) => {}
        }
    }
}

fn drawable_from_path(path: &usvg::Path) -> Option<DrawablePath> {
    let geometry = absolute_path(path)?;
    let fill = solid_fill(path);
    let stroke = solid_stroke(path);
    if fill.is_none() && stroke.is_none() {
        return None;
    }
    Some(DrawablePath {
        geometry,
        fill,
        stroke,
    })
}

fn solid_fill(path: &usvg::Path) -> Option<(u8, u8, u8, f32)> {
    let fill = path.fill()?;
    let Paint::Color(color) = fill.paint() else {
        return None;
    };
    Some((color.red, color.green, color.blue, fill.opacity().get()))
}

fn solid_stroke(path: &usvg::Path) -> Option<StrokePaint> {
    let stroke = path.stroke()?;
    let Paint::Color(color) = stroke.paint() else {
        return None;
    };
    Some(StrokePaint {
        color: (color.red, color.green, color.blue),
        opacity: stroke.opacity().get(),
        width: canvas_stroke_width(path, stroke.width().get()),
        linecap: map_linecap(stroke.linecap()),
        linejoin: map_linejoin(stroke.linejoin()),
    })
}

/// Scale a local stroke width into canvas units using `abs_transform`.
///
/// Geometry is baked through [`absolute_path`]; stroke width must use the same
/// scale or transformed icons export the wrong weight.
pub(super) fn canvas_stroke_width(path: &usvg::Path, local_width: f32) -> f32 {
    let (scale_x, scale_y) = path.abs_transform().get_scale();
    local_width * (scale_x + scale_y) / 2.0
}

const fn map_linecap(cap: LineCap) -> StrokeCap {
    match cap {
        LineCap::Butt => StrokeCap::Butt,
        LineCap::Round => StrokeCap::Round,
        LineCap::Square => StrokeCap::Square,
    }
}

const fn map_linejoin(join: LineJoin) -> StrokeJoin {
    match join {
        LineJoin::Miter | LineJoin::MiterClip => StrokeJoin::Miter,
        LineJoin::Round => StrokeJoin::Round,
        LineJoin::Bevel => StrokeJoin::Bevel,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub const TRIANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
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
    fn parse_matrix() {
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
    fn stroke_normalize_preserved() {
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
    fn stroke_width_scales() {
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
        assert!((stroke.width - 4.0).abs() < 0.01, "expected canvas width 4, got {}", stroke.width);
    }
}
