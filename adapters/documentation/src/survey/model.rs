//! The one survey turn a documentation tree opens: the documents' outlines
//! laid for the model to name the tree's subjects from, the answer held to
//! the tree, and the span each subject covers derived from its anchor.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use emery_sdk::survey::{Facts, Inventory, Lines, Surface};
use emery_sdk::{Context, Error, Model, serde_json, tracing};

use super::structure::{self, Kind};
use super::subjects::Prepared;

// One subject the survey named, with what code derived from its anchor.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subject {
    pub name: String,
    // Root-relative path of the document it anchors in.
    pub entry: String,
    // Stem its `requirement` and `criterion` ids lead with.
    pub stem: String,
    // Stem alone for the one subject under it, else stem + disambiguating slug.
    pub lead: String,
    // 1-based line the model anchored it at.
    pub anchor: u32,
    // From its anchor to the line before the next subject in the same document
    // (or the document's end), with any preamble before the first folded in.
    pub span: Lines,
}

impl Prepared<'_> {
    // Runs the survey turn over the prepared documents and holds the answer to
    // the tree: every document is a subject's entry or listed `unreached`, and
    // no two subjects anchor at one line.
    pub async fn inventory<P: Model>(
        &self, ctx: &Context<'_, P>, docs: &'static [emery_sdk::Doc],
    ) -> Result<Inventory, Error> {
        let source = self.source();
        let modules: Vec<String> = self.documents.iter().map(|doc| doc.path.clone()).collect();
        let text = self.facts();

        // trace the facts for a host reading them without a turn
        if tracing::enabled!(tracing::Level::TRACE) {
            let json = serde_json::Value::String(text.clone()).to_string();
            tracing::trace!(%source, facts = %json, "survey facts");
        }
        let facts = Facts {
            modules: &modules,
            text: &text,
            files: &modules,
        };

        emery_sdk::survey::surfaces(ctx, docs, &facts, |answer| self.check(answer)).await
    }

    // The documents in path order, each with its outline: its length, each
    // heading at its line, and what its body holds. The message's own nouns are
    // the SDK's — module, surface — so the facts say first what each is here.
    fn facts(&self) -> String {
        let mut text = String::from(
            "This source is a documentation tree. In this message a module is one of its \
             documents and a surface is a subject — a feature, a resource, a flow the documents \
             describe — as the prompt explains. Every document, in path order, with its outline: \
             its length, each heading at its line, and what its body holds.\n",
        );
        for doc in &self.documents {
            let outline = &doc.outline;
            let holds: Vec<&str> = [
                (Kind::Item, "lists"),
                (Kind::Row, "tables"),
                (Kind::Step, "steps"),
                (Kind::Quote, "quotations"),
                (Kind::Fence, "code blocks"),
            ]
            .into_iter()
            .filter_map(|(kind, name)| outline.holds(kind).then_some(name))
            .collect();
            let holds = if holds.is_empty() { "prose alone".to_owned() } else { holds.join(", ") };
            let _ = write!(text, "\n- `{}` — {} lines; {holds}", doc.path, outline.lines);
            if outline.headings.is_empty() {
                match structure::opening_line(&doc.text) {
                    Some((line, opening)) => {
                        let _ = write!(
                            text,
                            "\n  - no heading; opens at L{line}: {}",
                            clipped(opening)
                        );
                    }
                    None => text.push_str("\n  - empty"),
                }
            }
            for heading in &outline.headings {
                let level = "#".repeat(usize::from(heading.level));
                let _ = write!(text, "\n  - L{} {level} {}", heading.line, clipped(&heading.text));
            }
        }
        text
    }

    // What the tree alone can hold the answer to, past the SDK's own checks:
    // every document is a subject's entry or listed unreached, and no two
    // subjects anchor at one line.
    fn check(&self, answer: &Inventory) -> Vec<String> {
        let mut findings = Vec::new();

        let mut entries = BTreeSet::new();
        let mut at: BTreeMap<(&str, u32), &str> = BTreeMap::new();
        for surface in &answer.surfaces {
            let Ok(anchor) = surface.anchor() else { continue };
            entries.insert(anchor.path);
            let start = start_line(anchor.lines);
            if let Some(other) = at.insert((anchor.path, start), &surface.name) {
                findings.push(format!(
                    "- subjects `{other}` and `{}` both anchor at `{}#L{start}`; a subject \
                     anchors at the heading, or the first line, that introduces it, one subject \
                     to a line",
                    surface.name, anchor.path
                ));
            }
        }

        for doc in &self.documents {
            if !entries.contains(doc.path.as_str()) && !answer.unreached.contains(&doc.path) {
                findings.push(format!(
                    "- `{}` is neither a subject's entry nor listed under `unreached`; name the \
                     subject it introduces, anchored at its heading or first line, or list it",
                    doc.path
                ));
            }
        }

        findings
    }

    // Derives each named subject's span and lead from its accepted anchor in
    // document and line order.
    pub fn subjects(&self, inventory: &Inventory) -> Vec<Subject> {
        // the accepted anchors by document, each at its start line
        let mut by_doc: BTreeMap<&str, Vec<(u32, &Surface)>> = BTreeMap::new();
        for surface in &inventory.surfaces {
            let Ok(anchor) = surface.anchor() else { continue };
            if let Some(doc) = self.document(anchor.path) {
                by_doc.entry(&doc.path).or_default().push((start_line(anchor.lines), surface));
            }
        }

        let mut subjects = Vec::new();
        for doc in &self.documents {
            let Some(anchored) = by_doc.get_mut(doc.path.as_str()) else { continue };
            anchored.sort_by_key(|(start, surface)| (*start, surface.name.as_str()));
            anchored.dedup_by_key(|(start, _)| *start);
            let end = doc.outline.lines.max(1);
            for (index, (start, surface)) in anchored.iter().enumerate() {
                let anchor = (*start).min(end);
                let span = Lines {
                    start: if index == 0 { 1 } else { anchor },
                    end: anchored
                        .get(index + 1)
                        .map_or(end, |(next, _)| next.saturating_sub(1).max(anchor)),
                };
                subjects.push(Subject {
                    name: surface.name.clone(),
                    entry: doc.path.clone(),
                    stem: surface.stem.clone(),
                    lead: String::new(),
                    anchor,
                    span,
                });
            }
        }

        self.leads(&mut subjects);
        subjects
    }

    // The lead of each subject: the stem alone where it is the one subject
    // under its stem, else the stem and the first slug candidate no other
    // subject under the stem has taken.
    fn leads(&self, subjects: &mut [Subject]) {
        let mut under: BTreeMap<String, usize> = BTreeMap::new();
        for subject in subjects.iter() {
            *under.entry(subject.stem.clone()).or_default() += 1;
        }
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for subject in subjects.iter_mut() {
            if under.get(&subject.stem).copied().unwrap_or_default() <= 1 {
                subject.lead.clone_from(&subject.stem);
                continue;
            }
            let doc = self.document(&subject.entry);
            let heading = doc
                .and_then(|doc| {
                    doc.outline
                        .heading_at(subject.anchor)
                        .or_else(|| doc.outline.heading_in(subject.span))
                })
                .map(|heading| heading.text.as_str());
            let file = subject.entry.rsplit('/').next().and_then(|name| name.split('.').next());
            let candidates = [
                heading.and_then(emery_sdk::kebab),
                emery_sdk::kebab(&subject.name),
                file.and_then(emery_sdk::kebab),
                emery_sdk::kebab(&subject.entry),
            ];
            let slug = candidates
                .into_iter()
                .flatten()
                .find(|slug| {
                    *slug != subject.stem && !taken.contains(&format!("{}.{slug}", subject.stem))
                })
                .unwrap_or_else(|| format!("subject-{}", taken.len() + 1));
            subject.lead = format!("{}.{slug}", subject.stem);
            taken.insert(subject.lead.clone());
        }
    }
}

// A heading or an opening line, cut to one line of the outline.
fn clipped(text: &str) -> String {
    const WIDTH: usize = 96;
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= WIDTH {
        return text;
    }
    let mut clipped: String = text.chars().take(WIDTH).collect();
    clipped.push('…');
    clipped
}

fn start_line(lines: Option<(u64, u64)>) -> u32 {
    lines.map_or(1, |(start, _)| u32::try_from(start).unwrap_or(u32::MAX).max(1))
}
