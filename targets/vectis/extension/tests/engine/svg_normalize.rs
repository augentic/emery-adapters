//! SVG vector normalization integration tests (re-homed from `src` unit tests).

use specify_vectis::materialize::{collect_paths, parse_vector_svg};

const TRIANGLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <path fill="#010203" d="M12 2L2 22h20z"/>
</svg>"##;

fn figma_clip_wrapper(width: f32, height: f32, inner: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}" height="{height}">
  <defs>
    <clipPath id="clip"><path d="M0 0h{width}v{height}H0z"/></clipPath>
  </defs>
  <g clip-path="url(#clip)">{inner}</g>
</svg>"#
    )
}

#[test]
fn parse_vector_svg_noop_clip_icon_sized() {
    let svg = figma_clip_wrapper(24.0, 24.0, r##"<path fill="#010203" d="M12 2L2 22h20z"/>"##);
    let parsed = parse_vector_svg(svg.as_bytes(), "icon").expect("parse");
    assert!(!parsed.tree.has_defs_nodes());
    let report = parsed.normalization.expect("normalized");
    assert!(report.transforms.contains(&"stripped-noop-clip"));
}

#[test]
fn parse_vector_svg_noop_clip_illustration_sized() {
    let svg =
        figma_clip_wrapper(240.0, 160.0, r##"<rect width="240" height="160" fill="#AABBCC"/>"##);
    let parsed = parse_vector_svg(svg.as_bytes(), "illus").expect("parse");
    assert!(!parsed.tree.has_defs_nodes());
}

#[test]
fn parse_vector_svg_noop_clip_launcher_sized() {
    let svg = figma_clip_wrapper(
        1024.0,
        1024.0,
        r##"<rect width="1024" height="1024" fill="#445566"/>"##,
    );
    let parsed = parse_vector_svg(svg.as_bytes(), "launcher").expect("parse");
    assert!(!parsed.tree.has_defs_nodes());
}

#[test]
fn parse_vector_svg_opacity_bake_preserves_rgb() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <g opacity="0.12"><circle cx="12" cy="12" r="8" fill="#1A73E8"/></g>
</svg>"##;
    let parsed = parse_vector_svg(svg.as_bytes(), "fade").expect("parse");
    let report = parsed.normalization.expect("normalized");
    assert!(report.transforms.contains(&"baked-group-opacity"));

    let mut paths = Vec::new();
    collect_paths(parsed.tree.root(), &mut paths);
    assert_eq!(paths.len(), 1);
    let (r, g, b, alpha) = paths[0].color;
    assert_eq!((r, g, b), (26, 115, 232));
    assert!((alpha - 0.12).abs() < 0.01);
}

#[test]
fn parse_vector_svg_gradient_still_fails() {
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
fn parse_vector_svg_real_clip_still_fails() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
  <defs><clipPath id="c"><rect width="12" height="12"/></clipPath></defs>
  <g clip-path="url(#c)"><path fill="#000" d="M0 0h24v24z"/></g>
</svg>"##;
    let err = parse_vector_svg(svg.as_bytes(), "clip").unwrap_err();
    assert!(err.contains("clip"));
}

#[test]
fn parse_vector_svg_clean_tree_skips_normalization() {
    let parsed = parse_vector_svg(TRIANGLE.as_bytes(), "tri").expect("parse");
    assert!(parsed.normalization.is_none());
}
