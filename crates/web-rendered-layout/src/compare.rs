//! Scores how closely a render's facts hold an original's, at every viewport both cover.
//!
//! Items of one kind are paired in visual order by signature (a longest
//! common subsequence). A pair holds at a viewport when it has the same
//! number of lines, every box's size is within max(1px, 10%) of the
//! original's, and its position relative to the nearest paired painted box
//! containing it is within max(1px, 5% of that box) (the viewport width and
//! page height where none does). The score of a kind at a viewport is the
//! area of holding originals over the area of all originals plus the area of
//! unpaired render items. A comparison passes when the lowest score of each
//! kind over every viewport is at least [`PASS_MARK`].

use std::collections::BTreeMap;

use serde::Serialize;

use crate::facts::{Rect, js_round};
use crate::items::{PAINT_FIELDS, TEXT_FIELDS, TEXT_SEPARATOR};
use crate::{Error, Facts, Item, ItemKind, Viewport, Width, items};

/// The lowest score of each kind a passing comparison holds at every viewport.
pub const PASS_MARK: f64 = 0.7;

const WORST: usize = 25;

/// The scores at one viewport.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Point {
    /// The text score, 0 to 1.
    pub text: f64,
    /// The paint score, 0 to 1.
    pub paint: f64,
    /// The page height of the original and the render, rounded.
    pub page_height: [f64; 2],
    /// Item counts by kind: original, render.
    pub items: BTreeMap<&'static str, [usize; 2]>,
}

impl Point {
    /// The score of `kind`.
    #[must_use]
    pub const fn score(&self, kind: ItemKind) -> f64 {
        match kind {
            ItemKind::Text => self.text,
            ItemKind::Paint => self.paint,
        }
    }
}

/// One item that did not hold, with where and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Failure {
    /// The item's id on its own side.
    pub id: String,
    /// `text` or `paint`.
    pub kind: &'static str,
    /// The item's label.
    pub label: String,
    /// How many viewports it failed at.
    pub points: usize,
    /// The first viewport it failed at.
    pub first: String,
    /// The last viewport it failed at.
    pub last: String,
    /// Why it failed at the first.
    pub why: String,
    /// Its area summed over every viewport it failed at.
    pub area: f64,
}

/// The lowest and mean score of one kind over a set of viewports.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KindSummary {
    /// The lowest score.
    pub min: f64,
    /// The first viewport holding the lowest score.
    pub min_at: String,
    /// The mean score, to four places.
    pub mean: f64,
}

/// The scores of both kinds over a set of viewports.
pub type Summary = BTreeMap<&'static str, KindSummary>;

/// A whole comparison, shaped as the scorer's `score.json`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Comparison {
    /// The widths compared, low and high.
    pub widths: BTreeMap<&'static str, u32>,
    /// The heights compared and the widths they were compared at.
    pub heights: Heights,
    /// The pass mark, [`PASS_MARK`].
    pub pass_mark: f64,
    /// The summary over the widths, and over the heights where any were compared.
    pub summary: BTreeMap<&'static str, Option<Summary>>,
    /// Whether every kind's lowest score is at least the pass mark.
    pub pass: bool,
    /// The original items failing over the largest area, at most 25.
    pub worst_original_items: Vec<Failure>,
    /// The render items paired with nothing over the largest area, at most 25.
    pub unpaired_render_items: Vec<Failure>,
    /// The scores at every width, at the facts' height.
    pub by_width: BTreeMap<u32, Point>,
    /// The summary over the heights at each swept width.
    pub by_height: BTreeMap<u32, Option<Summary>>,
}

/// The heights a comparison covered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Heights {
    /// The lowest height.
    pub lo: u32,
    /// The highest height.
    pub hi: u32,
    /// The widths the heights were compared at.
    pub at_widths: Vec<u32>,
}

impl Comparison {
    /// The lowest score of any kind over every viewport compared.
    #[must_use]
    pub fn lowest(&self) -> f64 {
        self.summary
            .values()
            .flatten()
            .flat_map(|summary| summary.values().map(|kind| kind.min))
            .fold(1.0, f64::min)
    }
}

/// What a comparison found at one viewport, item by item.
#[derive(Debug, Clone, PartialEq)]
pub struct PointComparison {
    /// The scores.
    pub point: Point,
    /// The original items that did not hold, each with why.
    pub failed: Vec<(Item, String)>,
    /// The render items paired with nothing, each with why.
    pub unpaired: Vec<(Item, String)>,
}

/// Compares `render` against `original` at every integer width both cover,
/// and every integer height both cover at each width both swept heights at.
///
/// # Errors
///
/// When the two cover no common width.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "viewport sizes are small positive integers"
)]
pub fn compare(original: &Facts, render: &Facts) -> Result<Comparison, Error> {
    let (olo, ohi) = original.width_range();
    let (rlo, rhi) = render.width_range();
    let lo = olo.max(rlo).ceil();
    let hi = ohi.min(rhi).floor();
    if lo > hi {
        return Err(Error::new(format!(
            "the width ranges do not overlap: {olo}..{ohi} and {rlo}..{rhi}"
        )));
    }
    let (lo, hi) = (lo as u32, hi as u32);
    let (original_low, original_high) = original.height_range();
    let (render_low, render_high) = render.height_range();
    let hlo = original_low.max(render_low).ceil() as u32;
    let hhi = original_high.min(render_high).floor() as u32;
    let rendered: Vec<Width> = render.swept_widths();
    let swept: Vec<Width> = original
        .swept_widths()
        .into_iter()
        .filter(|width| rendered.contains(width) && (lo..=hi).contains(&width.0))
        .collect();

    let mut tally = Tally::default();
    let mut by_width = BTreeMap::new();
    for width in lo..=hi {
        let viewport = Viewport::at(Width(width));
        by_width.insert(width, tally.point(original, render, viewport));
    }
    let mut height_rows: Vec<(String, Point)> = Vec::new();
    let mut by_height = BTreeMap::new();
    for width in &swept {
        let mut rows = Vec::new();
        for height in hlo..=hhi {
            let viewport = Viewport {
                width: *width,
                height: Some(height),
            };
            rows.push((height.to_string(), tally.point(original, render, viewport)));
        }
        by_height.insert(width.0, summarise(&rows));
        height_rows.extend(
            rows.into_iter().map(|(height, point)| (format!("{}x{height}", width.0), point)),
        );
    }

    let width_rows: Vec<(String, Point)> =
        by_width.iter().map(|(width, point)| (width.to_string(), point.clone())).collect();
    let summary =
        BTreeMap::from([("widths", summarise(&width_rows)), ("heights", summarise(&height_rows))]);
    let pass = summary.values().flatten().all(|s| s.values().all(|kind| kind.min >= PASS_MARK));
    Ok(Comparison {
        widths: BTreeMap::from([("lo", lo), ("hi", hi)]),
        heights: Heights {
            lo: hlo,
            hi: hhi,
            at_widths: swept.iter().map(|width| width.0).collect(),
        },
        pass_mark: PASS_MARK,
        summary,
        pass,
        worst_original_items: tally.failed.worst(),
        unpaired_render_items: tally.unpaired.worst(),
        by_width,
        by_height,
    })
}

/// Compares `render` against `original` at `viewport` alone, item by item.
#[must_use]
#[expect(clippy::too_many_lines, reason = "the pairing and hold rules read best as one pass")]
pub fn compare_at(original: &Facts, render: &Facts, viewport: Viewport) -> PointComparison {
    let originals = items(original, viewport);
    let renders = items(render, viewport);
    let page =
        |facts: &Facts| Facts::box_at(facts.node(facts.root()), viewport).map_or(0.0, |r| r[3]);
    let (page_original, page_render) = (page(original), page(render));
    let width = f64::from(viewport.width.0);

    let mut matched: BTreeMap<usize, usize> = BTreeMap::new();
    for kind in ItemKind::ALL {
        let a = indices(&originals, kind);
        let b = indices(&renders, kind);
        let left: Vec<&Item> = a.iter().map(|&i| &originals[i]).collect();
        let right: Vec<&Item> = b.iter().map(|&j| &renders[j]).collect();
        for (i, j) in pair(&left, &right) {
            matched.insert(a[i], b[j]);
        }
    }
    let painted: Vec<usize> = indices(&originals, ItemKind::Paint)
        .into_iter()
        .filter(|i| matched.contains_key(i))
        .collect();
    let used: Vec<bool> = {
        let mut used = vec![false; renders.len()];
        for &j in matched.values() {
            used[j] = true;
        }
        used
    };
    let original_paint: Vec<&Item> =
        originals.iter().filter(|o| o.kind == ItemKind::Paint).collect();
    let unpaired_paint: Vec<&Item> = renders
        .iter()
        .enumerate()
        .filter(|(j, r)| r.kind == ItemKind::Paint && !used[*j])
        .map(|(_, r)| r)
        .collect();

    let mut point = Point {
        text: 1.0,
        paint: 1.0,
        page_height: [js_round(page_original), js_round(page_render)],
        items: ItemKind::ALL
            .iter()
            .map(|&kind| {
                (kind.name(), [indices(&originals, kind).len(), indices(&renders, kind).len()])
            })
            .collect(),
    };
    let mut failed = Vec::new();
    let mut unpaired = Vec::new();
    for kind in ItemKind::ALL {
        let mut held = 0.0;
        let mut total = 0.0;
        let mut extra = 0.0;
        for (i, item) in originals.iter().enumerate() {
            if item.kind != kind {
                continue;
            }
            total += item.area;
            let why = match matched.get(&i) {
                None if kind == ItemKind::Text => text_mismatch(item, &renders),
                None => paint_mismatch(item, &unpaired_paint, true),
                Some(&j) => {
                    let container = painted
                        .iter()
                        .copied()
                        .filter(|&p| p != i && contains(originals[p].bounds, item.bounds))
                        .fold(None, |best: Option<usize>, p| match best {
                            Some(c) if originals[p].area >= originals[c].area => Some(c),
                            _ => Some(p),
                        });
                    let (frame_original, frame_render) = container.map_or(
                        ([0.0, 0.0, width, page_original], [0.0, 0.0, width, page_render]),
                        |c| (originals[c].bounds, renders[matched[&c]].bounds),
                    );
                    holding(item, &renders[j], frame_original, frame_render)
                }
            };
            if why.is_empty() {
                held += item.area;
            } else {
                failed.push((item.clone(), why));
            }
        }
        for (j, item) in renders.iter().enumerate() {
            if item.kind != kind || used[j] {
                continue;
            }
            extra += item.area;
            let why = if kind == ItemKind::Paint {
                paint_mismatch(item, &original_paint, false)
            } else {
                "unpaired".to_owned()
            };
            unpaired.push((item.clone(), why));
        }
        let score = if total + extra > 0.0 {
            js_round(held / (total + extra) * 10_000.0) / 10_000.0
        } else {
            1.0
        };
        match kind {
            ItemKind::Text => point.text = score,
            ItemKind::Paint => point.paint = score,
        }
    }
    PointComparison {
        point,
        failed,
        unpaired,
    }
}

// Why a paired item does not hold, or empty where it does.
fn holding(original: &Item, render: &Item, frame_original: Rect, frame_render: Rect) -> String {
    let tolerance_x = (0.05 * frame_original[2]).max(1.0);
    let tolerance_y = (0.05 * frame_original[3]).max(1.0);
    if original.boxes.len() != render.boxes.len() {
        return format!("lines {}->{}", original.boxes.len(), render.boxes.len());
    }
    for (a, b) in original.boxes.iter().zip(&render.boxes) {
        if !near(a[2], b[2], (0.1 * a[2]).max(1.0)) {
            return format!("w {}->{}", fixed(a[2]), fixed(b[2]));
        }
        if !near(a[3], b[3], (0.1 * a[3]).max(1.0)) {
            return format!("h {}->{}", fixed(a[3]), fixed(b[3]));
        }
        let (ox, rx) = (a[0] - frame_original[0], b[0] - frame_render[0]);
        if !near(ox, rx, tolerance_x) {
            return format!("x {}->{}", fixed(ox), fixed(rx));
        }
        let (oy, ry) = (a[1] - frame_original[1], b[1] - frame_render[1]);
        if !near(oy, ry, tolerance_y) {
            return format!("y {}->{}", fixed(oy), fixed(ry));
        }
    }
    String::new()
}

fn indices(items: &[Item], kind: ItemKind) -> Vec<usize> {
    items.iter().enumerate().filter(|(_, item)| item.kind == kind).map(|(i, _)| i).collect()
}

// Pairs `a[i]` with `b[j]` in order where signatures are equal: a longest common subsequence.
#[expect(clippy::many_single_char_names, reason = "the textbook names of the table")]
fn pair(a: &[&Item], b: &[&Item]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let stride = m + 1;
    let mut lengths = vec![0_u32; (n + 1) * stride];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lengths[i * stride + j] = if a[i].signature == b[j].signature {
                lengths[(i + 1) * stride + j + 1] + 1
            } else {
                lengths[(i + 1) * stride + j].max(lengths[i * stride + j + 1])
            };
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i].signature == b[j].signature {
            out.push((i, j));
            i += 1;
            j += 1;
        } else if lengths[(i + 1) * stride + j] >= lengths[i * stride + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

fn contains(outer: Rect, inner: Rect) -> bool {
    let t = 1.0;
    outer[0] - t <= inner[0]
        && outer[1] - t <= inner[1]
        && inner[0] + inner[2] <= outer[0] + outer[2] + t
        && inner[1] + inner[3] <= outer[1] + outer[3] + t
}

fn near(x: f64, y: f64, tolerance: f64) -> bool {
    (x - y).abs() <= tolerance
}

// `toFixed(0)`: halves away from zero.
fn fixed(value: f64) -> String {
    if value < 0.0 {
        format!("-{}", (-value + 0.5).floor())
    } else {
        format!("{}", (value + 0.5).floor())
    }
}

// Why an original text found no partner: the first field that differs from
// a render text with the same words.
fn text_mismatch(original: &Item, renders: &[Item]) -> String {
    let fields: Vec<&str> = original.signature.split(TEXT_SEPARATOR).collect();
    let Some(render) = renders.iter().find(|r| {
        r.kind == ItemKind::Text && r.signature.split(TEXT_SEPARATOR).next() == Some(fields[0])
    }) else {
        return "unpaired".to_owned();
    };
    let theirs: Vec<&str> = render.signature.split(TEXT_SEPARATOR).collect();
    fields.iter().enumerate().find(|(k, v)| theirs.get(*k) != Some(v)).map_or_else(
        || "unpaired".to_owned(),
        |(k, v)| format!("{} {v}->{}", TEXT_FIELDS[k], theirs.get(k).unwrap_or(&"undefined")),
    )
}

fn overlap_ratio(a: Rect, b: Rect) -> f64 {
    let width = ((a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0])).max(0.0);
    let height = ((a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1])).max(0.0);
    let intersection = width * height;
    #[expect(clippy::suboptimal_flops, reason = "the scores are the unfused results")]
    let union = a[2] * a[3] + b[2] * b[3] - intersection;
    if intersection > 0.0 { intersection / union } else { 0.0 }
}

// Why a painted item found no partner, as original->render: its first paint
// value that differs from the candidate overlapping it most.
fn paint_mismatch(item: &Item, candidates: &[&Item], is_original: bool) -> String {
    let mut closest: Option<&Item> = None;
    let mut ratio = 0.0;
    for candidate in candidates {
        let r = overlap_ratio(item.bounds, candidate.bounds);
        if r > ratio {
            ratio = r;
            closest = Some(candidate);
        }
    }
    let Some(closest) = closest else {
        return "nothing painted there".to_owned();
    };
    let (original, render) = if is_original {
        (&item.signature, &closest.signature)
    } else {
        (&closest.signature, &item.signature)
    };
    let original: Vec<&str> = original.split('|').collect();
    let render: Vec<&str> = render.split('|').collect();
    let boxes = original[0] == "box" && render[0] == "box";
    let differing = if boxes {
        original.iter().enumerate().position(|(k, v)| render.get(k) != Some(v))
    } else if item.signature == closest.signature {
        None
    } else {
        Some(0)
    };
    differing.map_or_else(
        || "same paint as the box there (an extra layer, or out of visual order)".to_owned(),
        |k| {
            format!(
                "{} {}->{}",
                PAINT_FIELDS[k],
                original[k],
                render.get(k).unwrap_or(&"undefined")
            )
        },
    )
}

// The failing and unpaired items over a comparison, keyed by id in first-seen order.
#[derive(Default)]
struct Tally {
    failed: Notes,
    unpaired: Notes,
}

impl Tally {
    fn point(&mut self, original: &Facts, render: &Facts, viewport: Viewport) -> Point {
        let at = viewport.label();
        let found = compare_at(original, render, viewport);
        for (item, why) in &found.failed {
            self.failed.note(item, &at, why);
        }
        for (item, why) in &found.unpaired {
            self.unpaired.note(item, &at, why);
        }
        found.point
    }
}

#[derive(Default)]
struct Notes {
    order: Vec<Failure>,
    at: BTreeMap<String, usize>,
}

impl Notes {
    fn note(&mut self, item: &Item, at: &str, why: &str) {
        let index = *self.at.entry(item.id.clone()).or_insert_with(|| {
            self.order.push(Failure {
                id: item.id.clone(),
                kind: item.kind.name(),
                label: item.label.clone(),
                points: 0,
                first: at.to_owned(),
                last: at.to_owned(),
                why: why.to_owned(),
                area: 0.0,
            });
            self.order.len() - 1
        });
        let failure = &mut self.order[index];
        failure.points += 1;
        at.clone_into(&mut failure.last);
        failure.area += item.area;
    }

    fn worst(mut self) -> Vec<Failure> {
        self.order.sort_by(|a, b| b.area.total_cmp(&a.area));
        self.order.truncate(WORST);
        for failure in &mut self.order {
            failure.area = js_round(failure.area);
        }
        self.order
    }
}

#[expect(clippy::cast_precision_loss, reason = "a few thousand viewports")]
fn summarise(rows: &[(String, Point)]) -> Option<Summary> {
    if rows.is_empty() {
        return None;
    }
    Some(
        ItemKind::ALL
            .iter()
            .map(|&kind| {
                let scores: Vec<f64> = rows.iter().map(|(_, point)| point.score(kind)).collect();
                let min = scores.iter().copied().fold(f64::INFINITY, f64::min);
                #[expect(clippy::float_cmp, reason = "the minimum is one of the scores")]
                let at = scores.iter().position(|&s| s == min).unwrap_or(0);
                let mean = scores.iter().sum::<f64>() / scores.len() as f64;
                (
                    kind.name(),
                    KindSummary {
                        min,
                        min_at: rows[at].0.clone(),
                        mean: js_round(mean * 10_000.0) / 10_000.0,
                    },
                )
            })
            .collect(),
    )
}
