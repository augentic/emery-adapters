//! A comparison with what a reviewer reads alongside it: overlays at the
//! review widths and at the worst widths, and findings in plain sentences.

use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::{
    Comparison, Error, Facts, ItemKind, PASS_MARK, REVIEW_WIDTHS, Width, compare, overlay,
};

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
    /// The name of the overlay at `width` in a written review.
    #[must_use]
    pub fn overlay_name(width: Width) -> String {
        format!("overlay-{}.svg", width.0)
    }

    /// Writes `score.json` and every overlay, under [`overlay_name`](Self::overlay_name),
    /// into `dir`, creating it, and removes any overlay an earlier review left there.
    ///
    /// # Errors
    ///
    /// When `dir` cannot be read or a file in it written or removed.
    pub fn write(&self, dir: &Path) -> Result<(), Error> {
        let failed = |doing: &str, path: &Path, err: std::io::Error| {
            Error::new(format!("{doing} {}: {err}", path.display()))
        };
        fs::create_dir_all(dir).map_err(|err| failed("creating", dir, err))?;
        for entry in fs::read_dir(dir).map_err(|err| failed("reading", dir, err))?.flatten() {
            let path = entry.path();
            let overlay = entry.file_name().to_string_lossy().starts_with("overlay-")
                && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));
            if overlay {
                fs::remove_file(&path).map_err(|err| failed("removing", &path, err))?;
            }
        }
        let score = serde_json::to_string_pretty(&self.comparison)
            .map_err(|err| Error::new(format!("serialising the score: {err}")))?;
        let to = dir.join("score.json");
        fs::write(&to, score + "\n").map_err(|err| failed("writing", &to, err))?;
        for (width, svg) in &self.overlays {
            let to = dir.join(Self::overlay_name(*width));
            fs::write(&to, svg).map_err(|err| failed("writing", &to, err))?;
        }
        Ok(())
    }

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

    /// What a failing attempt found, in terms the agent can act on: the
    /// summary, the original items that differ by their node id in `facts`
    /// (a text also by its words; no path, so a refactor learns no
    /// structure), and the render's items that pair with nothing by their
    /// own label. Shaped as the demo harness's `feedback/attempt-<n>.json`.
    #[must_use]
    pub fn feedback(&self, attempt: usize, facts: &str) -> Value {
        let comparison = &self.comparison;
        let differs: Vec<Value> = comparison
            .worst_original_items
            .iter()
            .map(|failure| {
                let mut item = json!({ "id": failure.id, "kind": failure.kind });
                if failure.kind == ItemKind::Text.name() {
                    item["text"] = json!(failure.label);
                }
                item["why"] = json!(failure.why);
                item["at"] = json!(format!("{}..{}", failure.first, failure.last));
                item
            })
            .collect();
        let only_in_render: Vec<Value> = comparison
            .unpaired_render_items
            .iter()
            .map(|failure| {
                json!({
                    "kind": failure.kind,
                    "label": failure.label,
                    "why": failure.why,
                    "at": format!("{}..{}", failure.first, failure.last),
                })
            })
            .collect();
        json!({
            "attempt": attempt,
            "about": format!(
                "your page rendered and compared with {facts} at every width and height. \
                 differs: items of {facts} that do not match; only_in_render: painted boxes and \
                 texts of your page that match nothing in {facts}, each adding its area against \
                 the score even when it looks the same. id: node id in {facts}. why: the first \
                 difference, original->render; for a painted box without a partner, against the \
                 box of the other side that overlaps it most. at: first..last viewport (W or \
                 WxH) where it differs."
            ),
            "score": { "pass_mark": comparison.pass_mark, "summary": comparison.summary },
            "differs": differs,
            "only_in_render": only_in_render,
        })
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
