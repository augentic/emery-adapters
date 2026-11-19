//! Names the plan parts a change reaches through its changed design sections.

use std::collections::BTreeSet;

use emery_sdk::target::{Change, PartReach, SectionChange};
use emery_sdk::words;

use crate::scaffold::{DesignKey, Settings, blocks, cited};
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

    if change
        .sections
        .iter()
        .all(|section| SectionScope::of(&section.kind) == SectionScope::Ignored)
    {
        return ordered_parts(&change.parts, &reached);
    }

    let retired: BTreeSet<String> =
        change.retired.iter().map(|departure| departure.requirement.id.clone()).collect();
    let built = BuiltTree::read(tree, &change.parts);
    let accounted_domain = DomainTerms::accounted_for_by(change, built.crate_dirs());

    for section in &change.sections {
        let rule = SectionScope::of(&section.kind);
        if rule == SectionScope::Ignored {
            continue;
        }
        for edit in SectionDiff::edits(section, &retired) {
            match edit {
                ParagraphEdit::Cited(ids) => {
                    for part in &change.parts {
                        if part.requirements.iter().any(|req| ids.contains(req)) {
                            reached.insert(part.id.clone());
                        }
                    }
                }
                ParagraphEdit::Uncited(uncited) => {
                    match built.scope_uncited(rule, &uncited, &accounted_domain) {
                        ScopeMatch::Parts(matched) => reached.extend(matched),
                        ScopeMatch::AllParts => return all_parts(&change.parts),
                    }
                }
            }
        }
    }

    ordered_parts(&change.parts, &reached)
}

fn ordered_parts(parts: &[PartReach], reached: &BTreeSet<String>) -> Vec<String> {
    parts.iter().filter(|part| reached.contains(&part.id)).map(|part| part.id.clone()).collect()
}

fn all_parts(parts: &[PartReach]) -> Vec<String> {
    parts.iter().map(|part| part.id.clone()).collect()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SectionScope {
    Ignored,
    Mentions,
    DomainModel,
    AllParts,
}

impl SectionScope {
    fn of(kind: &str) -> Self {
        match kind {
            "overview" | "ui-layout" => Self::Ignored,
            "apis" | "technical-logic" | "observability" => Self::Mentions,
            "domain-model" => Self::DomainModel,
            _ => Self::AllParts,
        }
    }
}

enum ScopeMatch {
    Parts(BTreeSet<String>),
    AllParts,
}

struct BuiltTree {
    crate_dirs: BTreeSet<String>,
    crates: Vec<BuiltCrate>,
}

impl BuiltTree {
    fn read(tree: &Tree, parts: &[PartReach]) -> Self {
        let dirs: Vec<String> = tree.stem_crates().map(str::to_owned).collect();
        let crate_dirs: BTreeSet<String> = dirs.iter().cloned().collect();
        let crates =
            dirs.into_iter().map(|dir| BuiltCrate::read(tree, &dir, &crate_dirs, parts)).collect();
        Self { crate_dirs, crates }
    }

    const fn crate_dirs(&self) -> &BTreeSet<String> {
        &self.crate_dirs
    }

    fn scope_uncited(
        &self, rule: SectionScope, edit: &UncitedEdit, accounted_domain: &DomainTerms,
    ) -> ScopeMatch {
        match rule {
            SectionScope::Mentions => {
                let matched = self.parts_with_setting_or_path(&edit.full);
                if matched.is_empty() { ScopeMatch::AllParts } else { ScopeMatch::Parts(matched) }
            }
            SectionScope::DomainModel => self.scope_domain_model(edit, accounted_domain),
            SectionScope::AllParts => ScopeMatch::AllParts,
            SectionScope::Ignored => ScopeMatch::Parts(BTreeSet::new()),
        }
    }

    fn scope_domain_model(&self, edit: &UncitedEdit, accounted_domain: &DomainTerms) -> ScopeMatch {
        if edit.delta.domain.intersects(accounted_domain) {
            return ScopeMatch::Parts(self.parts_with_setting_or_path(&edit.delta));
        }
        let matched = self.parts_with_domain(&edit.delta);
        if !matched.is_empty() {
            return ScopeMatch::Parts(matched);
        }
        let matched = self.parts_with_domain(&edit.full);
        if matched.is_empty() { ScopeMatch::AllParts } else { ScopeMatch::Parts(matched) }
    }

    fn parts_with_setting_or_path(&self, prose: &ProseMentions) -> BTreeSet<String> {
        if !prose.has_setting_or_path() {
            return BTreeSet::new();
        }
        self.crates
            .iter()
            .filter(|built| built.mentions_setting_or_path(prose))
            .flat_map(|built| built.parts.iter().cloned())
            .collect()
    }

    fn parts_with_domain(&self, prose: &ProseMentions) -> BTreeSet<String> {
        if !prose.has_any_mention() {
            return BTreeSet::new();
        }
        self.crates
            .iter()
            .filter(|built| built.mentions_domain(prose))
            .flat_map(|built| built.parts.iter().cloned())
            .collect()
    }
}

struct BuiltCrate {
    parts: Vec<String>,
    settings: BTreeSet<String>,
    paths: BTreeSet<UrlPath>,
    domain: DomainTerms,
}

impl BuiltCrate {
    // Associates a stem crate at the built head with its settings, `/`-paths,
    // declared domain types, and the plan parts whose requirements its comments
    // cite, falling back to the part's stem name.
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

        Self {
            parts: owning_parts(dir, &requirements, crate_dirs, plan_parts),
            settings,
            paths,
            domain,
        }
    }

    fn mentions_setting_or_path(&self, prose: &ProseMentions) -> bool {
        prose.settings.iter().any(|key| self.settings.contains(key))
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
) -> Vec<String> {
    let by_requirement: Vec<String> = parts
        .iter()
        .filter(|part| part.requirements.iter().any(|req| requirements.contains(req)))
        .map(|part| part.id.clone())
        .collect();
    if !by_requirement.is_empty() {
        return by_requirement;
    }

    let by_dir: Vec<String> = parts
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

enum ParagraphEdit {
    Cited(Vec<String>),
    Uncited(UncitedEdit),
}

struct UncitedEdit {
    full: ProseMentions,
    delta: ProseMentions,
}

impl UncitedEdit {
    fn single(paragraph: &str) -> Self {
        let mentions = ProseMentions::from_paragraph(paragraph);
        Self {
            delta: mentions.clone(),
            full: mentions,
        }
    }

    fn modified(before: &str, after: &str) -> Self {
        let before = ProseMentions::from_paragraph(before);
        let after = ProseMentions::from_paragraph(after);
        let delta = before.symmetric_difference(&after);
        let full = before.into_union(after);
        Self { full, delta }
    }
}

#[derive(Clone)]
struct ProseMentions {
    settings: BTreeSet<String>,
    paths: BTreeSet<UrlPath>,
    domain: DomainTerms,
}

impl ProseMentions {
    fn from_paragraph(text: &str) -> Self {
        Self {
            settings: setting_tokens(text).collect(),
            paths: UrlPath::in_prose(text),
            domain: DomainTerms::from_prose(text),
        }
    }

    fn symmetric_difference(&self, other: &Self) -> Self {
        Self {
            settings: self.settings.symmetric_difference(&other.settings).cloned().collect(),
            paths: self.paths.symmetric_difference(&other.paths).cloned().collect(),
            domain: self.domain.symmetric_difference(&other.domain),
        }
    }

    fn into_union(mut self, other: Self) -> Self {
        self.settings.extend(other.settings);
        self.paths.extend(other.paths);
        self.domain.extend(other.domain);
        self
    }

    fn has_setting_or_path(&self) -> bool {
        !self.settings.is_empty() || !self.paths.is_empty()
    }

    fn has_any_mention(&self) -> bool {
        self.has_setting_or_path() || !self.domain.is_empty()
    }
}

#[derive(Clone, Default)]
struct DomainTerms(BTreeSet<String>);

impl DomainTerms {
    const MIN_LEN: usize = 3;
    const MIN_PLURAL_LEN: usize = 4;

    // Terms already accounted for by added/changed/removed `Type:` blocks,
    // newly added stems, or retired stems, so narrative `domain-model` edits
    // introducing them do not widen carried crates they merely link to.
    fn accounted_for_by(change: &Change, crate_dirs: &BTreeSet<String>) -> Self {
        let mut terms = Self::default();
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
            let stem = departure.requirement.stem();
            let kept_untouched = change.parts.iter().any(|part| {
                part.reasons.is_empty() && layout::stem_of(layout::crate_dir(&part.name)) == stem
            });
            if !kept_untouched {
                terms.insert_identifier(stem);
            }
        }
        terms
    }

    fn from_prose(text: &str) -> Self {
        let seq: Vec<String> = text
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
        let mut terms: BTreeSet<String> =
            seq.iter().filter(|word| word.len() >= Self::MIN_LEN).cloned().collect();
        terms.extend(
            seq.windows(2)
                .filter(|pair| pair[0].len() >= Self::MIN_LEN || pair[1].len() >= Self::MIN_LEN)
                .map(|pair| format!("{}_{}", pair[0], pair[1])),
        );
        Self(terms)
    }

    fn symmetric_difference(&self, other: &Self) -> Self {
        Self(self.0.symmetric_difference(&other.0).cloned().collect())
    }

    fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    fn insert_identifier(&mut self, raw: &str) {
        let segments = words(raw);
        let Some((last, head)) = segments.split_last() else {
            return;
        };
        for tail in Self::number_forms(last) {
            let plain = format!("{}{tail}", head.concat());
            if plain.len() >= Self::MIN_LEN {
                self.0.insert(plain);
            }
            let snake = if head.is_empty() { tail } else { format!("{}_{tail}", head.join("_")) };
            if snake.len() >= Self::MIN_LEN {
                self.0.insert(snake);
            }
        }
    }

    fn number_forms(word: &str) -> Vec<String> {
        let mut forms = vec![word.to_owned()];
        if word.len() >= Self::MIN_PLURAL_LEN
            && word.ends_with('s')
            && !word.ends_with("ss")
            && !word.ends_with("us")
            && !word.ends_with("is")
        {
            forms.push(word[..word.len() - 1].to_owned());
        } else if word.len() >= Self::MIN_LEN && !word.ends_with('s') {
            forms.push(format!("{word}s"));
        }
        forms
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn intersects(&self, other: &Self) -> bool {
        !self.is_empty() && !other.is_empty() && self.0.iter().any(|term| other.0.contains(term))
    }
}

// Diffs a section's `before` and `after` paragraphs, ignoring paragraphs that
// stand verbatim on both sides even if reordered, and separating cited
// paragraphs from uncited ones within each hunk.
struct SectionDiff {
    before_set: BTreeSet<String>,
    after_set: BTreeSet<String>,
    retired: BTreeSet<String>,
    removed: Vec<String>,
    added: Vec<String>,
    edits: Vec<ParagraphEdit>,
}

impl SectionDiff {
    fn edits(section: &SectionChange, retired: &BTreeSet<String>) -> Vec<ParagraphEdit> {
        let before = trimmed_blocks(section.before.as_deref());
        let after = trimmed_blocks(section.after.as_deref());
        let lcs = lcs_table(&before, &after);

        let mut diff = Self {
            before_set: before.iter().cloned().collect(),
            after_set: after.iter().cloned().collect(),
            retired: retired.clone(),
            removed: Vec::new(),
            added: Vec::new(),
            edits: Vec::new(),
        };

        let (mut i, mut j) = (0, 0);
        while i < before.len() && j < after.len() {
            if before[i] == after[j] {
                diff.flush();
                i += 1;
                j += 1;
            } else if lcs[i + 1][j] >= lcs[i][j + 1] {
                diff.removed.push(before[i].clone());
                i += 1;
            } else {
                diff.added.push(after[j].clone());
                j += 1;
            }
        }
        diff.removed.extend_from_slice(&before[i..]);
        diff.added.extend_from_slice(&after[j..]);
        diff.flush();
        diff.edits
    }

    fn flush(&mut self) {
        let mut uncited_removed = Vec::new();
        for paragraph in self.removed.drain(..) {
            if self.after_set.contains(&paragraph) {
                continue;
            }
            let ids = cited(&paragraph);
            if ids.is_empty() {
                uncited_removed.push(paragraph);
            } else if !ids.iter().all(|id| self.retired.contains(id)) {
                self.edits.push(ParagraphEdit::Cited(ids));
            }
        }

        let mut uncited_added = Vec::new();
        for paragraph in self.added.drain(..) {
            if self.before_set.contains(&paragraph) {
                continue;
            }
            let ids = cited(&paragraph);
            if ids.is_empty() {
                uncited_added.push(paragraph);
            } else {
                self.edits.push(ParagraphEdit::Cited(ids));
            }
        }

        if uncited_removed.len() == uncited_added.len() {
            for (before, after) in uncited_removed.into_iter().zip(uncited_added) {
                self.edits.push(ParagraphEdit::Uncited(UncitedEdit::modified(&before, &after)));
            }
        } else {
            for paragraph in uncited_removed.into_iter().chain(uncited_added) {
                self.edits.push(ParagraphEdit::Uncited(UncitedEdit::single(&paragraph)));
            }
        }
    }
}

fn trimmed_blocks(text: Option<&str>) -> Vec<String> {
    text.map_or_else(Vec::new, |text| {
        blocks(text)
            .into_iter()
            .map(str::trim)
            .filter(|block| !block.is_empty())
            .map(str::to_owned)
            .collect()
    })
}

fn lcs_table(before: &[String], after: &[String]) -> Vec<Vec<usize>> {
    let (n, m) = (before.len(), after.len());
    let mut table = vec![vec![0_usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if before[i] == after[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    table
}

fn setting_tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|word| Settings::is_key(word))
        .map(str::to_owned)
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Segment {
    Literal(String),
    Param,
}

impl Segment {
    fn parse(raw: &str) -> Self {
        if (raw.starts_with('{') && raw.ends_with('}')) || raw.starts_with(':') {
            Self::Param
        } else {
            Self::Literal(raw.to_owned())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct UrlPath {
    segments: Vec<Segment>,
}

impl UrlPath {
    fn in_prose(text: &str) -> BTreeSet<Self> {
        text.split_whitespace().filter_map(Self::from_design_token).collect()
    }

    fn from_design_token(token: &str) -> Option<Self> {
        let trimmed = token
            .trim_matches(|ch: char| {
                matches!(ch, '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>')
            })
            .trim_end_matches(['.', ',', ':', ';', '!', '?']);
        Self::from_url_or_path(trimmed)
    }

    fn from_literal(literal: &str) -> Option<Self> {
        Self::from_url_or_path(literal)
    }

    fn from_url_or_path(text: &str) -> Option<Self> {
        if let Some(after_scheme) =
            text.strip_prefix("https://").or_else(|| text.strip_prefix("http://"))
        {
            let slash = after_scheme.find('/')?;
            return Self::parse(&after_scheme[slash..]);
        }
        Self::parse(strip_leading_placeholder(text))
    }

    fn parse(candidate: &str) -> Option<Self> {
        let path = candidate.split(['?', '#']).next().unwrap_or(candidate);
        if !path.starts_with('/') || path.starts_with("//") {
            return None;
        }
        let trimmed = path.trim_matches('/');
        if trimmed.is_empty()
            || !trimmed.chars().all(|ch| {
                ch.is_ascii_alphanumeric()
                    || matches!(ch, '/' | '-' | '_' | '{' | '}' | ':' | '*' | '.')
            })
        {
            return None;
        }
        let segments: Vec<Segment> =
            trimmed.split('/').filter(|segment| !segment.is_empty()).map(Segment::parse).collect();
        (!segments.is_empty()).then_some(Self { segments })
    }

    fn starts_with(&self, prefix: &Self) -> bool {
        self.segments.starts_with(&prefix.segments)
    }
}

fn strip_leading_placeholder(text: &str) -> &str {
    let opened = text.strip_prefix("${").or_else(|| text.strip_prefix('{'));
    if let Some((inner, rest)) = opened.and_then(|rest| rest.split_once('}'))
        && !inner.contains('/')
        && rest.starts_with('/')
    {
        return rest;
    }
    text
}
