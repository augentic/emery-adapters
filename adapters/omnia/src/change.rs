//! Classifies each stem a slice touches against the built revision and the tree.

mod touch;

use std::collections::{BTreeMap, BTreeSet};

use emery_sdk::push_unique;
use emery_sdk::target::{Slice, SliceChange, Standing};

pub use self::touch::TouchPoint;
use self::touch::TouchPoints;
use crate::layout;
use crate::scaffold::{DesignKey, trigger};
use crate::tree::Tree;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StemChange {
    // `first` is true when the tree already holds the stem's crate and no
    // earlier slice of this build rebuilt a requirement of it.
    Reached { first: bool },
    Kept,
    Vanished,
}

#[derive(Default)]
pub struct Change {
    stems: BTreeMap<String, StemChange>,
    kept_requirements: Vec<String>,
    touch_points: Vec<TouchPoint>,
    is_retiring: bool,
}

impl Change {
    pub fn read(slice: &Slice, tree: &Tree<'_>) -> Self {
        let Some(change) = &slice.change else {
            return Self::default();
        };
        let existing: BTreeSet<String> = tree.stem_crates().map(str::to_owned).collect();
        let stems = slice_stems(slice, change);
        let reached = reached_stems(slice, change, &stems);
        let built_before = built_before_stems(tree, change, &stems);

        let mut classified = BTreeMap::new();
        for stem in &stems {
            let dir = layout::crate_dir(stem);
            let status = if !existing.contains(dir) {
                StemChange::Reached { first: false }
            } else if reached.contains(stem) {
                StemChange::Reached {
                    first: !built_before.contains(stem),
                }
            } else {
                StemChange::Kept
            };
            classified.insert(stem.clone(), status);
        }
        for vanished in &change.vanished {
            if !layout::is_start_stem(vanished) {
                classified.insert(vanished.clone(), StemChange::Vanished);
            }
        }

        let kept_requirements = slice
            .requirements
            .iter()
            .filter(|requirement| {
                trigger::stem(requirement)
                    .is_some_and(|stem| classified.get(&stem) == Some(&StemChange::Kept))
            })
            .map(|requirement| requirement.id.clone())
            .collect();

        let touched: Vec<String> = classified
            .iter()
            .filter(|(_, status)| {
                matches!(status, StemChange::Reached { .. } | StemChange::Vanished)
            })
            .map(|(stem, _)| stem.clone())
            .collect();
        let touch_points = TouchPoints::read(tree, &touched);

        Self {
            stems: classified,
            kept_requirements,
            touch_points,
            is_retiring: slice.is_retiring(),
        }
    }

    pub const fn is_retiring(&self) -> bool {
        self.is_retiring
    }

    pub fn is_kept(&self, stem: &str) -> bool {
        self.stems.get(stem) == Some(&StemChange::Kept)
    }

    pub fn kept_requirements(&self) -> &[String] {
        &self.kept_requirements
    }

    pub fn touch_points(&self) -> &[TouchPoint] {
        &self.touch_points
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
        if let Some(stem) = DesignKey::parse(&design_type.key).stem
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

// Which of `stems` the change reaches: an added or changed requirement, a
// departure, a changed owned or used type, or every stem of the slice when
// the part was widened into scope by a design section change alone.
fn reached_stems(slice: &Slice, change: &SliceChange, stems: &[String]) -> BTreeSet<String> {
    let mut reached = BTreeSet::new();
    for (requirement, standing) in slice.requirements.iter().zip(&change.standing) {
        if matches!(standing, Standing::Added | Standing::Changed)
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
    let fallback = || stems.first().cloned();
    for design_type in &slice.types {
        if change.types.iter().any(|changed| changed.key == design_type.key)
            && let Some(stem) =
                DesignKey::parse(&design_type.key).stem.map(str::to_owned).or_else(fallback)
        {
            reached.insert(stem);
        }
    }
    for changed in &change.types {
        if let Some(stem) = DesignKey::parse(&changed.key).stem.map(str::to_owned).or_else(fallback)
            && stems.contains(&stem)
        {
            reached.insert(stem);
        }
    }
    if reached.is_empty() && !change.sections.is_empty() {
        reached.extend(stems.iter().cloned());
    }
    reached
}

// The stems of `stems` that an earlier slice of this build already rebuilt,
// identified by a `built_before` requirement id appearing in the stem's crate.
fn built_before_stems(tree: &Tree<'_>, change: &SliceChange, stems: &[String]) -> BTreeSet<String> {
    if change.built_before.is_empty() {
        return BTreeSet::new();
    }
    let matched: BTreeSet<String> = stems
        .iter()
        .filter(|stem| {
            let dir = layout::crate_dir(stem);
            tree.stem_files(dir).any(|module| {
                change.built_before.iter().any(|id| module.text.contains(id.as_str()))
            })
        })
        .cloned()
        .collect();
    if matched.is_empty() { stems.iter().cloned().collect() } else { matched }
}
