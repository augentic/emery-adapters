//! Names the plan parts a change reaches through its changed design sections.

use std::collections::BTreeSet;

use emery_sdk::push_unique;
use emery_sdk::target::{Change, PartReach, SectionChange};

use crate::scaffold::{Settings, blocks, cited};
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
                ParagraphEdit::Uncited(mentions) => match rule {
                    SectionScope::Mentions => {
                        let matched = built.parts_mentioning(&mentions);
                        if matched.is_empty() {
                            return all_parts(&change.parts);
                        }
                        reached.extend(matched);
                    }
                    SectionScope::AllParts => return all_parts(&change.parts),
                    SectionScope::Ignored => {}
                },
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
    AllParts,
}

impl SectionScope {
    fn of(kind: &str) -> Self {
        match kind {
            "overview" | "ui-layout" => Self::Ignored,
            "apis" | "technical-logic" | "observability" => Self::Mentions,
            _ => Self::AllParts,
        }
    }
}

struct BuiltTree {
    crates: Vec<BuiltCrate>,
}

impl BuiltTree {
    fn read(tree: &Tree, parts: &[PartReach]) -> Self {
        let dirs: Vec<String> = tree.stem_crates().map(str::to_owned).collect();
        let existing: BTreeSet<String> = dirs.iter().cloned().collect();
        let crates =
            dirs.into_iter().map(|dir| BuiltCrate::read(tree, &dir, &existing, parts)).collect();
        Self { crates }
    }

    fn parts_mentioning(&self, prose: &ProseMentions) -> BTreeSet<String> {
        if prose.is_empty() {
            return BTreeSet::new();
        }
        self.crates
            .iter()
            .filter(|built| built.mentions(prose))
            .flat_map(|built| built.parts.iter().cloned())
            .collect()
    }
}

struct BuiltCrate {
    parts: Vec<String>,
    settings: BTreeSet<String>,
    paths: Vec<UrlPath>,
}

impl BuiltCrate {
    // Associates a stem crate at the built head with its settings, `/`-paths,
    // and the plan parts whose requirements its comments cite, falling back to
    // the part's stem name.
    fn read(tree: &Tree, dir: &str, existing: &BTreeSet<String>, plan_parts: &[PartReach]) -> Self {
        let mut requirements = BTreeSet::new();
        let mut settings = BTreeSet::new();
        let mut paths = Vec::new();

        for module in tree.stem_files(dir) {
            requirements.extend(cited(&module.text));
            for ident in module.mentions().idents() {
                if Settings::is_key(ident) {
                    settings.insert(ident.to_owned());
                }
            }
            if let Some(file) = module.file() {
                for declared in syntax::setting_keys(file) {
                    if Settings::is_key(&declared.key) {
                        settings.insert(declared.key);
                    }
                }
                for literal in syntax::string_literals(file) {
                    settings.extend(setting_tokens(&literal).map(str::to_owned));
                    if let Some(path) = UrlPath::from_literal(&literal) {
                        push_unique(&mut paths, path);
                    }
                }
            }
        }

        Self {
            parts: owning_parts(dir, &requirements, existing, plan_parts),
            settings,
            paths,
        }
    }

    fn mentions(&self, prose: &ProseMentions) -> bool {
        prose.settings.iter().any(|key| self.settings.contains(key))
            || prose
                .paths
                .iter()
                .any(|prefix| self.paths.iter().any(|held| held.starts_with(prefix)))
    }
}

fn owning_parts(
    dir: &str, requirements: &BTreeSet<String>, existing: &BTreeSet<String>, parts: &[PartReach],
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
            !existing.contains(layout::crate_dir(&part.name))
                && part.name.strip_prefix(stem).is_some_and(|rest| rest.starts_with('-'))
        })
        .map(|part| part.id.clone())
        .collect()
}

enum ParagraphEdit {
    Cited(Vec<String>),
    Uncited(ProseMentions),
}

struct ProseMentions {
    settings: BTreeSet<String>,
    paths: Vec<UrlPath>,
}

impl ProseMentions {
    fn from_paragraphs<I, S>(paragraphs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut settings = BTreeSet::new();
        let mut paths = Vec::new();
        for paragraph in paragraphs {
            let text = paragraph.as_ref();
            settings.extend(setting_tokens(text).map(str::to_owned));
            for token in text.split_whitespace() {
                if let Some(path) = UrlPath::from_design_token(token) {
                    push_unique(&mut paths, path);
                }
            }
        }
        Self { settings, paths }
    }

    fn is_empty(&self) -> bool {
        self.settings.is_empty() && self.paths.is_empty()
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
            for pair in uncited_removed.into_iter().zip(uncited_added) {
                self.edits
                    .push(ParagraphEdit::Uncited(ProseMentions::from_paragraphs(
                        <[String; 2]>::from(pair),
                    )));
            }
        } else {
            for paragraph in uncited_removed.into_iter().chain(uncited_added) {
                self.edits
                    .push(ParagraphEdit::Uncited(ProseMentions::from_paragraphs([paragraph])));
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

fn setting_tokens(text: &str) -> impl Iterator<Item = &str> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|word| Settings::is_key(word))
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct UrlPath {
    segments: Vec<Segment>,
}

impl UrlPath {
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
