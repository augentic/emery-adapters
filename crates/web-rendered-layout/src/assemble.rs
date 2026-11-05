//! Builds facts from one raw browser capture (`capture.mjs`).
//!
//! A capture holds `meta.json`, a sample per searched width under `w/`, a
//! sample per searched height under `h/<width>/`, accessibility trees under
//! `ax/`, and assets under `assets/`. Nodes are the union of elements shown
//! at any sampled viewport, their ancestors, and `<br>`. Along the width axis
//! each node has box pieces (one line `a*W+b` per run of samples it holds
//! on), text line boxes with break offsets, computed styles, sizing values,
//! attributes, and pseudo-elements as value runs; under `h.<width>` the same
//! along the height axis, only where it changes. The result is split into
//! `index.json` and `parts/<id>.json`. Deterministic: no clock, network, or
//! randomness.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde_json::{Map, Value, json};

use crate::{Error, FactsFolder};

/// The largest part, in bytes of its nodes as JSON, before it is split into its children.
pub const MAX_PART_BYTES: usize = 200_000;

// What one sample holds: its coordinate on the searched axis, the flat
// boxes (x, y, w, h, shown per element), the text lines, and the fact deltas.
struct Sample {
    at: f64,
    boxes: Vec<f64>,
    lines: Vec<Value>,
    deltas: Value,
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

fn round3(x: f64) -> f64 {
    (x * 1e3).round() / 1e3
}

#[expect(clippy::suboptimal_flops, reason = "the facts' numbers are the unfused results")]
fn on_line(a: f64, b: f64, x: f64) -> f64 {
    a * x + b
}

// The line through (x0, v0) and (x1, v1); constant where the coordinates coincide.
#[expect(clippy::suboptimal_flops, reason = "the facts' numbers are the unfused results")]
fn line_through(x0: f64, v0: f64, x1: f64, v1: f64) -> (f64, f64) {
    if (x1 - x0).abs() < 1e-9 {
        (0.0, v0)
    } else {
        let a = (v1 - v0) / (x1 - x0);
        (a, v0 - a * x0)
    }
}

// Values by key along one axis, recorded where they change.
#[derive(Default)]
struct Runs(BTreeMap<String, Vec<(f64, Value)>>);

impl Runs {
    fn set(&mut self, at: f64, key: &str, value: &Value) {
        let runs = self.0.entry(key.to_owned()).or_default();
        match runs.last_mut() {
            Some(last) if (last.0 - at).abs() < 1e-9 => last.1 = value.clone(),
            Some(last) if &last.1 == value => {}
            _ => runs.push((at, value.clone())),
        }
    }

    fn apply(&mut self, at: f64, delta: &Value) {
        for (key, value) in delta.as_object().into_iter().flatten() {
            self.set(at, key, value);
        }
    }

    fn at(&self, at: f64, key: &str) -> Option<&Value> {
        self.0.get(key)?.iter().rev().find(|run| run.0 <= at).map(|run| &run.1)
    }

    // Over the sample coordinates `xs`: `{key: value}` where one value covers
    // the axis; `{key: {"px": [[from, to, [[a, b], ...]], ...]}}` where every
    // value is a list of px numbers and fewer lines (within 0.5px at every
    // sample) than runs describe it; else `{key: [[from, to, value], ...]}`.
    // Keys whose one value `skip` accepts are left out.
    fn json(&self, xs: &[f64], skip: impl Fn(&str, &Value) -> bool) -> Map<String, Value> {
        let (lo, hi) = (xs[0], xs[xs.len() - 1]);
        let mut out = Map::new();
        for (key, runs) in &self.0 {
            if runs.len() == 1 && skip(key, &runs[0].1) {
                continue;
            }
            let fitted = (runs.len() > 2).then(|| self.fit_px(key, runs[0].0, xs)).flatten();
            let value = if runs.len() == 1 && runs[0].0 <= lo {
                runs[0].1.clone()
            } else if let Some(pieces) = fitted.filter(|pieces| pieces.len() < runs.len()) {
                json!({ "px": pieces })
            } else {
                json!(
                    runs.iter()
                        .enumerate()
                        .map(|(m, (from, value))| {
                            json!([from, runs.get(m + 1).map_or(hi, |next| next.0), value])
                        })
                        .collect::<Vec<_>>()
                )
            };
            out.insert(key.clone(), value);
        }
        out
    }

    // Piecewise lines through the px numbers of `key` at every sample from
    // `from` on, merged while one line per number holds every sample
    // between; `None` unless every value is the same count of px numbers.
    fn fit_px(&self, key: &str, from: f64, xs: &[f64]) -> Option<Vec<Value>> {
        let xs: Vec<f64> = xs.iter().copied().filter(|&x| x >= from).collect();
        let numbers = |value: &Value| -> Option<Vec<f64>> {
            value
                .as_str()?
                .split_whitespace()
                .map(|token| token.strip_suffix("px")?.parse::<f64>().ok())
                .collect()
        };
        let values: Vec<Vec<f64>> =
            xs.iter().map(|&x| numbers(self.at(x, key)?)).collect::<Option<_>>()?;
        let count = values[0].len();
        if count == 0 || values.iter().any(|v| v.len() != count) || xs.len() < 2 {
            return None;
        }
        let mut out = Vec::new();
        let mut start = 0;
        while start < xs.len() - 1 {
            let mut end = start + 1;
            while end + 1 < xs.len()
                && (start + 1..=end).all(|m| {
                    (0..count).all(|j| {
                        let (a, b) = line_through(
                            xs[start],
                            values[start][j],
                            xs[end + 1],
                            values[end + 1][j],
                        );
                        (on_line(a, b, xs[m]) - values[m][j]).abs() <= 0.5
                    })
                })
            {
                end += 1;
            }
            let lines: Vec<Value> = (0..count)
                .map(|j| {
                    let (a, b) = line_through(xs[start], values[start][j], xs[end], values[end][j]);
                    json!([round6(a), round3(b)])
                })
                .collect();
            out.push(json!([xs[start], xs[end], lines]));
            start = end;
        }
        Some(out)
    }
}

// Box pieces: consecutive samples merged while one line holds every sample between.
#[expect(clippy::many_single_char_names, reason = "line fitting: a*x+b at samples m, k")]
fn box_pieces(samples: &[Sample], rect: &dyn Fn(&Sample) -> Option<[f64; 4]>) -> Vec<Value> {
    let n = samples.len();
    let mut out = Vec::new();
    let mut start = 0;
    while start < n - 1 {
        let first = rect(&samples[start]);
        let mut end = start + 1;
        let mut last = start + 1;
        loop {
            let held = match (first, rect(&samples[end])) {
                (None, None) => true,
                (Some(a0), Some(a1)) => (start + 1..end).all(|m| {
                    rect(&samples[m]).is_some_and(|between| {
                        (0..4).all(|k| {
                            let (a, b) =
                                line_through(samples[start].at, a0[k], samples[end].at, a1[k]);
                            (on_line(a, b, samples[m].at) - between[k]).abs() <= 0.5
                        })
                    })
                }),
                _ => false,
            };
            if !held {
                break;
            }
            last = end;
            if end == n - 1 {
                break;
            }
            end += 1;
        }
        // `last` is the last sample on the line; where the next one already
        // breaks it, a constant piece [start, start+1)
        let (hi, a, b, shown) = match (first, rect(&samples[last])) {
            (Some(p), Some(q)) if last > start && rect(&samples[start + 1]).is_some() => {
                let mut a = [0.0; 4];
                let mut b = [0.0; 4];
                for k in 0..4 {
                    let (slope, intercept) =
                        line_through(samples[start].at, p[k], samples[last].at, q[k]);
                    a[k] = round6(slope);
                    b[k] = round3(intercept);
                }
                (last, a, b, 1)
            }
            (Some(p), _) => {
                (start + 1, [0.0; 4], [round3(p[0]), round3(p[1]), round3(p[2]), round3(p[3])], 1)
            }
            (None, _) => (last.max(start + 1), [0.0; 4], [0.0; 4], 0),
        };
        let hi =
            if rect(&samples[start + 1]).is_none() && first.is_some() { start + 1 } else { hi };
        out.push(json!([
            samples[start].at,
            samples[hi].at,
            a[0],
            a[1],
            a[2],
            a[3],
            b[0],
            b[1],
            b[2],
            b[3],
            shown
        ]));
        start = hi;
    }
    out
}

fn text_lines(text: usize, sample: &Sample) -> Option<Vec<Value>> {
    sample.lines.get(text).and_then(Value::as_array).cloned()
}

// Line pieces of text `text`: one piece while the line count and breaks
// stay and every rect stays on one line.
#[expect(clippy::cast_possible_truncation, reason = "a line count")]
#[expect(clippy::many_single_char_names, reason = "line fitting: p*x+q at samples m, k")]
fn line_pieces(samples: &[Sample], text: usize) -> Vec<Value> {
    let n = samples.len();
    let at = |m: usize| text_lines(text, &samples[m]);
    let mut pieces = Vec::new();
    let mut start = 0;
    while start < n - 1 {
        let first = at(start);
        let mut last = start;
        for end in start + 1..n {
            let same = match (&first, &at(end)) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    let count = a[0].as_u64().unwrap_or(0) as usize;
                    a[0] == b[0]
                        && a[1 + 4 * count..] == b[1 + 4 * count..]
                        && (start + 1..end).all(|m| {
                            at(m).is_some_and(|between| {
                                between[0] == a[0]
                                    && (1..=4 * count).all(|k| {
                                        let f = |v: &Value| v.as_f64().unwrap_or(0.0);
                                        let (p, q) = line_through(
                                            samples[start].at,
                                            f(&a[k]),
                                            samples[end].at,
                                            f(&b[k]),
                                        );
                                        (on_line(p, q, samples[m].at) - f(&between[k])).abs() <= 0.5
                                    })
                            })
                        })
                }
                _ => false,
            };
            if !same {
                break;
            }
            last = end;
        }
        let hi = last.max(start + 1);
        let Some(a) = &first else {
            pieces.push(json!({"lo": samples[start].at, "hi": samples[hi].at, "lines": 0}));
            start = hi;
            continue;
        };
        let count = a[0].as_u64().unwrap_or(0) as usize;
        let far = if last > start { at(last) } else { None };
        let rects: Vec<Value> = (0..count)
            .map(|line| {
                let v =
                    |held: &Vec<Value>, k: usize| held[1 + 4 * line + k].as_f64().unwrap_or(0.0);
                let fitted: Vec<Value> = (0..4)
                    .map(|k| {
                        let (p, q) = far.as_ref().map_or_else(
                            || (0.0, v(a, k)),
                            |far| {
                                line_through(
                                    samples[start].at,
                                    v(a, k),
                                    samples[last].at,
                                    v(far, k),
                                )
                            },
                        );
                        json!([round6(p), round3(q)])
                    })
                    .collect();
                json!(fitted)
            })
            .collect();
        pieces.push(json!({"lo": samples[start].at, "hi": samples[hi].at, "lines": count, "breaks": a[1 + 4 * count..].to_vec(), "rects": rects}));
        start = hi;
    }
    pieces
}

// Leaves out a height run whose one value is the width-axis value at the sweep's width.
fn unchanged(base: &Runs, at: f64) -> impl Fn(&str, &Value) -> bool + '_ {
    move |key, value| base.at(at, key) == Some(value)
}

fn box_constant(pieces: &[Value]) -> bool {
    pieces.len() == 1 && (2..6).all(|k| pieces[0][k].as_f64() == Some(0.0))
}

fn lines_constant(pieces: &[Value]) -> bool {
    pieces.len() == 1
        && pieces[0]["rects"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|rect| rect.as_array().into_iter().flatten())
            .all(|line| line[0].as_f64() == Some(0.0))
}

// The font token of an element, from a lookup of its computed style by property.
type FontOf<'a> = dyn Fn(&dyn Fn(&str) -> String) -> Value + 'a;

// Fact runs along one axis: per element styles, sizing values, attributes,
// and font token; per pseudo-element styles; the accessibility tree file.
#[derive(Default)]
struct AxisFacts {
    style: Vec<Runs>,
    sizing: Vec<Runs>,
    attrs: Vec<Runs>,
    pseudo: BTreeMap<String, Runs>,
    font: Vec<Runs>,
    ax: Runs,
}

impl AxisFacts {
    fn new(elements: usize) -> Self {
        let runs = || (0..elements).map(|_| Runs::default()).collect::<Vec<_>>();
        Self {
            style: runs(),
            sizing: runs(),
            attrs: runs(),
            font: runs(),
            ..Default::default()
        }
    }

    // Applies each sample's deltas in order; `seed` gives a key's value
    // before the first sample, the width facts at the sweep's width.
    fn apply(&mut self, samples: &[Sample], seed: Option<(&Self, f64)>, font_of: &FontOf<'_>) {
        let each = |delta: &Value, f: &mut dyn FnMut(usize, &Value)| {
            for (key, value) in delta.as_object().into_iter().flatten() {
                if let Ok(index) = key.parse::<usize>() {
                    f(index, value);
                }
            }
        };
        for sample in samples {
            let at = sample.at;
            let seeded = |runs: &mut Runs, base: Option<&Runs>, delta: &Value| {
                if let (Some((_, from)), Some(base)) = (seed, base) {
                    for key in delta.as_object().into_iter().flatten().map(|pair| pair.0) {
                        if !runs.0.contains_key(key)
                            && let Some(value) = base.at(from, key)
                        {
                            runs.set(samples[0].at, key, &value.clone());
                        }
                    }
                }
                runs.apply(at, delta);
            };
            each(&sample.deltas["s"], &mut |i, v| {
                seeded(&mut self.style[i], seed.map(|s| &s.0.style[i]), v);
            });
            each(&sample.deltas["m"], &mut |i, v| {
                seeded(&mut self.sizing[i], seed.map(|s| &s.0.sizing[i]), v);
            });
            each(&sample.deltas["a"], &mut |i, v| {
                seeded(&mut self.attrs[i], seed.map(|s| &s.0.attrs[i]), v);
            });
            for (key, pseudo) in sample.deltas["ps"].as_object().into_iter().flatten() {
                let base = seed.and_then(|s| s.0.pseudo.get(key));
                let runs = self.pseudo.entry(key.clone()).or_default();
                seeded(runs, base, &pseudo["s"]);
            }
            for i in 0..self.font.len() {
                let style = &self.style[i];
                let lookup = |property: &str| {
                    style
                        .at(at, property)
                        .or_else(|| seed.and_then(|(s, from)| s.style[i].at(from, property)))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_owned()
                };
                let token = font_of(&lookup);
                self.font[i].set(at, "font", &token);
            }
            self.ax.set(at, "ax", &sample.deltas["ax"]);
        }
    }
}

fn read_json(path: &Path) -> Result<Value, Error> {
    let text = fs::read_to_string(path)
        .map_err(|err| Error::new(format!("reading {}: {err}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|err| Error::new(format!("{} is not JSON: {err}", path.display())))
}

fn load(path: &Path, at: f64) -> Result<Sample, Error> {
    let mut deltas = read_json(path)?;
    let boxes = deltas["b"]
        .as_array()
        .map(|flat| flat.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect())
        .unwrap_or_default();
    let lines = std::mem::take(
        deltas["l"]
            .as_array_mut()
            .ok_or_else(|| Error::new(format!("{} has no `l`", path.display())))?,
    );
    Ok(Sample {
        at,
        boxes,
        lines,
        deltas,
    })
}

fn numbers(value: &Value) -> Vec<f64> {
    value.as_array().into_iter().flatten().filter_map(Value::as_f64).collect()
}

/// Builds the facts of the capture in `dir`, split into parts of at most [`MAX_PART_BYTES`].
///
/// # Errors
///
/// When `meta.json` or a sample it names is missing or not JSON, or the
/// widths hold fewer than two samples.
#[expect(clippy::too_many_lines, reason = "one pass over the capture, ported as one")]
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "element indices")]
pub fn assemble(dir: &Path) -> Result<FactsFolder, Error> {
    let meta = read_json(&dir.join("meta.json"))?;
    let elements =
        meta["elements"].as_array().ok_or_else(|| Error::new("`meta.json` lists no `elements`"))?;
    let texts =
        meta["texts"].as_array().ok_or_else(|| Error::new("`meta.json` lists no `texts`"))?;
    let count = elements.len();
    let range =
        [meta["range"][0].as_f64().unwrap_or(320.0), meta["range"][1].as_f64().unwrap_or(1920.0)];
    let height_range = [
        meta["heights"]["range"][0].as_f64().unwrap_or(0.0),
        meta["heights"]["range"][1].as_f64().unwrap_or(0.0),
    ];

    let mut samples = Vec::new();
    for width in numbers(meta.get("w_files").unwrap_or_else(|| &meta["ends"])) {
        samples.push(load(&dir.join("w").join(format!("{width}.json")), width)?);
    }
    if samples.len() < 2 {
        return Err(Error::new(format!("{} holds fewer than two width samples", dir.display())));
    }
    let mut sweeps: Vec<(f64, Vec<Sample>)> = Vec::new();
    for (width, sweep) in meta["heights"]["sweeps"].as_object().into_iter().flatten() {
        let mut held = Vec::new();
        for height in numbers(&sweep["ends"]) {
            held.push(load(&dir.join("h").join(width).join(format!("{height}.json")), height)?);
        }
        if held.len() >= 2 {
            let width: f64 = width
                .parse()
                .map_err(|err| Error::new(format!("`meta.json` sweeps width `{width}`: {err}")))?;
            sweeps.push((width, held));
        }
    }
    sweeps.sort_by(|p, q| p.0.total_cmp(&q.0));
    let xs: Vec<f64> = samples.iter().map(|s| s.at).collect();

    let shown = |s: &Sample, i: usize| s.boxes[i * 5 + 4] > 0.5;
    let rect = |s: &Sample, i: usize| {
        shown(s, i)
            .then(|| [s.boxes[i * 5], s.boxes[i * 5 + 1], s.boxes[i * 5 + 2], s.boxes[i * 5 + 3]])
    };
    let parent = |i: usize| elements[i]["parent"].as_i64().unwrap_or(-1);
    let kind = |i: usize| elements[i]["kind"].as_str().unwrap_or("box");
    let every_sample = || samples.iter().chain(sweeps.iter().flat_map(|s| s.1.iter()));

    // the union node set
    let shown_anywhere: Vec<bool> =
        (0..count).map(|i| every_sample().any(|s| shown(s, i))).collect();
    let mut include = shown_anywhere.clone();
    for i in 0..count {
        if kind(i) == "br" && parent(i) >= 0 && shown_anywhere[parent(i) as usize] {
            include[i] = true;
        }
    }
    for i in (0..count).rev() {
        if include[i] && parent(i) >= 0 {
            include[parent(i) as usize] = true;
        }
    }
    let text_included: Vec<bool> = (0..texts.len())
        .map(|j| {
            let p = texts[j]["parent"].as_u64().unwrap_or(0) as usize;
            include[p] && every_sample().any(|s| text_lines(j, s).is_some())
        })
        .collect();

    let fonts = meta["fonts"].as_object().cloned().unwrap_or_default();
    let font_of = |lookup: &dyn Fn(&str) -> String| -> Value {
        let key =
            format!("{}|{}|{}", lookup("font-family"), lookup("font-weight"), lookup("font-style"));
        fonts
            .iter()
            .find(|(_, font)| font["key"].as_str() == Some(key.as_str()))
            .map_or(Value::Null, |(token, _)| json!(token))
    };
    let mut widthwise = AxisFacts::new(count);
    widthwise.apply(&samples, None, &font_of);
    let heightwise: Vec<AxisFacts> = sweeps
        .iter()
        .map(|(width, held)| {
            let mut facts = AxisFacts::new(count);
            facts.apply(held, Some((&widthwise, *width)), &font_of);
            facts
        })
        .collect();

    // defaults: per property, the most common value at the widest sample over the included elements
    let widest = samples[samples.len() - 1].at;
    let mut counts: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    for i in (0..count).filter(|&i| include[i]) {
        for key in widthwise.style[i].0.keys() {
            if let Some(value) = widthwise.style[i].at(widest, key) {
                *counts.entry(key).or_default().entry(value.to_string()).or_default() += 1;
            }
        }
    }
    let defaults: Map<String, Value> = counts
        .into_iter()
        .filter_map(|(key, tally)| {
            let (value, _) = tally.into_iter().max_by(|p, q| p.1.cmp(&q.1).then(q.0.cmp(&p.0)))?;
            Some((key.to_owned(), serde_json::from_str(&value).ok()?))
        })
        .collect();
    let is_default = |key: &str, value: &Value| defaults.get(key) == Some(value);

    let assets: Map<String, Value> = meta["assets"]
        .as_object()
        .map(|held| held.iter().map(|(url, asset)| (url.clone(), asset["file"].clone())).collect())
        .unwrap_or_default();
    let element_id = |i: usize| format!("n{i}");
    let text_id = |j: usize| format!("t{j}");
    let put = |node: &mut Value, key: &str, map: Map<String, Value>| {
        if !map.is_empty() {
            node[key] = Value::Object(map);
        }
    };
    let mut nodes = Map::new();
    for i in (0..count).filter(|&i| include[i]) {
        let mut children: Vec<Value> = Vec::new();
        for child in elements[i]["seq"].as_array().into_iter().flatten() {
            if let Some(k) = child.as_u64() {
                if include[k as usize] {
                    children.push(json!(element_id(k as usize)));
                }
            } else if let Some(s) = child.as_str() {
                if s == " " {
                    children.push(json!(" "));
                } else if let Some(j) = s.strip_prefix('t').and_then(|x| x.parse::<usize>().ok())
                    && text_included[j]
                {
                    children.push(json!(text_id(j)));
                }
            }
        }
        let mut node = json!({
            "path": elements[i]["p"],
            "tag": elements[i]["tag"],
            "kind": kind(i),
            "parent": if parent(i) >= 0 { json!(element_id(parent(i) as usize)) } else { Value::Null },
            "children": children,
            "box": box_pieces(&samples, &|s| rect(s, i)),
        });
        put(&mut node, "style", widthwise.style[i].json(&xs, is_default));
        put(&mut node, "sizing", widthwise.sizing[i].json(&xs, |_, _| false));
        put(&mut node, "attrs", widthwise.attrs[i].json(&xs, |_, _| false));
        node["font"] =
            widthwise.font[i].json(&xs, |_, _| false).remove("font").unwrap_or(Value::Null);
        let mut pseudo = Map::new();
        for (key, runs) in &widthwise.pseudo {
            let Some(name) = key.strip_prefix(&format!("{i}::")) else {
                continue;
            };
            let pseudo_rect = |s: &Sample| -> Option<[f64; 4]> {
                let b = s.deltas["ps"][key.as_str()]["b"].as_array()?;
                Some([0, 1, 2, 3].map(|k| b.get(k).and_then(Value::as_f64).unwrap_or(0.0)))
            };
            let mut held = json!({"box": box_pieces(&samples, &pseudo_rect)});
            put(&mut held, "style", runs.json(&xs, is_default));
            pseudo.insert(name.to_owned(), held);
        }
        put(&mut node, "pseudo", pseudo);
        let mut heights = Map::new();
        for ((width, held), facts) in sweeps.iter().zip(&heightwise) {
            let hs: Vec<f64> = held.iter().map(|s| s.at).collect();
            let mut at = json!({});
            let pieces = box_pieces(held, &|s| rect(s, i));
            if !box_constant(&pieces) {
                at["box"] = json!(pieces);
            }
            put(&mut at, "style", facts.style[i].json(&hs, unchanged(&widthwise.style[i], *width)));
            put(
                &mut at,
                "sizing",
                facts.sizing[i].json(&hs, unchanged(&widthwise.sizing[i], *width)),
            );
            put(&mut at, "attrs", facts.attrs[i].json(&hs, unchanged(&widthwise.attrs[i], *width)));
            put(&mut at, "font", facts.font[i].json(&hs, unchanged(&widthwise.font[i], *width)));
            if at.as_object().is_some_and(|held| !held.is_empty()) {
                heights.insert(width.to_string(), at);
            }
        }
        put(&mut node, "h", heights);
        if let Some(extra) = meta["extras"].get(i.to_string()) {
            let mut extra = extra.clone();
            if let Some(src) = extra.get("src").and_then(Value::as_str) {
                extra["file"] = assets.get(src).cloned().unwrap_or(Value::Null);
            }
            node["extra"] = extra;
        }
        nodes.insert(element_id(i), node);
    }
    for (j, text) in texts.iter().enumerate() {
        if !text_included[j] {
            continue;
        }
        let p = text["parent"].as_u64().unwrap_or(0) as usize;
        let mut node = json!({
            "path": text["p"],
            "tag": "#text",
            "kind": "text",
            "parent": element_id(p),
            "children": [],
            "text": text["text"],
            "lines": line_pieces(&samples, j),
        });
        let mut heights = Map::new();
        for (width, held) in &sweeps {
            let pieces = line_pieces(held, j);
            if !lines_constant(&pieces) {
                heights.insert(width.to_string(), json!({"lines": pieces}));
            }
        }
        put(&mut node, "h", heights);
        nodes.insert(text_id(j), node);
    }

    let mut by_kind = BTreeMap::<String, (usize, usize)>::new();
    for check in meta["checks"].as_array().into_iter().flatten() {
        let tally = by_kind.entry(check["kind"].as_str().unwrap_or("?").to_owned()).or_default();
        tally.0 += 1;
        if check["ok"] == false {
            tally.1 += 1;
        }
    }
    let ax_files: BTreeSet<String> = std::iter::once(&widthwise.ax)
        .chain(heightwise.iter().map(|f| &f.ax))
        .flat_map(|runs| {
            runs.0.values().flatten().filter_map(|run| run.1.as_str().map(str::to_owned))
        })
        .collect();
    let mut ax_h = Map::new();
    for ((width, held), facts) in sweeps.iter().zip(&heightwise) {
        let hs: Vec<f64> = held.iter().map(|s| s.at).collect();
        if let Some(value) = facts.ax.json(&hs, unchanged(&widthwise.ax, *width)).get("ax") {
            ax_h.insert(width.to_string(), value.clone());
        }
    }
    let search = json!({
        "url": meta["url"],
        "tolerance": meta["tolerance"],
        "seed": meta["seed"],
        "captures": meta["captures"],
        "capped": meta["capped"],
        "segments": meta["segments"],
        "checks": by_kind.iter().map(|(k, (t, f))| (k.clone(), json!({"total": t, "failed": f}))).collect::<Map<_, _>>(),
        "height_sweeps": meta["heights"]["sweeps"].as_object().map(|s| s.iter().map(|(w, v)| (w.clone(), json!({"captures": v["captures"], "capped": v["capped"], "ends": v["ends"]}))).collect::<Map<_, _>>()),
        "completeness": "probabilistic: a segment is accepted when its midpoint, 1/3, 2/3 and one seeded random point lie on the line through its ends; a kink that bends away and back between probes is missed. Pseudo-element boxes are known only at segment ends.",
    });
    let widest_sample = &samples[samples.len() - 1];
    let document = json!({
        "version": 2,
        "range": range,
        "height": meta["height"],
        "height_range": height_range,
        "search": search,
        "defaults": defaults,
        "faces": meta["faces"],
        "fonts": fonts,
        "assets": assets,
        "ax": widthwise.ax.json(&xs, |_, _| false).remove("ax").unwrap_or(Value::Null),
        "ax_h": ax_h,
        "ax_files": ax_files,
        "root": element_id(0),
        "nodes": nodes,
        "counts": {
            "elements": include.iter().filter(|&&x| x).count(),
            "elements_hidden_at_widest": (0..count).filter(|&i| include[i] && !shown(widest_sample, i)).count(),
            "texts": text_included.iter().filter(|&&x| x).count(),
        },
    });

    let mut folder = FactsFolder::default();
    let (index, parts) = split(&document, MAX_PART_BYTES);
    folder.insert("index.json", line(&index))?;
    for (id, part) in &parts {
        folder.insert(&format!("parts/{id}.json"), line(part))?;
    }
    for sub in ["assets", "ax"] {
        let Ok(entries) = fs::read_dir(dir.join(sub)) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let body = fs::read(entry.path())
                .map_err(|err| Error::new(format!("reading {}: {err}", entry.path().display())))?;
            folder.insert(&format!("{sub}/{name}"), body)?;
        }
    }
    Ok(folder)
}

fn line(value: &Value) -> Vec<u8> {
    let mut bytes = value.to_string().into_bytes();
    bytes.push(b'\n');
    bytes
}

fn subtree<'a>(nodes: &'a Map<String, Value>, id: &str, out: &mut Vec<&'a str>) {
    let Some((key, node)) = nodes.get_key_value(id) else {
        return;
    };
    out.push(key);
    for child in node["children"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        subtree(nodes, child, out);
    }
}

// `(index, parts)`: from the root, a node whose subtree fits `max_bytes` is
// one part; a larger node is a part of its own (`container`, its tag, path,
// and children also in the index's `containers`) and its children split the same way.
fn split(document: &Value, max_bytes: usize) -> (Value, Vec<(String, Value)>) {
    let empty = Map::new();
    let nodes = document["nodes"].as_object().unwrap_or(&empty);
    let size = |id: &str| -> usize {
        let mut ids = Vec::new();
        subtree(nodes, id, &mut ids);
        ids.iter().map(|key| nodes[*key].to_string().len()).sum()
    };
    let mut containers = Map::new();
    let mut parts = Vec::new();
    let mut listed = Vec::new();
    let mut stack = vec![document["root"].as_str().unwrap_or("n0").to_owned()];
    while let Some(id) = stack.pop() {
        let Some(node) = nodes.get(&id) else { continue };
        let bytes = size(&id);
        let children: Vec<String> = node["children"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|child| {
                child.as_str().filter(|child| nodes.contains_key(*child)).map(str::to_owned)
            })
            .collect();
        if bytes <= max_bytes || children.is_empty() {
            let mut ids = Vec::new();
            subtree(nodes, &id, &mut ids);
            let part: Map<String, Value> =
                ids.iter().map(|key| ((*key).to_owned(), nodes[*key].clone())).collect();
            listed.push(json!({"id": id, "tag": node["tag"], "path": node["path"], "parent": node["parent"], "file": format!("parts/{id}.json"), "nodes": ids.len(), "bytes": bytes}));
            parts.push((id, Value::Object(part)));
        } else {
            let own = node.to_string().len();
            containers.insert(id.clone(), json!({"tag": node["tag"], "path": node["path"], "parent": node["parent"], "children": node["children"], "file": format!("parts/{id}.json")}));
            listed.push(json!({"id": id, "tag": node["tag"], "path": node["path"], "parent": node["parent"], "file": format!("parts/{id}.json"), "nodes": 1, "bytes": own, "container": true}));
            parts.push((id.clone(), json!({ id.clone(): node })));
            stack.extend(children.into_iter().rev());
        }
    }
    let mut index = document.as_object().cloned().unwrap_or_default();
    index.remove("nodes");
    index.insert("containers".into(), Value::Object(containers));
    index.insert("parts".into(), Value::Array(listed));
    (Value::Object(index), parts)
}
