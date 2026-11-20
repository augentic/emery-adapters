//! Diffs changed design sections into paragraph and clause edits.

use std::collections::{BTreeSet, VecDeque};

use emery_sdk::target::SectionChange;

use super::mentions::ProseMentions;
use crate::scaffold::blocks;

const REQ_PREFIX: &str = "REQ-";
const MAX_RANGE_SPAN: u32 = 500;
const HEADING_SEPARATORS: [&str; 4] = [" — ", " – ", " - ", ": "];
const RANGE_CONNECTORS: [&str; 7] = ["through", "to", "–", "—", "-", "..=", ".."];
const ABBREVIATIONS: [&str; 5] = ["e.g", "i.e", "vs", "cf", "approx"];

pub enum ProseEdit {
    Cited(CitedEdit),
    Uncited(UncitedEdit),
}

pub struct CitedEdit {
    pub citations: BTreeSet<String>,
    pub prose: UncitedEdit,
}

impl CitedEdit {
    fn single(citations: BTreeSet<String>, text: &str) -> Self {
        Self {
            citations,
            prose: UncitedEdit::single(text),
        }
    }

    fn modified(citations: BTreeSet<String>, before: &str, after: &str) -> Self {
        Self {
            citations,
            prose: UncitedEdit::modified(before, after),
        }
    }
}

pub struct UncitedEdit {
    pub full: ProseMentions,
    pub delta: ProseMentions,
}

impl UncitedEdit {
    fn single(text: &str) -> Self {
        let mentions = ProseMentions::parse(text);
        Self {
            delta: mentions.clone(),
            full: mentions,
        }
    }

    fn modified(before: &str, after: &str) -> Self {
        let before = ProseMentions::parse(before);
        let after = ProseMentions::parse(after);
        let delta = before.symmetric_difference(&after);
        let full = before.into_union(after);
        Self { full, delta }
    }
}

pub struct SectionDiff {
    retired: BTreeSet<String>,
    edits: Vec<ProseEdit>,
}

impl SectionDiff {
    pub fn edits(section: &SectionChange, retired: &BTreeSet<String>) -> Vec<ProseEdit> {
        let before = trimmed_blocks(section.before.as_deref());
        let after = trimmed_blocks(section.after.as_deref());
        let mut diff = Self {
            retired: retired.clone(),
            edits: Vec::new(),
        };
        for hunk in DiffHunk::between(&before, &after) {
            diff.diff_paragraph_hunk(hunk);
        }
        diff.edits
    }

    fn diff_paragraph_hunk(&mut self, hunk: DiffHunk) {
        let removed =
            hunk.removed.into_iter().map(|raw| Paragraph::parse(raw, &self.retired)).collect();
        let added =
            hunk.added.into_iter().map(|raw| Paragraph::parse(raw, &self.retired)).collect();
        for step in pair_items(removed, added, Paragraph::corresponds_to) {
            match step {
                HunkStep::Unpaired { removed, added } => {
                    self.flush_unpaired_paragraphs(removed, added);
                }
                HunkStep::Paired { before, after } => self.diff_paragraph(&before, &after),
            }
        }
    }

    fn flush_unpaired_paragraphs(&mut self, removed: Vec<Paragraph>, added: Vec<Paragraph>) {
        let uncited_removed = self.partition_unpaired(removed, Paragraph::classify_unpaired);
        let uncited_added = self.partition_unpaired(added, Paragraph::classify_unpaired);
        if uncited_removed.len() == uncited_added.len() {
            for (before, after) in uncited_removed.into_iter().zip(uncited_added) {
                self.diff_paragraph(&before, &after);
            }
        } else {
            for paragraph in uncited_removed.into_iter().chain(uncited_added) {
                self.edits.push(ProseEdit::Uncited(UncitedEdit::single(&paragraph.raw)));
            }
        }
    }

    fn diff_paragraph(&mut self, before: &Paragraph, after: &Paragraph) {
        let (before_body, after_body) = before.bodies_against(after);
        let before_clauses = split_clauses(&before_body);
        let after_clauses = split_clauses(&after_body);
        if before_clauses.len() <= 1 && after_clauses.len() <= 1 {
            let left = before_clauses.into_iter().next().unwrap_or(before_body);
            let right = after_clauses.into_iter().next().unwrap_or(after_body);
            let before_clause = Clause::parse(left, &self.retired);
            let after_clause = Clause::parse(right, &self.retired);
            self.push_clause_pair(&before_clause, &after_clause);
            return;
        }

        for hunk in DiffHunk::between(&before_clauses, &after_clauses) {
            self.diff_clause_hunk(hunk);
        }
    }

    fn diff_clause_hunk(&mut self, hunk: DiffHunk) {
        let removed =
            hunk.removed.into_iter().map(|raw| Clause::parse(raw, &self.retired)).collect();
        let added = hunk.added.into_iter().map(|raw| Clause::parse(raw, &self.retired)).collect();
        for step in pair_items(removed, added, Clause::corresponds_to) {
            match step {
                HunkStep::Unpaired { removed, added } => {
                    self.flush_unpaired_clauses(removed, added);
                }
                HunkStep::Paired { before, after } => self.push_clause_pair(&before, &after),
            }
        }
    }

    fn flush_unpaired_clauses(&mut self, removed: Vec<Clause>, added: Vec<Clause>) {
        let uncited_removed = self.partition_unpaired(removed, Clause::classify_unpaired);
        let uncited_added = self.partition_unpaired(added, Clause::classify_unpaired);
        if uncited_removed.len() == uncited_added.len() {
            for (before, after) in uncited_removed.into_iter().zip(uncited_added) {
                self.edits.push(ProseEdit::Uncited(UncitedEdit::modified(&before.raw, &after.raw)));
            }
        } else {
            for clause in uncited_removed.into_iter().chain(uncited_added) {
                self.edits.push(ProseEdit::Uncited(UncitedEdit::single(&clause.raw)));
            }
        }
    }

    fn partition_unpaired<T>(&mut self, items: Vec<T>, classify: fn(&T) -> UnpairedKind) -> Vec<T> {
        let mut uncited = Vec::new();
        for item in items {
            match classify(&item) {
                UnpairedKind::Cited(edit) => self.edits.push(ProseEdit::Cited(edit)),
                UnpairedKind::RetiredOnly => {}
                UnpairedKind::Uncited => uncited.push(item),
            }
        }
        uncited
    }

    fn push_clause_pair(&mut self, before: &Clause, after: &Clause) {
        if !before.citations.has_any && !after.citations.has_any {
            self.edits.push(ProseEdit::Uncited(UncitedEdit::modified(&before.raw, &after.raw)));
            return;
        }

        if before.prose_without_citations == after.prose_without_citations {
            let changed: BTreeSet<String> = before
                .citations
                .live
                .symmetric_difference(&after.citations.live)
                .cloned()
                .collect();
            if !changed.is_empty() {
                self.edits.push(ProseEdit::Cited(CitedEdit::modified(
                    changed,
                    &before.raw,
                    &after.raw,
                )));
            }
            return;
        }

        let live: BTreeSet<String> =
            before.citations.live.union(&after.citations.live).cloned().collect();
        if !live.is_empty() {
            self.edits.push(ProseEdit::Cited(CitedEdit::modified(live, &before.raw, &after.raw)));
        } else if !after.citations.has_any && !after.raw.is_empty() {
            self.edits.push(ProseEdit::Uncited(UncitedEdit::single(&after.raw)));
        }
    }
}

struct Citations {
    live: BTreeSet<String>,
    has_any: bool,
}

enum UnpairedKind {
    Cited(CitedEdit),
    RetiredOnly,
    Uncited,
}

impl Citations {
    fn parse(text: &str, retired: &BTreeSet<String>) -> Self {
        let all = citations_in(text);
        let has_any = !all.is_empty();
        let live = all.into_iter().filter(|id| !retired.contains(id)).collect();
        Self { live, has_any }
    }

    fn shares_live_with(&self, other: &Self) -> bool {
        !self.live.is_disjoint(&other.live)
    }

    fn classify_unpaired(&self, text: &str) -> UnpairedKind {
        if !self.has_any {
            UnpairedKind::Uncited
        } else if self.live.is_empty() {
            UnpairedKind::RetiredOnly
        } else {
            UnpairedKind::Cited(CitedEdit::single(self.live.clone(), text))
        }
    }
}

struct Paragraph {
    raw: String,
    heading: Option<String>,
    body: String,
    citations: Citations,
}

impl Paragraph {
    fn parse(raw: String, retired: &BTreeSet<String>) -> Self {
        let citations = Citations::parse(&raw, retired);
        let (heading, body) = match split_heading(&raw) {
            Some((head, rest)) => (Some(head), rest),
            None => (None, raw.clone()),
        };
        Self {
            raw,
            heading,
            body,
            citations,
        }
    }

    fn classify_unpaired(&self) -> UnpairedKind {
        self.citations.classify_unpaired(&self.raw)
    }

    fn corresponds_to(&self, other: &Self) -> bool {
        if self.shares_heading_with(other) || self.citations.shares_live_with(&other.citations) {
            return true;
        }
        let (left, right) = self.bodies_against(other);
        let left_clauses: BTreeSet<String> = split_clauses(&left).into_iter().collect();
        let right_clauses: BTreeSet<String> = split_clauses(&right).into_iter().collect();
        !left_clauses.is_disjoint(&right_clauses)
    }

    fn shares_heading_with(&self, other: &Self) -> bool {
        self.heading.is_some() && self.heading == other.heading
    }

    fn bodies_against(&self, other: &Self) -> (String, String) {
        if self.shares_heading_with(other) {
            (self.body.clone(), other.body.clone())
        } else {
            (self.raw.clone(), other.raw.clone())
        }
    }
}

struct Clause {
    raw: String,
    prose_without_citations: String,
    citations: Citations,
}

impl Clause {
    fn parse(raw: String, retired: &BTreeSet<String>) -> Self {
        let citations = Citations::parse(&raw, retired);
        let prose_without_citations = strip_citations(&raw);
        Self {
            raw,
            prose_without_citations,
            citations,
        }
    }

    fn classify_unpaired(&self) -> UnpairedKind {
        self.citations.classify_unpaired(&self.raw)
    }

    fn corresponds_to(&self, other: &Self) -> bool {
        self.citations.shares_live_with(&other.citations)
            || self.prose_without_citations == other.prose_without_citations
    }
}

struct DiffHunk {
    removed: Vec<String>,
    added: Vec<String>,
}

impl DiffHunk {
    fn between(before: &[String], after: &[String]) -> Vec<Self> {
        let before_set: BTreeSet<String> = before.iter().cloned().collect();
        let after_set: BTreeSet<String> = after.iter().cloned().collect();
        let lcs = lcs_table(before, after);

        let mut hunks = Vec::new();
        let mut removed = Vec::new();
        let mut added = Vec::new();
        let (mut i, mut j) = (0, 0);
        while i < before.len() && j < after.len() {
            if before[i] == after[j] {
                Self::push_filtered(&mut hunks, &mut removed, &mut added, &before_set, &after_set);
                i += 1;
                j += 1;
            } else if lcs[i + 1][j] >= lcs[i][j + 1] {
                removed.push(before[i].clone());
                i += 1;
            } else {
                added.push(after[j].clone());
                j += 1;
            }
        }
        removed.extend_from_slice(&before[i..]);
        added.extend_from_slice(&after[j..]);
        Self::push_filtered(&mut hunks, &mut removed, &mut added, &before_set, &after_set);
        hunks
    }

    fn push_filtered(
        hunks: &mut Vec<Self>, removed: &mut Vec<String>, added: &mut Vec<String>,
        before_set: &BTreeSet<String>, after_set: &BTreeSet<String>,
    ) {
        let removed: Vec<String> =
            removed.drain(..).filter(|item| !after_set.contains(item)).collect();
        let added: Vec<String> =
            added.drain(..).filter(|item| !before_set.contains(item)).collect();
        if !removed.is_empty() || !added.is_empty() {
            hunks.push(Self { removed, added });
        }
    }
}

enum HunkStep<T> {
    Unpaired { removed: Vec<T>, added: Vec<T> },
    Paired { before: T, after: T },
}

fn pair_items<T>(
    removed: Vec<T>, added: Vec<T>, mut corresponds: impl FnMut(&T, &T) -> bool,
) -> Vec<HunkStep<T>> {
    let mut steps = Vec::new();
    let mut rem_gap = Vec::new();
    let mut remaining: VecDeque<T> = added.into();
    for rem_item in removed {
        if let Some(pos) = remaining.iter().position(|add_item| corresponds(&rem_item, add_item)) {
            let add_gap: Vec<T> = remaining.drain(..pos).collect();
            if let Some(add_item) = remaining.pop_front() {
                if !rem_gap.is_empty() || !add_gap.is_empty() {
                    steps.push(HunkStep::Unpaired {
                        removed: std::mem::take(&mut rem_gap),
                        added: add_gap,
                    });
                }
                steps.push(HunkStep::Paired {
                    before: rem_item,
                    after: add_item,
                });
            }
        } else {
            rem_gap.push(rem_item);
        }
    }
    if !rem_gap.is_empty() || !remaining.is_empty() {
        steps.push(HunkStep::Unpaired {
            removed: rem_gap,
            added: remaining.into(),
        });
    }
    steps
}

fn split_heading(text: &str) -> Option<(String, String)> {
    let rest = text.strip_prefix("**")?;
    let mut in_code = false;
    for (at, ch) in rest.char_indices() {
        if ch == '`' {
            in_code = !in_code;
        } else if !in_code && rest[at..].starts_with("**") {
            let heading = &rest[..at];
            let after = &rest[at + 2..];
            return HEADING_SEPARATORS.iter().find_map(|sep| {
                after
                    .strip_prefix(sep)
                    .map(|body| (heading.to_owned(), body.trim_start().to_owned()))
            });
        }
    }
    None
}

fn split_clauses(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_code = false;
    let mut depth: usize = 0;
    let mut start = 0;
    for (at, ch) in text.char_indices() {
        match ch {
            '`' => in_code = !in_code,
            '(' | '[' | '{' if !in_code => depth += 1,
            ')' | ']' | '}' if !in_code => depth = depth.saturating_sub(1),
            '.' | ';'
                if !in_code
                    && depth == 0
                    && text[at + ch.len_utf8()..]
                        .chars()
                        .next()
                        .is_some_and(char::is_whitespace)
                    && (ch != '.' || !ends_with_non_sentence_dot(&text[start..at])) =>
            {
                if let Some(clause) = clean_clause(&text[start..at]) {
                    out.push(clause);
                }
                start = at + ch.len_utf8();
            }
            _ => {}
        }
    }
    if let Some(clause) = clean_clause(&text[start..]) {
        out.push(clause);
    }
    out
}

fn ends_with_non_sentence_dot(clause_prefix: &str) -> bool {
    let trimmed = clause_prefix.trim();
    if !trimmed.is_empty() && trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    ABBREVIATIONS.iter().any(|abbr| {
        lower.strip_suffix(abbr).is_some_and(|before| {
            before.is_empty() || !before.ends_with(|ch: char| ch.is_ascii_alphabetic())
        })
    })
}

fn clean_clause(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches(['.', ';']).trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

struct CitationSpan {
    ids: Vec<String>,
    consumed: usize,
}

impl CitationSpan {
    fn parse_after_prefix(after_prefix: &str) -> Option<Self> {
        let (first, after) = leading_digits(after_prefix);
        if first.is_empty() {
            return None;
        }
        let mut ids = vec![format!("{REQ_PREFIX}{first}")];
        let mut consumed = first.len();
        if let Some(after_range) = strip_range_connector(after) {
            let (last, tail) = leading_digits(after_range);
            if let (Ok(from), Ok(to)) = (first.parse::<u32>(), last.parse::<u32>())
                && from <= to
                && to - from <= MAX_RANGE_SPAN
            {
                ids.extend(
                    (from + 1..=to)
                        .map(|number| format!("{REQ_PREFIX}{number:0width$}", width = first.len())),
                );
                consumed = after_prefix.len() - tail.len();
            }
        }
        Some(Self { ids, consumed })
    }
}

fn citations_in(text: &str) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    let mut rest = text;
    while let Some(at) = rest.find(REQ_PREFIX) {
        let after_prefix = &rest[at + REQ_PREFIX.len()..];
        if let Some(span) = CitationSpan::parse_after_prefix(after_prefix) {
            ids.extend(span.ids);
            rest = &after_prefix[span.consumed..];
        } else {
            rest = after_prefix;
        }
    }
    ids
}

// Clauses that only drop retired citations or reformat ranges compare equal.
fn strip_citations(text: &str) -> String {
    let mut without_ids = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(REQ_PREFIX) {
        without_ids.push_str(&rest[..at]);
        let after_prefix = &rest[at + REQ_PREFIX.len()..];
        if let Some(span) = CitationSpan::parse_after_prefix(after_prefix) {
            rest = &after_prefix[span.consumed..];
        } else {
            without_ids.push_str(REQ_PREFIX);
            rest = after_prefix;
        }
    }
    without_ids.push_str(rest);

    let without_empty_parens = strip_empty_citation_parens(&without_ids);
    normalize_spacing(&without_empty_parens)
}

fn strip_empty_citation_parens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut cursor = text;
    while let Some(open) = cursor.find('(') {
        out.push_str(&cursor[..open]);
        let inner_and_rest = &cursor[open + 1..];
        if let Some(close) = inner_and_rest.find(')')
            && !inner_and_rest[..close].contains('(')
        {
            let kept: Vec<&str> = inner_and_rest[..close]
                .split([',', ';'])
                .map(str::trim)
                .filter(|piece| !piece.is_empty())
                .collect();
            if !kept.is_empty() {
                out.push('(');
                out.push_str(&kept.join(", "));
                out.push(')');
            }
            cursor = &inner_and_rest[close + 1..];
        } else {
            out.push('(');
            cursor = inner_and_rest;
        }
    }
    out.push_str(cursor);
    out
}

fn normalize_spacing(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !out.is_empty() && !matches!(word.as_bytes().first(), Some(b',' | b';' | b'.')) {
            out.push(' ');
        }
        out.push_str(word);
    }
    out
}

fn strip_range_connector(after: &str) -> Option<&str> {
    let trimmed = after.trim_start_matches(' ');
    RANGE_CONNECTORS.iter().find_map(|connector| {
        trimmed.strip_prefix(connector)?.trim_start_matches(' ').strip_prefix(REQ_PREFIX)
    })
}

fn leading_digits(text: &str) -> (&str, &str) {
    let digits_len = text.bytes().take_while(u8::is_ascii_digit).count();
    text.split_at(digits_len)
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
