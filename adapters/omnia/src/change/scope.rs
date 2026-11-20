//! Names the plan parts a change reaches through its changed design sections.

mod diff;
mod mentions;

use std::collections::BTreeSet;

use emery_sdk::target::{Change, PartReach};

use self::diff::{CitedEdit, ProseEdit, SectionDiff, UncitedEdit};
use self::mentions::{DomainTerms, ProseMentions, UrlPath, setting_tokens};
use crate::scaffold::{DesignKey, Settings, cited, trigger};
use crate::tree::Tree;
use crate::{layout, syntax};

pub fn scope(change: &Change, tree: &Tree) -> Vec<String> {
    if change.parts.is_empty() {
        return Vec::new();
    }
    let mut reached: BTreeSet<String> = change
        .parts
        .iter()
        .filter(|part| !part.reasons.is_empty())
        .map(|part| part.id.clone())
        .collect();

    if change.sections.iter().all(|section| SectionRule::of(&section.kind) == SectionRule::Ignored)
    {
        return ordered_part_ids(&change.parts, &reached);
    }

    let retired: BTreeSet<String> =
        change.retired.iter().map(|departure| departure.requirement.id.clone()).collect();
    let scoper = ChangeScope::new(change, tree);

    for section in &change.sections {
        let rule = SectionRule::of(&section.kind);
        if rule == SectionRule::Ignored {
            continue;
        }
        for edit in SectionDiff::edits(section, &retired) {
            match scoper.scope_edit(rule, &edit) {
                Reach::Parts(matched) => reached.extend(matched),
                Reach::AllParts => return scoper.all_part_ids(),
            }
        }
    }

    scoper.ordered_part_ids(&reached)
}

fn ordered_part_ids(parts: &[PartReach], reached: &BTreeSet<String>) -> Vec<String> {
    parts.iter().filter(|part| reached.contains(&part.id)).map(|part| part.id.clone()).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SectionRule {
    Ignored,
    Operational,
    DomainModel,
    AllParts,
}

impl SectionRule {
    fn of(kind: &str) -> Self {
        match kind {
            "overview" | "ui-layout" => Self::Ignored,
            "apis" | "technical-logic" | "observability" => Self::Operational,
            "domain-model" => Self::DomainModel,
            _ => Self::AllParts,
        }
    }
}

enum Reach {
    Parts(BTreeSet<String>),
    AllParts,
}

impl Reach {
    const fn none() -> Self {
        Self::Parts(BTreeSet::new())
    }

    fn parts_or_all(matched: BTreeSet<String>) -> Self {
        if matched.is_empty() { Self::AllParts } else { Self::Parts(matched) }
    }
}

struct ScopedPart {
    id: String,
    is_start: bool,
    requirements: BTreeSet<String>,
}

impl ScopedPart {
    fn from_reach(part: &PartReach) -> Self {
        Self {
            id: part.id.clone(),
            is_start: layout::is_start_stem(&part.name),
            requirements: part.requirements.iter().cloned().collect(),
        }
    }
}

struct ChangeScope {
    parts: Vec<ScopedPart>,
    built: BuiltTree,
    structural_domain: DomainTerms,
}

impl ChangeScope {
    fn new(change: &Change, tree: &Tree) -> Self {
        let built = BuiltTree::read(tree, &change.parts);
        let structural_domain = structural_domain_terms(change, &built);
        let parts = change.parts.iter().map(ScopedPart::from_reach).collect();
        Self {
            parts,
            built,
            structural_domain,
        }
    }

    fn ordered_part_ids(&self, reached: &BTreeSet<String>) -> Vec<String> {
        self.parts
            .iter()
            .filter(|part| reached.contains(&part.id))
            .map(|part| part.id.clone())
            .collect()
    }

    fn all_part_ids(&self) -> Vec<String> {
        self.parts.iter().map(|part| part.id.clone()).collect()
    }

    fn scope_edit(&self, rule: SectionRule, edit: &ProseEdit) -> Reach {
        match edit {
            ProseEdit::Cited(cited_edit) => self.scope_cited(rule, cited_edit),
            ProseEdit::Uncited(uncited) => self.scope_uncited(rule, uncited),
        }
    }

    // `start` parts match a citation only when a built crate owned by that part
    // cites the requirement. Untriggered `start` citations fall back to prose mentions.
    fn scope_cited(&self, rule: SectionRule, edit: &CitedEdit) -> Reach {
        let matched: BTreeSet<String> = self
            .parts
            .iter()
            .filter(|part| {
                if part.is_start {
                    self.built.crate_cites(&part.id, &edit.citations)
                } else {
                    !part.requirements.is_disjoint(&edit.citations)
                }
            })
            .map(|part| part.id.clone())
            .collect();
        if !matched.is_empty() {
            return Reach::Parts(matched);
        }

        let cites_untriggered_start = self
            .parts
            .iter()
            .any(|part| part.is_start && !part.requirements.is_disjoint(&edit.citations));
        if cites_untriggered_start {
            return self.scope_uncited(rule, &edit.prose);
        }

        Reach::none()
    }

    fn scope_uncited(&self, rule: SectionRule, edit: &UncitedEdit) -> Reach {
        match rule {
            SectionRule::Operational => self.scope_operational(edit),
            SectionRule::DomainModel => self.scope_domain_model(edit),
            SectionRule::AllParts => Reach::AllParts,
            SectionRule::Ignored => Reach::none(),
        }
    }

    fn scope_operational(&self, edit: &UncitedEdit) -> Reach {
        let by_delta_anchor = self.built.parts_with_setting_or_path(&edit.delta);
        if !by_delta_anchor.is_empty() {
            return Reach::Parts(by_delta_anchor);
        }
        if edit.delta.domain.intersects(&self.structural_domain) {
            return Reach::none();
        }
        let by_full_anchor = self.built.parts_with_setting_or_path(&edit.full);
        if !by_full_anchor.is_empty() {
            return Reach::Parts(by_full_anchor);
        }
        let by_delta_domain = self.built.parts_with_domain(&edit.delta);
        if !by_delta_domain.is_empty() {
            return Reach::Parts(by_delta_domain);
        }
        Reach::parts_or_all(self.built.parts_with_domain(&edit.full))
    }

    fn scope_domain_model(&self, edit: &UncitedEdit) -> Reach {
        if edit.delta.domain.intersects(&self.structural_domain) {
            return Reach::Parts(self.built.parts_with_setting_or_path(&edit.delta));
        }
        let by_delta_domain = self.built.parts_with_domain(&edit.delta);
        if !by_delta_domain.is_empty() {
            return Reach::Parts(by_delta_domain);
        }
        Reach::parts_or_all(self.built.parts_with_domain(&edit.full))
    }
}

struct BuiltTree {
    crate_dirs: BTreeSet<String>,
    crates: Vec<BuiltCrate>,
}

impl BuiltTree {
    fn read(tree: &Tree, parts: &[PartReach]) -> Self {
        let crate_dirs: BTreeSet<String> = tree.stem_crates().map(str::to_owned).collect();
        let crates =
            crate_dirs.iter().map(|dir| BuiltCrate::read(tree, dir, &crate_dirs, parts)).collect();
        Self { crate_dirs, crates }
    }

    const fn crate_dirs(&self) -> &BTreeSet<String> {
        &self.crate_dirs
    }

    fn vanished_domain(&self) -> DomainTerms {
        let mut domain = DomainTerms::default();
        for built in &self.crates {
            if built.owning_parts.is_empty() {
                domain.extend(built.domain.clone());
            }
        }
        domain
    }

    fn crate_cites(&self, part_id: &str, citations: &BTreeSet<String>) -> bool {
        self.crates.iter().any(|built| {
            built.owning_parts.contains(part_id) && !built.requirements.is_disjoint(citations)
        })
    }

    fn parts_with_setting_or_path(&self, prose: &ProseMentions) -> BTreeSet<String> {
        if !prose.has_setting_or_path() {
            return BTreeSet::new();
        }
        self.crates
            .iter()
            .filter(|built| built.mentions_setting_or_path(prose))
            .flat_map(|built| built.owning_parts.iter().cloned())
            .collect()
    }

    fn parts_with_domain(&self, prose: &ProseMentions) -> BTreeSet<String> {
        if !prose.has_any() {
            return BTreeSet::new();
        }
        self.crates
            .iter()
            .filter(|built| built.mentions_domain(prose))
            .flat_map(|built| built.owning_parts.iter().cloned())
            .collect()
    }
}

struct BuiltCrate {
    owning_parts: BTreeSet<String>,
    requirements: BTreeSet<String>,
    settings: BTreeSet<String>,
    paths: BTreeSet<UrlPath>,
    domain: DomainTerms,
}

impl BuiltCrate {
    fn read(
        tree: &Tree, dir: &str, crate_dirs: &BTreeSet<String>, plan_parts: &[PartReach],
    ) -> Self {
        let mut requirements = BTreeSet::new();
        let mut settings = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut domain = DomainTerms::default();
        domain.insert_identifier(layout::stem_of(dir));

        for module in tree.stem_files(dir) {
            requirements.extend(cited(&module.text));
            for ident in module.mentions().idents() {
                if Settings::is_key(ident) {
                    settings.insert(ident.to_owned());
                }
            }
            if let Some(file) = module.file() {
                for declared_type in syntax::Declared::of(file).types {
                    if !is_scaffold_helper_type(&declared_type) {
                        domain.insert_identifier(&declared_type);
                    }
                }
                for declared in syntax::setting_keys(file) {
                    if Settings::is_key(&declared.key) {
                        settings.insert(declared.key);
                    }
                }
                for literal in syntax::string_literals(file) {
                    settings.extend(setting_tokens(&literal));
                    if let Some(path) = UrlPath::from_literal(&literal) {
                        paths.insert(path);
                    }
                }
            }
        }

        let owning_parts = owning_parts(dir, &requirements, crate_dirs, plan_parts);
        Self {
            owning_parts,
            requirements,
            settings,
            paths,
            domain,
        }
    }

    fn mentions_setting_or_path(&self, prose: &ProseMentions) -> bool {
        !self.settings.is_disjoint(&prose.settings)
            || prose
                .paths
                .iter()
                .any(|prefix| self.paths.iter().any(|held| held.starts_with(prefix)))
    }

    fn mentions_domain(&self, prose: &ProseMentions) -> bool {
        self.mentions_setting_or_path(prose) || self.domain.intersects(&prose.domain)
    }
}

fn is_scaffold_helper_type(name: &str) -> bool {
    name == "Requires"
        || name.ends_with("Input")
        || name.ends_with("Output")
        || name.ends_with("Settings")
        || name.ends_with("Error")
}

fn owning_parts(
    dir: &str, requirements: &BTreeSet<String>, crate_dirs: &BTreeSet<String>, parts: &[PartReach],
) -> BTreeSet<String> {
    let by_requirement: BTreeSet<String> = parts
        .iter()
        .filter(|part| part.requirements.iter().any(|req| requirements.contains(req)))
        .map(|part| part.id.clone())
        .collect();
    if !by_requirement.is_empty() {
        return by_requirement;
    }

    let by_dir: BTreeSet<String> = parts
        .iter()
        .filter(|part| layout::crate_dir(&part.name) == dir)
        .map(|part| part.id.clone())
        .collect();
    if !by_dir.is_empty() {
        return by_dir;
    }

    let stem = layout::stem_of(dir);
    parts
        .iter()
        .filter(|part| {
            !crate_dirs.contains(layout::crate_dir(&part.name))
                && part.name.strip_prefix(stem).is_some_and(|rest| rest.starts_with('-'))
        })
        .map(|part| part.id.clone())
        .collect()
}

// Prose naming types or stems added or retired structurally should not widen carried crates.
fn structural_domain_terms(change: &Change, built: &BuiltTree) -> DomainTerms {
    let crate_dirs = built.crate_dirs();
    let mut terms = built.vanished_domain();
    for type_change in &change.types {
        let key = DesignKey::parse(&type_change.key);
        terms.insert_identifier(key.name);
        if let Some(stem) = key.stem {
            terms.insert_identifier(stem);
        }
    }
    for part in &change.parts {
        if !part.reasons.is_empty() && !crate_dirs.contains(layout::crate_dir(&part.name)) {
            terms.insert_identifier(&part.name);
            terms.insert_identifier(layout::stem_of(layout::crate_dir(&part.name)));
        }
    }
    for departure in &change.retired {
        insert_retired_stem(&mut terms, departure.requirement.stem(), &change.parts);
        if let Some(stem) = trigger::stem(&departure.requirement) {
            insert_retired_stem(&mut terms, &stem, &change.parts);
        }
    }
    terms
}

fn insert_retired_stem(terms: &mut DomainTerms, stem: &str, parts: &[PartReach]) {
    if layout::is_start_stem(stem) {
        return;
    }
    let kept_untouched = parts.iter().any(|part| {
        part.reasons.is_empty() && layout::stem_of(layout::crate_dir(&part.name)) == stem
    });
    if !kept_untouched {
        terms.insert_identifier(stem);
    }
}
