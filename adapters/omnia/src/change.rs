//! Scopes a revision change and classifies each stem a slice touches against the built tree.

mod rewrite;
mod scope;
mod touch;

use std::collections::{BTreeMap, BTreeSet};

use emery_sdk::push_unique;
use emery_sdk::target::{Report, Slice, SliceChange, Standing};

pub use self::scope::scope;
pub use self::touch::TouchPoint;
use self::touch::TouchPoints;
use crate::layout;
use crate::scaffold::trigger;
use crate::syntax::ExportedFn;
use crate::tree::Tree;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StemChange {
    // `first` is true when the tree already holds the stem's crate and no
    // earlier slice of this build rebuilt a requirement of it.
    Reached { first: bool },
    Kept,
    Vanished,
}

impl StemChange {
    const fn classify(exists_in_tree: bool, built_before: bool, reached: bool) -> Self {
        if !exists_in_tree || built_before {
            Self::Reached { first: false }
        } else if reached {
            Self::Reached { first: true }
        } else {
            Self::Kept
        }
    }

    const fn is_rewritten(self) -> bool {
        matches!(self, Self::Reached { first: true })
    }

    const fn is_touched(self) -> bool {
        matches!(self, Self::Reached { .. } | Self::Vanished)
    }
}

#[derive(Default)]
pub struct Change {
    stems: BTreeMap<String, StemChange>,
    kept_requirements: Vec<String>,
    touch_points: Vec<TouchPoint>,
    prior_exports: Vec<ExportedFn>,
    rewritten_lists: BTreeMap<String, String>,
    vanished_lines: BTreeSet<String>,
    is_retiring: bool,
}

impl Change {
    pub fn read(slice: &Slice, tree: &Tree) -> Self {
        let Some(change) = &slice.change else {
            return Self::default();
        };
        let existing: BTreeSet<String> = tree.stem_crates().map(str::to_owned).collect();
        let slice_stems = slice_stems(slice, change);
        let reached = reached_stems(slice, change, &slice_stems);
        let built_before = built_before_stems(tree, change, &slice_stems);

        let mut stems = BTreeMap::new();
        for stem in &slice_stems {
            let dir = layout::crate_dir(stem);
            let status = StemChange::classify(
                existing.contains(dir),
                built_before.contains(stem),
                reached.contains(stem),
            );
            stems.insert(stem.clone(), status);
        }
        for vanished in &change.vanished {
            if !layout::is_start_stem(vanished) {
                stems.insert(vanished.clone(), StemChange::Vanished);
            }
        }

        let kept_requirements = slice
            .requirements
            .iter()
            .filter(|requirement| {
                trigger::stem(requirement)
                    .is_some_and(|stem| stems.get(&stem) == Some(&StemChange::Kept))
            })
            .map(|requirement| requirement.id.clone())
            .collect();

        let touched: Vec<String> = stems
            .iter()
            .filter(|(_, status)| status.is_touched())
            .map(|(stem, _)| stem.clone())
            .collect();
        let touch_points = TouchPoints::read(tree, &touched);

        Self {
            stems,
            kept_requirements,
            touch_points,
            prior_exports: Vec::new(),
            rewritten_lists: BTreeMap::new(),
            vanished_lines: BTreeSet::new(),
            is_retiring: slice.is_retiring(),
        }
    }

    pub const fn is_retiring(&self) -> bool {
        self.is_retiring
    }

    pub fn vanished_stems(&self) -> impl Iterator<Item = &str> {
        self.stems
            .iter()
            .filter(|(_, status)| **status == StemChange::Vanished)
            .map(|(stem, _)| stem.as_str())
    }

    pub fn is_kept(&self, stem: &str) -> bool {
        self.stems.get(stem) == Some(&StemChange::Kept)
    }

    pub fn is_rewritten(&self, stem: &str) -> bool {
        self.stems.get(stem).copied().is_some_and(StemChange::is_rewritten)
    }

    pub fn quiet_report(self) -> Report {
        Report {
            covered: self.kept_requirements,
            written: self.rewritten_lists.into_keys().collect(),
        }
    }

    pub fn cover_kept(self, report: &mut Report) {
        for id in self.kept_requirements {
            push_unique(&mut report.covered, id);
        }
    }

    pub fn touch_points(&self) -> &[TouchPoint] {
        &self.touch_points
    }

    pub fn prior_exports(&self) -> &[ExportedFn] {
        &self.prior_exports
    }

    pub const fn rewritten_lists(&self) -> &BTreeMap<String, String> {
        &self.rewritten_lists
    }

    pub const fn vanished_lines(&self) -> &BTreeSet<String> {
        &self.vanished_lines
    }
}

fn slice_stems(slice: &Slice, change: &SliceChange) -> Vec<String> {
    let mut stems = Vec::new();
    for requirement in &slice.requirements {
        if let Some(stem) = trigger::stem(requirement) {
            push_unique(&mut stems, stem);
        }
    }
    for design_type in &slice.types {
        if let Some(stem) = design_type.stem()
            && !layout::is_start_stem(stem)
        {
            push_unique(&mut stems, stem.to_owned());
        }
    }
    for departure in &change.departed {
        if let Some(stem) = trigger::stem(&departure.requirement)
            && !change.vanished.contains(&stem)
        {
            push_unique(&mut stems, stem);
        }
    }
    stems
}

// A slice carrying no delta of its own was pulled into scope by a design
// section or a sibling slice, so every stem it touches is reached.
fn reached_stems(slice: &Slice, change: &SliceChange, stems: &[String]) -> BTreeSet<String> {
    let mut reached = BTreeSet::new();
    for (requirement, standing) in slice.requirements.iter().zip(&change.standing) {
        if is_changed(*standing)
            && let Some(stem) = trigger::stem(requirement)
        {
            reached.insert(stem);
        }
    }
    for departure in &change.departed {
        if let Some(stem) = trigger::stem(&departure.requirement) {
            reached.insert(stem);
        }
    }
    for changed in &change.types {
        let stem = changed.stem().map(str::to_owned).or_else(|| stems.first().cloned());
        if let Some(stem) = stem
            && stems.contains(&stem)
        {
            reached.insert(stem);
        }
    }
    if !carries_delta(change) && !slice.is_retiring() {
        reached.extend(stems.iter().cloned());
    }
    reached
}

const fn is_changed(standing: Standing) -> bool {
    matches!(standing, Standing::Added | Standing::Changed)
}

// Includes untriggered `start` changes, which lay no stem of their own and
// must not widen a carried stem sharing the slice.
fn carries_delta(change: &SliceChange) -> bool {
    change.standing.iter().copied().any(is_changed)
        || !change.departed.is_empty()
        || !change.types.is_empty()
}

// Earlier slices of the same part may be no-op `start` slices, so only stems
// whose crate already mentions a `built_before` id were rebuilt in this run.
fn built_before_stems(tree: &Tree, change: &SliceChange, stems: &[String]) -> BTreeSet<String> {
    if change.built_before.is_empty() {
        return BTreeSet::new();
    }
    stems
        .iter()
        .filter(|stem| {
            let dir = layout::crate_dir(stem);
            tree.stem_files(dir).any(|module| {
                change.built_before.iter().any(|id| module.text.contains(id.as_str()))
            })
        })
        .cloned()
        .collect()
}
