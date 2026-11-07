//! Measures a staged plan's parts, waves, merged stems, and dependency edges.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::build::strings;
use crate::grade::stem;

const BOOTSTRAP: &str = "start";

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlanShape {
    pub parts: usize,
    pub waves: usize,
    pub widest: usize,
    pub wave_shape: String,
    // Each part that merges two or more domain stems: `(part_name, stems)`.
    pub merged: Vec<(String, Vec<String>)>,
    pub type_backed: Vec<Edge>,
    pub unbacked: Vec<Edge>,
    pub unbacked_start: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    // The dependency's type names spelled in the dependent part's type
    // signatures; empty for an unbacked edge.
    pub types: Vec<String>,
}

struct PartNode {
    id: String,
    name: String,
    requirements: Vec<String>,
    types: Vec<String>,
    depends_on: Vec<String>,
}

impl PlanShape {
    pub fn of(
        plan: Option<&Value>, spec: Option<&Value>, design: Option<&Value>, waves: Option<&Value>,
    ) -> Self {
        let Some(entries) =
            plan.and_then(|plan| plan["parts"].as_array().or_else(|| plan["slices"].as_array()))
        else {
            return Self::default();
        };
        let parts: Vec<PartNode> = entries
            .iter()
            .map(|entry| PartNode {
                id: entry["id"].as_str().unwrap_or("").to_owned(),
                name: entry["name"].as_str().unwrap_or("").to_owned(),
                requirements: strings(&entry["requirements"]),
                types: strings(&entry["types"]),
                depends_on: strings(&entry["depends-on"]),
            })
            .collect();

        // waves and widest
        let widths = wave_widths(&parts, waves);
        let widest = widths.iter().copied().max().unwrap_or(0);
        let wave_shape = format_wave_shape(&widths);

        // requirement stems and triggers from `spec.json`
        let by_req: BTreeMap<&str, (&str, Option<&Value>)> = spec
            .and_then(|spec| spec["requirements"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|req| {
                let id = req["id"].as_str()?;
                let req_stem = req["subject"].as_str().and_then(stem)?;
                let trigger = req.get("trigger").filter(|trigger| !trigger.is_null());
                Some((id, (req_stem, trigger)))
            })
            .collect();

        let merged = merged_parts(&parts, &by_req);
        let (type_backed, unbacked, unbacked_start) = classify_edges(&parts, &by_req, design);

        Self {
            parts: parts.len(),
            waves: widths.len(),
            widest,
            wave_shape,
            merged,
            type_backed,
            unbacked,
            unbacked_start,
        }
    }

    // How many stems were folded into another stem's part across the plan.
    pub fn merged_stems(&self) -> usize {
        self.merged.iter().map(|(_, stems)| stems.len().saturating_sub(1)).sum()
    }

    pub fn merged_cell(&self) -> String {
        if self.merged.is_empty() {
            return "0".to_owned();
        }
        let detail: Vec<String> = self
            .merged
            .iter()
            .map(|(part, stems)| {
                let names: Vec<String> = stems.iter().map(|s| format!("`{s}`")).collect();
                format!("`{part}`: {}", names.join(" + "))
            })
            .collect();
        format!("{} ({})", self.merged_stems(), detail.join(", "))
    }

    pub fn type_backed_cell(&self) -> String {
        if self.type_backed.is_empty() {
            return "0".to_owned();
        }
        let detail: Vec<String> = self
            .type_backed
            .iter()
            .map(|edge| {
                let types: Vec<String> = edge.types.iter().map(|t| format!("`{t}`")).collect();
                format!("`{}` → `{}` ({})", edge.from, edge.to, types.join(", "))
            })
            .collect();
        format!("{} ({})", self.type_backed.len(), detail.join(", "))
    }

    pub fn unbacked_cell(&self) -> String {
        if self.unbacked.is_empty() {
            return "0".to_owned();
        }
        let detail: Vec<String> =
            self.unbacked.iter().map(|edge| format!("`{}` → `{}`", edge.from, edge.to)).collect();
        format!("{} ({})", self.unbacked.len(), detail.join(", "))
    }
}

impl PartNode {
    fn is_bootstrap(&self, by_req: &BTreeMap<&str, (&str, Option<&Value>)>) -> bool {
        self.name == BOOTSTRAP
            || (!self.requirements.is_empty()
                && self
                    .requirements
                    .iter()
                    .all(|id| by_req.get(id.as_str()).is_some_and(|&(s, _)| s == BOOTSTRAP)))
    }
}

// Formats a sequence of per-wave item counts as `N (w1/w2/...)`, or `0` when
// empty.
pub fn format_wave_shape(widths: &[usize]) -> String {
    if widths.is_empty() {
        return "0".to_owned();
    }
    let joined: Vec<String> = widths.iter().map(usize::to_string).collect();
    format!("{} ({})", widths.len(), joined.join("/"))
}

// The bare identifier a design type key declares: `Order` for `Order`,
// `orders.Order`, or `Order (src/orders.ts#L10-L20)`.
pub fn type_name(key: &str) -> &str {
    let dotted = key.split(" (").next().unwrap_or(key).trim();
    dotted.rsplit_once('.').map_or(dotted, |(_, name)| name)
}

fn wave_widths(parts: &[PartNode], waves: Option<&Value>) -> Vec<usize> {
    if let Some(waves) = waves.and_then(Value::as_array)
        && !waves.is_empty()
    {
        return waves.iter().map(|wave| wave.as_array().map_or(0, Vec::len)).collect();
    }
    let known: BTreeSet<&str> = parts.iter().map(|part| part.id.as_str()).collect();
    let mut pending: BTreeMap<&str, BTreeSet<&str>> = parts
        .iter()
        .map(|part| {
            let deps = part
                .depends_on
                .iter()
                .map(String::as_str)
                .filter(|&dep| dep != part.id && known.contains(dep))
                .collect();
            (part.id.as_str(), deps)
        })
        .collect();
    let mut widths = Vec::new();
    loop {
        let ready: Vec<&str> =
            pending.iter().filter(|(_, deps)| deps.is_empty()).map(|(&id, _)| id).collect();
        if ready.is_empty() {
            break;
        }
        for &id in &ready {
            pending.remove(id);
        }
        for deps in pending.values_mut() {
            for &id in &ready {
                deps.remove(id);
            }
        }
        widths.push(ready.len());
    }
    if !pending.is_empty() {
        widths.push(pending.len());
    }
    widths
}

// A `start` requirement pulled into another stem's part because it shares a
// route or message trigger with that stem's requirements is the trigger
// floor's placement, not a merged stem, when `start` has its own part in
// the plan.
fn merged_parts(
    parts: &[PartNode], by_req: &BTreeMap<&str, (&str, Option<&Value>)>,
) -> Vec<(String, Vec<String>)> {
    let mut merged = Vec::new();
    for part in parts {
        let start_elsewhere = parts.iter().any(|other| {
            other.id != part.id
                && other
                    .requirements
                    .iter()
                    .any(|id| by_req.get(id.as_str()).is_some_and(|&(s, _)| s == BOOTSTRAP))
        });
        let mut stems = BTreeSet::new();
        for id in &part.requirements {
            let Some(&(req_stem, trigger)) = by_req.get(id.as_str()) else {
                continue;
            };
            let trigger_pulled = req_stem == BOOTSTRAP
                && start_elsewhere
                && trigger.is_some_and(|trig| {
                    part.requirements.iter().any(|other| {
                        by_req.get(other.as_str()).is_some_and(|&(other_stem, other_trig)| {
                            other_stem != BOOTSTRAP && other_trig == Some(trig)
                        })
                    })
                });
            if !trigger_pulled {
                stems.insert(req_stem.to_owned());
            }
        }
        if stems.len() > 1 {
            merged.push((part.name.clone(), stems.into_iter().collect()));
        }
    }
    merged
}

fn classify_edges(
    parts: &[PartNode], by_req: &BTreeMap<&str, (&str, Option<&Value>)>, design: Option<&Value>,
) -> (Vec<Edge>, Vec<Edge>, usize) {
    let signatures: BTreeMap<&str, &str> = design
        .and_then(|design| design["sections"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|section| section["blocks"].as_array())
        .flatten()
        .filter_map(|block| {
            let ty = block.get("type")?;
            Some((ty["key"].as_str()?, ty["signature"].as_str()?))
        })
        .collect();
    let by_id: BTreeMap<&str, &PartNode> =
        parts.iter().map(|part| (part.id.as_str(), part)).collect();

    let mut type_backed = Vec::new();
    let mut unbacked = Vec::new();
    let mut unbacked_start = 0;

    for part in parts {
        for dep_id in &part.depends_on {
            if dep_id == &part.id {
                continue;
            }
            let Some(&dep) = by_id.get(dep_id.as_str()) else {
                continue;
            };
            let mut backed = Vec::new();
            for dep_key in &dep.types {
                let target = type_name(dep_key);
                if target.is_empty() || backed.iter().any(|seen| seen == target) {
                    continue;
                }
                let referenced = part.types.iter().any(|own_key| {
                    let Some(&sig) = signatures.get(own_key.as_str()) else {
                        return false;
                    };
                    let required = if type_name(own_key) == target { 2 } else { 1 };
                    word_count(sig, target) >= required
                });
                if referenced {
                    backed.push(target.to_owned());
                }
            }
            if backed.is_empty() {
                if dep.is_bootstrap(by_req) {
                    unbacked_start += 1;
                }
                unbacked.push(Edge {
                    from: part.name.clone(),
                    to: dep.name.clone(),
                    types: Vec::new(),
                });
            } else {
                type_backed.push(Edge {
                    from: part.name.clone(),
                    to: dep.name.clone(),
                    types: backed,
                });
            }
        }
    }

    (type_backed, unbacked, unbacked_start)
}

fn word_count(text: &str, word: &str) -> usize {
    if word.is_empty() {
        return 0;
    }
    let ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut count = 0;
    let mut from = 0;
    while let Some(at) = text[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = text[..start].chars().next_back().is_none_or(|ch| !ident(ch));
        let after = text[end..].chars().next().is_none_or(|ch| !ident(ch));
        if before && after {
            count += 1;
        }
        from = end;
    }
    count
}
