//! Is this ui-ir design system safe to emit?
//!
//! Copied from vx-spec (`augentic/vx`, `crates/vx-spec`) and trimmed to what the Vue emit needs:
//! the holes a source adapter leaves, and the cross-document checks no JSON Schema can express.
//! The schemas themselves are ui-ir's, and `ui-ir check` runs them where the IR is written; so are
//! vx's own `capabilities.yaml`, `behaviour.yaml` and `components.yaml`, which did not come
//! across. Documents are read as JSON only, from the ui-ir layout's names.

use std::path::Path;

use serde_json::Value;

pub mod assets;
pub mod composition;
pub mod holes;
pub(crate) mod shared;

pub use composition::Finding;
pub use composition::formats::FORMAT_STYLES;
pub use holes::Hole;

/// The composition's filename, relative to the design-system directory.
pub const COMPOSITION: &str = ui_ir::layout::COMPOSITION;
/// The token manifest's filename.
pub const TOKENS: &str = ui_ir::layout::TOKENS;
/// The asset inventory's filename.
pub const ASSETS: &str = ui_ir::layout::ASSETS;

/// What validating a design system produced.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// Everything that makes the spec invalid.
    pub errors: Vec<Finding>,

    /// Everything worth saying that does not make the spec invalid.
    pub warnings: Vec<Finding>,

    /// Decisions the source did not contain (`TODO(...)` markers).
    ///
    /// Kept apart from [`Self::errors`]: a hole means the spec is incomplete by design, which is
    /// the expected state of a fresh extract, not that it is wrong.
    pub holes: Vec<Hole>,
}

impl Report {
    /// Whether the spec has nothing wrong with it. Holes do not count.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.errors.is_empty()
    }

    /// Whether the spec is both valid and complete, which the emit requires.
    #[must_use]
    pub const fn is_filled(&self) -> bool {
        self.is_clean() && self.holes.is_empty()
    }

    /// The errors rendered one per line.
    #[must_use]
    pub fn render(&self) -> String {
        self.errors
            .iter()
            .map(|finding| format!("- `{}`: {}", finding.path(), finding.message()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Validates a ui-ir design-system directory.
///
/// Reads `tokens.json`, `assets.json` and `composition.json` where present and runs the checks
/// over them: holes, typefaces a type style names, and the composition's cross-document checks.
///
/// # Errors
///
/// When a document exists and cannot be read or parsed.
pub fn validate(dir: &Path) -> Result<Report, crate::Error> {
    let mut report = Report::default();

    let tokens = optional(&dir.join(TOKENS))?;
    let assets = optional(&dir.join(ASSETS))?;
    if let Some(tokens) = &tokens {
        report.holes.extend(holes::find(tokens, TOKENS));
        check_typefaces(tokens, &mut report);
    }
    if let Some(assets) = &assets {
        report.holes.extend(holes::find(assets, ASSETS));
    }

    let Some(composition) = optional(&dir.join(COMPOSITION))? else {
        report.errors.push(Finding::new(
            "",
            format!("no {COMPOSITION} in {} -- nothing to validate", dir.display()),
        ));
        return Ok(report);
    };
    report.holes.extend(holes::find(&composition, COMPOSITION));

    if composition.get("delta").is_some() {
        report.errors.push(Finding::new(
            "/delta",
            "this composition is a `delta:` document, which describes a change against a merged \
             baseline there is none of here: author the full `screens:` form instead",
        ));
        return Ok(report);
    }

    composition::check(&composition, tokens.as_ref(), assets.as_ref(), &mut report);
    let mut bound = Vec::new();
    composition::variants::check_bound_params(&composition, None, &mut bound);
    report.errors.extend(bound);

    Ok(report)
}

fn optional(path: &Path) -> Result<Option<Value>, crate::Error> {
    if path.exists() { crate::read(path).map(Some) } else { Ok(None) }
}

fn check_typefaces(tokens: &Value, report: &mut Report) {
    let declared = tokens.get("typefaces").and_then(Value::as_object);
    let Some(styles) = tokens.get("typography").and_then(Value::as_object) else { return };

    for (style, entry) in styles {
        let Some(slug) = entry.get("typeface").and_then(Value::as_str) else { continue };
        // A hole is reported as a hole, and a second finding about it would bury the first.
        if slug.starts_with("TODO(") || declared.is_some_and(|faces| faces.contains_key(slug)) {
            continue;
        }
        report.errors.push(Finding::new(
            format!("/typography/{}/typeface", shared::escape_pointer_token(style)),
            format!(
                "{TOKENS}: type style `{style}` names typeface `{slug}`, which `typefaces` does \
                 not declare -- it would render in the platform's default face",
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_composition_is_a_finding_not_an_error() {
        let empty = tempfile::tempdir().expect("a temporary directory");
        let report = validate(empty.path()).expect("a missing composition is not an error");
        assert!(!report.is_clean());
        assert!(report.render().contains("nothing to validate"), "{}", report.render());
    }

    #[test]
    fn an_undeclared_typeface_is_an_error() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        std::fs::write(
            dir.path().join(TOKENS),
            r#"{"typography": {"body": {"typeface": "inter"}}}"#,
        )
        .expect("tokens");
        std::fs::write(dir.path().join(COMPOSITION), r#"{"screens": {}}"#).expect("composition");
        let report = validate(dir.path()).expect("validates");
        assert!(report.render().contains("typeface `inter`"), "{}", report.render());
    }
}
