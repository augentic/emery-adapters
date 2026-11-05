//! A comparison with what a reviewer reads alongside it: overlays at the
//! review widths and at the worst widths, and findings in plain sentences.

use crate::{Comparison, Error, Facts, PASS_MARK, REVIEW_WIDTHS, Width, compare, overlay};

// The most failing items a set of findings names.
const NAMED_FAILURES: usize = 10;

/// A comparison of a render against an original, with its overlays.
#[derive(Debug, Clone)]
pub struct Review {
    /// The comparison, shaped as `score.json`.
    pub comparison: Comparison,
    /// An [`overlay`] at each review width the comparison covered and at
    /// each kind's lowest-scoring width, in width order.
    pub overlays: Vec<(Width, String)>,
}

/// Compares `render` against `original` and draws the overlays.
///
/// # Errors
///
/// When the two cover no common width.
pub fn review(original: &Facts, render: &Facts) -> Result<Review, Error> {
    let comparison = compare(original, render)?;
    let (lo, hi) = (comparison.widths["lo"], comparison.widths["hi"]);
    let mut widths: Vec<Width> =
        REVIEW_WIDTHS.into_iter().filter(|width| (lo..=hi).contains(&width.0)).collect();
    widths.extend(worst_widths(&comparison));
    widths.sort_unstable();
    widths.dedup();
    let overlays =
        widths.into_iter().map(|width| (width, overlay(original, render, width))).collect();
    Ok(Review { comparison, overlays })
}

impl Review {
    /// What a failing comparison found, one sentence each, naming the
    /// overlay at each drawn width through `overlay_path`. Empty when the
    /// comparison passes.
    pub fn findings(&self, overlay_path: impl Fn(Width) -> String) -> Vec<String> {
        let comparison = &self.comparison;
        if comparison.pass {
            return Vec::new();
        }
        let mut findings = Vec::new();
        for (over, summary) in &comparison.summary {
            let Some(summary) = summary else { continue };
            for (kind, scores) in summary {
                if scores.min < PASS_MARK {
                    findings.push(format!(
                        "{kind} over the {over} scores {} at viewport {} (mean {}), below the pass mark {PASS_MARK}",
                        scores.min, scores.min_at, scores.mean
                    ));
                }
            }
        }
        for failure in comparison.worst_original_items.iter().take(NAMED_FAILURES) {
            findings.push(format!(
                "original {} `{}` does not hold at {} viewports from {} to {}: {}",
                failure.kind,
                failure.label,
                failure.points,
                failure.first,
                failure.last,
                failure.why
            ));
        }
        for failure in comparison.unpaired_render_items.iter().take(NAMED_FAILURES) {
            findings.push(format!(
                "render {} `{}` pairs with nothing at {} viewports from {} to {}: {}",
                failure.kind,
                failure.label,
                failure.points,
                failure.first,
                failure.last,
                failure.why
            ));
        }
        for (width, _) in &self.overlays {
            findings.push(format!("overlay at {width}: {}", overlay_path(*width)));
        }
        findings
    }
}

// The width each kind scores lowest at over the widths.
fn worst_widths(comparison: &Comparison) -> Vec<Width> {
    comparison
        .summary
        .get("widths")
        .and_then(Option::as_ref)
        .map(|summary| {
            summary.values().filter_map(|kind| kind.min_at.parse().ok().map(Width)).collect()
        })
        .unwrap_or_default()
}
