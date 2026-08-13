//! Component-identity detection: cluster recurring composition groups
//! into the infer report's catalog candidates.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::VectisError;
use crate::validate::engine::composition::{
    Skeleton, build_group_skeleton, fingerprint, skeleton_to_json,
};

const SCREEN_REGIONS: &[&str] =
    &["header", "body", "footer", "fab", "states", "overlays", "platforms"];

/// Default clustering threshold: distinct screens a group must span.
pub const DEFAULT_MIN_OCCURRENCES: u32 = 2;

/// Inputs for one inference run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferArgs {
    /// Composition baseline to cluster (`.emery/specs/composition.yaml`).
    pub composition: PathBuf,

    /// `.emery/slices` root. `*/evidence/*.yaml` claims with
    /// `notes.candidate_component` reconstruct as group occurrences.
    pub slices: Option<PathBuf>,

    /// Optional extra composition-shaped candidate YAML tree, keyed
    /// by provenance. Overlay on top of Evidence reconstruction.
    pub candidate_cache: Option<PathBuf>,

    /// Operator parts file: authoritative parts that seed inference
    /// with naming + promotion authority.
    pub parts: Option<PathBuf>,

    /// Minimum distinct screens a group must span to cluster.
    pub min_occurrences: u32,
}

struct GroupOccurrence {
    screen: String,
    region: String,
    skeleton: Skeleton,
    event_targets: BTreeSet<String>,
    candidate_name: Option<String>,
}

struct Cluster {
    screens: BTreeSet<String>,
    region: String,
    skeleton: Skeleton,
    representative_screen: String,
    event_targets: BTreeSet<String>,
    candidate_names: BTreeSet<String>,
}

/// Run the inference engine over the composition baseline.
///
/// # Errors
///
/// Returns [`VectisError::InvalidProject`] when the composition file is
/// unreadable or is not valid YAML.
pub fn run(args: &InferArgs) -> Result<Value, VectisError> {
    let source =
        std::fs::read_to_string(&args.composition).map_err(|err| VectisError::InvalidProject {
            message: format!(
                "composition baseline not readable at {}: {err}",
                args.composition.display()
            ),
        })?;
    let instance: Value =
        serde_saphyr::from_str(&source).map_err(|err| VectisError::InvalidProject {
            message: format!(
                "composition baseline at {} is not valid YAML: {err}",
                args.composition.display()
            ),
        })?;

    let mut occurrences: Vec<GroupOccurrence> = Vec::new();
    collect_baseline_groups(&instance, &mut occurrences);
    if let Some(slices_dir) = &args.slices
        && slices_dir.is_dir()
    {
        collect_evidence_groups(slices_dir, &mut occurrences);
    }
    if let Some(cache_dir) = &args.candidate_cache
        && cache_dir.is_dir()
    {
        collect_cached_groups(cache_dir, &mut occurrences);
    }

    // Register a pinned binding per operator part.
    let pins =
        args.parts.as_ref().map_or_else(Vec::new, |parts_path| collect_part_pins(parts_path));

    // Fingerprints present in the baseline/Evidence/cache, splitting
    // matched pins (promoted + named) from unmatched pins (reported).
    let observed: BTreeSet<String> =
        occurrences.iter().map(|occ| fingerprint(&occ.skeleton)).collect();

    // §C6 dedup: when two parts normalise to one skeleton, the
    // lexicographically-first slug binds the fingerprint.
    let mut pin_slug_by_fp: BTreeMap<String, String> = BTreeMap::new();
    let mut unmatched_parts: BTreeSet<String> = BTreeSet::new();
    for pin in pins {
        if observed.contains(&pin.fingerprint) {
            pin_slug_by_fp.entry(pin.fingerprint).or_insert(pin.slug);
        } else {
            // §C2 step 4: a pin matching zero groups surfaces as an
            // unused part (§C5), never a cluster.
            unmatched_parts.insert(pin.slug);
        }
    }

    let clusters = cluster(occurrences, args.min_occurrences, &pin_slug_by_fp);

    Ok(json!({
        "version": 1,
        "clusters": clusters,
        "unmatched-parts": unmatched_parts.into_iter().collect::<Vec<_>>(),
    }))
}

fn collect_baseline_groups(instance: &Value, out: &mut Vec<GroupOccurrence>) {
    if let Some(screens) = instance.get("screens").and_then(Value::as_object) {
        for (slug, entry) in screens {
            collect_screen_groups(slug, entry, out);
        }
    }
    if let Some(delta) = instance.get("delta").and_then(Value::as_object) {
        for section in ["added", "modified"] {
            if let Some(screens) = delta.get(section).and_then(Value::as_object) {
                for (slug, entry) in screens {
                    collect_screen_groups(slug, entry, out);
                }
            }
        }
    }
}

fn collect_screen_groups(screen: &str, entry: &Value, out: &mut Vec<GroupOccurrence>) {
    let Some(map) = entry.as_object() else {
        return;
    };
    for (key, val) in map {
        if SCREEN_REGIONS.contains(&key.as_str()) {
            walk_region_for_groups(screen, key, val, out);
        }
    }
}

fn walk_region_for_groups(
    screen: &str, region: &str, node: &Value, out: &mut Vec<GroupOccurrence>,
) {
    match node {
        Value::Object(map) => {
            for (key, val) in map {
                if key == "group" {
                    let mut event_targets = BTreeSet::new();
                    collect_event_targets(val, &mut event_targets);
                    out.push(GroupOccurrence {
                        screen: screen.to_string(),
                        region: region.to_string(),
                        skeleton: build_group_skeleton(val),
                        event_targets,
                        candidate_name: None,
                    });
                }
                walk_region_for_groups(screen, region, val, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                walk_region_for_groups(screen, region, v, out);
            }
        }
        _ => {}
    }
}

fn collect_event_targets(node: &Value, out: &mut BTreeSet<String>) {
    match node {
        Value::Object(map) => {
            for (key, val) in map {
                if key == "event"
                    && let Some(target) = val.as_str()
                {
                    out.insert(target.to_string());
                }
                collect_event_targets(val, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                collect_event_targets(v, out);
            }
        }
        _ => {}
    }
}

fn collect_evidence_groups(slices_dir: &Path, out: &mut Vec<GroupOccurrence>) {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_slice_evidence_files(slices_dir, &mut files);
    files.sort();
    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(doc) = serde_saphyr::from_str::<Value>(&source) else {
            continue;
        };
        collect_evidence_doc(&doc, out);
    }
}

fn collect_slice_evidence_files(slices_dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(slices) = std::fs::read_dir(slices_dir) else {
        return;
    };
    for slice in slices.flatten() {
        let evidence = slice.path().join("evidence");
        if evidence.is_dir() {
            collect_yaml_files(&evidence, out);
        }
    }
}

fn collect_evidence_doc(doc: &Value, out: &mut Vec<GroupOccurrence>) {
    let Some(claims) = doc.get("claims").and_then(Value::as_array) else {
        return;
    };
    let lead = doc.get("lead").and_then(Value::as_str).unwrap_or("");
    let mut by_parent: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, claim) in claims.iter().enumerate() {
        if let Some(parent) = claim.get("parent").and_then(Value::as_str) {
            by_parent.entry(parent.to_string()).or_default().push(index);
        }
    }
    for claim in claims {
        if !is_group_container(claim) {
            continue;
        }
        let Some(name) = candidate_component(claim) else {
            continue;
        };
        let screen = claim.get("screen").and_then(Value::as_str).unwrap_or(lead).to_string();
        let region = claim.get("region").and_then(Value::as_str).unwrap_or_default().to_string();
        let group = assemble_group(claim, claims, &by_parent);
        let mut event_targets = BTreeSet::new();
        collect_event_targets(&group, &mut event_targets);
        out.push(GroupOccurrence {
            screen,
            region,
            skeleton: build_group_skeleton(&group),
            event_targets,
            candidate_name: Some(name.to_string()),
        });
    }
}

fn is_group_container(claim: &Value) -> bool {
    claim.get("kind").and_then(Value::as_str) == Some("container")
        && claim.get("container").and_then(Value::as_str) == Some("group")
}

fn candidate_component(claim: &Value) -> Option<&str> {
    let notes = claim.get("notes")?;
    notes
        .get("candidate_component")
        .or_else(|| notes.get("candidate-component"))
        .and_then(Value::as_str)
}

fn assemble_group(
    claim: &Value, claims: &[Value], by_parent: &BTreeMap<String, Vec<usize>>,
) -> Value {
    let id = claim.get("id").and_then(Value::as_str).unwrap_or("");
    json!({ "items": assemble_items(id, claim_direction(claim), claims, by_parent) })
}

fn assemble_items(
    parent_id: &str, direction: Option<&str>, claims: &[Value],
    by_parent: &BTreeMap<String, Vec<usize>>,
) -> Vec<Value> {
    let Some(indexes) = by_parent.get(parent_id) else {
        return Vec::new();
    };
    // Claim-array order is not visual order for mixed nested-group /
    // leaf children; sort by bbox when every sibling has one.
    let mut ordered: Vec<usize> = indexes.clone();
    if ordered.iter().all(|&index| bbox_xy(&claims[index]).is_some()) {
        ordered.sort_by_key(|&index| sibling_key(&claims[index], direction, index));
    }
    ordered.iter().filter_map(|&index| claim_to_item(&claims[index], claims, by_parent)).collect()
}

fn claim_to_item(
    claim: &Value, claims: &[Value], by_parent: &BTreeMap<String, Vec<usize>>,
) -> Option<Value> {
    match claim.get("kind").and_then(Value::as_str)? {
        "leaf" => {
            let leaf = claim.get("leaf").and_then(Value::as_str)?;
            Some(json!({ leaf: {} }))
        }
        "container" => {
            let container = claim.get("container").and_then(Value::as_str)?;
            if container == "group" {
                let id = claim.get("id").and_then(Value::as_str).unwrap_or("");
                Some(json!({
                    "group": {
                        "items": assemble_items(id, claim_direction(claim), claims, by_parent)
                    }
                }))
            } else {
                Some(json!({ container: {} }))
            }
        }
        _ => None,
    }
}

fn claim_direction(claim: &Value) -> Option<&str> {
    claim.get("direction").and_then(Value::as_str)
}

fn bbox_xy(claim: &Value) -> Option<(i64, i64)> {
    let bbox = claim.get("bbox")?;
    Some((json_i64(bbox.get("x")?)?, json_i64(bbox.get("y")?)?))
}

fn json_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
}

fn sibling_key(claim: &Value, direction: Option<&str>, index: usize) -> (i64, i64, usize) {
    match bbox_xy(claim) {
        Some((x, y)) if direction == Some("row") => (x, y, index),
        Some((x, y)) => (y, x, index),
        None => (i64::MAX, 0, index),
    }
}

fn collect_cached_groups(dir: &Path, out: &mut Vec<GroupOccurrence>) {
    let mut files: Vec<PathBuf> = Vec::new();
    collect_yaml_files(dir, &mut files);
    // Deterministic read order keeps the representative-screen and
    // candidate-name accumulation stable across runs.
    files.sort();
    for file in &files {
        let Ok(source) = std::fs::read_to_string(file) else {
            continue;
        };
        let Ok(entry) = serde_saphyr::from_str::<Value>(&source) else {
            continue;
        };
        let Some(group) = entry.get("group") else {
            continue;
        };
        let mut event_targets = BTreeSet::new();
        collect_event_targets(group, &mut event_targets);
        out.push(GroupOccurrence {
            screen: cache_screen_id(dir, file),
            region: entry.get("region").and_then(Value::as_str).unwrap_or_default().to_string(),
            skeleton: build_group_skeleton(group),
            event_targets,
            candidate_name: entry
                .get("candidate_component")
                .and_then(Value::as_str)
                .map(str::to_string),
        });
    }
}

fn collect_yaml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_yaml_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "yaml") {
            out.push(path);
        }
    }
}

fn cache_screen_id(root: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(root).unwrap_or(file);
    let components: Vec<String> =
        rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    components.get(1).cloned().unwrap_or_else(|| {
        file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    })
}

struct PartPin {
    slug: String,
    fingerprint: String,
}

fn collect_part_pins(path: &Path) -> Vec<PartPin> {
    let Ok(source) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(doc) = serde_saphyr::from_str::<Value>(&source) else {
        return Vec::new();
    };
    let Some(parts) = doc.get("parts").and_then(Value::as_object) else {
        return Vec::new();
    };
    parts
        .iter()
        .filter_map(|(slug, entry)| {
            let group = entry.get("group")?;
            Some(PartPin {
                slug: slug.clone(),
                fingerprint: fingerprint(&build_group_skeleton(group)),
            })
        })
        .collect()
}

fn cluster(
    occurrences: Vec<GroupOccurrence>, min_occurrences: u32,
    pin_slug_by_fp: &BTreeMap<String, String>,
) -> Vec<Value> {
    let mut by_fp: BTreeMap<String, Cluster> = BTreeMap::new();
    for occ in occurrences {
        let fp = fingerprint(&occ.skeleton);
        let entry = by_fp.entry(fp).or_insert_with(|| Cluster {
            screens: BTreeSet::new(),
            region: occ.region.clone(),
            skeleton: occ.skeleton.clone(),
            representative_screen: occ.screen.clone(),
            event_targets: BTreeSet::new(),
            candidate_names: BTreeSet::new(),
        });
        if occ.screen < entry.representative_screen {
            entry.representative_screen.clone_from(&occ.screen);
            entry.region.clone_from(&occ.region);
            entry.skeleton = occ.skeleton.clone();
        }
        entry.screens.insert(occ.screen);
        entry.event_targets.extend(occ.event_targets);
        if let Some(name) = occ.candidate_name {
            entry.candidate_names.insert(name);
        }
    }

    by_fp
        .into_iter()
        // A pinned fingerprint bypasses the threshold (§C2 promotion);
        // every other cluster must span ≥ min_occurrences distinct screens.
        .filter(|(fp, c)| {
            pin_slug_by_fp.contains_key(fp)
                || u32::try_from(c.screens.len()).is_ok_and(|n| n >= min_occurrences)
        })
        .map(|(fp, c)| {
            let mut item_kinds = BTreeSet::new();
            skeleton_item_kinds(&c.skeleton, &mut item_kinds);
            let mut evidence = json!({
                "region": c.region,
                "item-kinds": item_kinds.into_iter().collect::<Vec<_>>(),
                "event-targets": c.event_targets.into_iter().collect::<Vec<_>>(),
            });
            // Emit name hints only when present so a baseline-only
            // report keeps its evidence shape unchanged.
            if !c.candidate_names.is_empty()
                && let Value::Object(ref mut map) = evidence
            {
                map.insert(
                    "candidate-names".to_string(),
                    json!(c.candidate_names.into_iter().collect::<Vec<_>>()),
                );
            }
            let mut entry = json!({
                "fingerprint": fp,
                "occurrences": c.screens.len(),
                "screens": c.screens.into_iter().collect::<Vec<_>>(),
                "skeleton": skeleton_to_json(&c.skeleton),
                "evidence": evidence,
                "bound-slug": Value::Null,
            });
            // §C2 step 5: a matched pin echoes the operator slug;
            // `pinned` is emitted only when true so a pin-free report
            // keeps its existing cluster shape.
            if let Some(slug) = pin_slug_by_fp.get(&fp)
                && let Value::Object(ref mut map) = entry
            {
                map.insert("bound-slug".to_string(), Value::String(slug.clone()));
                map.insert("pinned".to_string(), Value::Bool(true));
            }
            entry
        })
        .collect()
}

fn skeleton_item_kinds(skeleton: &Skeleton, out: &mut BTreeSet<String>) {
    match skeleton {
        Skeleton::Item(kind) => {
            out.insert(kind.clone());
        }
        Skeleton::Group { items, .. } => {
            for item in items {
                skeleton_item_kinds(item, out);
            }
        }
    }
}
