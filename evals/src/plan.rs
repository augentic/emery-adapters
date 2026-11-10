//! Measures a staged plan's parts, waves, merged stems, and dependency edges.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};

use serde_json::Value;

use crate::build::strings;
use crate::grade::stem;

const BOOTSTRAP: &str = "start";
const KEYWORDS: &[&str] = &["interface", "type", "class", "struct", "enum", "const", "def"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlanShape {
    pub parts: usize,
    pub waves: usize,
    pub widest: usize,
    pub wave_shape: String,
    // Each part that merges two or more domain stems: `(part_name, stems)`.
    pub merged: Vec<(String, Vec<String>)>,
    // Whether the plan JSON carries grammar 10's `uses` field on its parts.
    pub uses_edges: bool,
    pub derived: Vec<Edge>,
    pub named: Vec<Edge>,
    pub onto_typeless: usize,
    pub type_backed: Vec<Edge>,
    pub unbacked: Vec<Edge>,
    pub unbacked_start: usize,
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
        let uses_edges = entries.iter().any(|entry| entry.get("uses").is_some());
        let parts: Vec<PartNode> = entries.iter().map(PartNode::of).collect();

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
        let classified = ClassifiedEdges::of(&parts, &by_req, design);

        Self {
            parts: parts.len(),
            waves: widths.len(),
            widest,
            wave_shape,
            merged,
            uses_edges,
            derived: classified.derived,
            named: classified.named,
            onto_typeless: classified.onto_typeless,
            type_backed: classified.type_backed,
            unbacked: classified.unbacked,
            unbacked_start: classified.unbacked_start,
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

    pub fn derived_cell(&self) -> String {
        edges_cell(&self.derived)
    }

    pub fn named_cell(&self) -> String {
        edges_cell(&self.named)
    }

    pub fn type_backed_cell(&self) -> String {
        edges_cell(&self.type_backed)
    }

    pub fn unbacked_cell(&self) -> String {
        edges_cell(&self.unbacked)
    }
}

fn edges_cell(edges: &[Edge]) -> String {
    if edges.is_empty() {
        return "0".to_owned();
    }
    let detail: Vec<String> = edges.iter().map(Edge::to_string).collect();
    format!("{} ({})", edges.len(), detail.join(", "))
}

struct PartNode {
    id: String,
    name: String,
    requirements: Vec<String>,
    types: Vec<String>,
    uses: Vec<String>,
    depends_on: Vec<String>,
}

impl PartNode {
    fn of(entry: &Value) -> Self {
        Self {
            id: entry["id"].as_str().unwrap_or("").to_owned(),
            name: entry["name"].as_str().unwrap_or("").to_owned(),
            requirements: strings(&entry["requirements"]),
            types: strings(&entry["types"]),
            uses: strings(&entry["uses"]),
            depends_on: strings(&entry["depends-on"]),
        }
    }

    fn is_bootstrap(&self, by_req: &BTreeMap<&str, (&str, Option<&Value>)>) -> bool {
        self.name == BOOTSTRAP
            || (!self.requirements.is_empty()
                && self
                    .requirements
                    .iter()
                    .all(|id| by_req.get(id.as_str()).is_some_and(|&(s, _)| s == BOOTSTRAP)))
    }
}

#[derive(Default)]
struct ClassifiedEdges {
    derived: Vec<Edge>,
    named: Vec<Edge>,
    onto_typeless: usize,
    type_backed: Vec<Edge>,
    unbacked: Vec<Edge>,
    unbacked_start: usize,
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

struct DesignTypes<'a> {
    signatures: BTreeMap<&'a str, &'a str>,
    declared: BTreeMap<&'a str, &'a str>,
    counts: BTreeMap<&'a str, usize>,
}

impl<'a> DesignTypes<'a> {
    fn of(design: Option<&'a Value>) -> Self {
        let signatures: BTreeMap<&str, &str> = design
            .and_then(|design| design["sections"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|section| section["blocks"].as_array())
            .flatten()
            .filter_map(|block| {
                let ty = block.get("type")?;
                if ty.get("role").and_then(Value::as_str).is_some_and(|role| role != "data") {
                    return None;
                }
                Some((ty["key"].as_str()?, ty["signature"].as_str()?))
            })
            .collect();
        let declared: BTreeMap<&str, &str> =
            signatures.iter().map(|(&key, &sig)| (key, declared_name(sig, key))).collect();
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for &name in declared.values() {
            *counts.entry(name).or_default() += 1;
        }
        Self {
            signatures,
            declared,
            counts,
        }
    }

    fn name(&self, key: &'a str) -> &'a str {
        self.declared.get(key).copied().unwrap_or_else(|| type_name(key))
    }

    fn references(&self, own_key: &str, dep_key: &'a str) -> Option<&'a str> {
        if own_key == dep_key {
            return None;
        }
        let target = self.name(dep_key);
        if target.is_empty() || self.counts.get(target).is_some_and(|&count| count > 1) {
            return None;
        }
        let &sig = self.signatures.get(own_key)?;
        let required = if self.name(own_key) == target { 2 } else { 1 };
        (word_count(sig, target) >= required).then_some(target)
    }

    fn edge_types(&self, from: &PartNode, to: &'a PartNode) -> (Vec<String>, Vec<String>) {
        let mut backed = Vec::new();
        let mut used = Vec::new();
        for dep_key in &to.types {
            if let Some(target) =
                from.types.iter().find_map(|own_key| self.references(own_key, dep_key))
            {
                if !backed.iter().any(|seen| seen == target) {
                    backed.push(target.to_owned());
                }
            } else if from.uses.contains(dep_key) {
                let bare = bare_key(dep_key);
                if !used.iter().any(|seen| seen == bare) {
                    used.push(bare.to_owned());
                }
            }
        }
        (backed, used)
    }
}

impl ClassifiedEdges {
    fn of(
        parts: &[PartNode], by_req: &BTreeMap<&str, (&str, Option<&Value>)>, design: Option<&Value>,
    ) -> Self {
        let types = DesignTypes::of(design);
        let by_id: BTreeMap<&str, &PartNode> =
            parts.iter().map(|part| (part.id.as_str(), part)).collect();
        let mut out = Self::default();

        for part in parts {
            for dep_id in &part.depends_on {
                if dep_id == &part.id {
                    continue;
                }
                let Some(&dep) = by_id.get(dep_id.as_str()) else {
                    continue;
                };
                if dep.types.is_empty() {
                    out.onto_typeless += 1;
                }
                let (backed, used) = types.edge_types(part, dep);
                if !backed.is_empty() {
                    let edge = Edge {
                        from: part.name.clone(),
                        to: dep.name.clone(),
                        types: backed,
                    };
                    out.derived.push(edge.clone());
                    out.type_backed.push(edge);
                } else if !used.is_empty() {
                    out.named.push(Edge {
                        from: part.name.clone(),
                        to: dep.name.clone(),
                        types: used,
                    });
                } else {
                    if dep.is_bootstrap(by_req) {
                        out.unbacked_start += 1;
                    }
                    out.unbacked.push(Edge {
                        from: part.name.clone(),
                        to: dep.name.clone(),
                        types: Vec::new(),
                    });
                }
            }
        }

        out
    }
}
fn declared_name<'a>(signature: &'a str, key: &'a str) -> &'a str {
    for line in signature.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.starts_with("/*")
            || trimmed.starts_with('*')
            || trimmed.starts_with('@')
        {
            continue;
        }
        let head = trimmed.split(['{', '=']).next().unwrap_or(trimmed);
        let mut iter = head
            .split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
            .filter(|token| !token.is_empty());
        while let Some(token) = iter.next() {
            if KEYWORDS.contains(&token)
                && let Some(name) = iter.find(|candidate| {
                    !KEYWORDS.contains(candidate)
                        && candidate
                            .chars()
                            .next()
                            .is_some_and(|ch| ch.is_alphabetic() || ch == '_')
                })
            {
                return name;
            }
        }
        if trimmed.contains(['{', '=']) {
            break;
        }
    }
    type_name(key)
}

fn word_count(text: &str, word: &str) -> usize {
    if word.is_empty() {
        return 0;
    }
    text.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .filter(|&token| token == word)
        .count()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    pub to: String,
    // The dependency's type names or keys explaining the edge; empty for an
    // unbacked edge.
    pub types: Vec<String>,
}

impl Display for Edge {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.types.is_empty() {
            write!(f, "`{}` → `{}`", self.from, self.to)
        } else {
            let types: Vec<String> = self.types.iter().map(|t| format!("`{t}`")).collect();
            write!(f, "`{}` → `{}` ({})", self.from, self.to, types.join(", "))
        }
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
    let dotted = bare_key(key);
    dotted.rsplit_once('.').map_or(dotted, |(_, name)| name)
}

// A design type key without its trailing `(path#Ln)` source citation.
fn bare_key(key: &str) -> &str {
    key.split(" (").next().unwrap_or(key).trim()
}
