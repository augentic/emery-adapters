//! The subject survey under `model-survey`: what code reads of the tree
//! before the one turn, and the seams it cuts from the subjects the model
//! names.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use emery_sdk::survey::{Inventory, Lines};
use emery_sdk::{Error, INLINE_BYTES, Seam, SourceContent, SourceInput, serde_json, tracing};

use super::model::{self, Subject};
use super::structure::Outline;

// What an input is to the survey.
pub enum Preparation {
    // an inline value, mined whole and never surveyed
    Value,
    // a workspace, its documents read
    Workspace(Prepared),
}

// A workspace read for the survey: the source's name and every document
// of the tree in path order.
pub struct Prepared {
    pub source: String,
    pub documents: Vec<Document>,
}

// One document: its root-relative path, its text, and its structure.
pub struct Document {
    pub path: String,
    pub text: String,
    pub outline: Outline,
}

impl Prepared {
    pub fn document(&self, path: &str) -> Option<&Document> {
        self.documents.iter().find(|doc| doc.path == path)
    }
}

// Reads the workspace: every file the directory cut would list that
// reads as text is a document, with its outline. A file that does not —
// an image beside the documents — is no document and no module.
pub fn prepare(input: &SourceInput) -> Result<Preparation, Error> {
    let SourceContent::Workspace(root) = &input.content else {
        return Ok(Preparation::Value);
    };
    let files = emery_sdk::workspace::list(root, |entry| !entry.hidden())?;
    let documents = files
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(Path::new(root).join(&path)).ok()?;
            let outline = Outline::read(&text);
            Some(Document { path, text, outline })
        })
        .collect();
    Ok(Preparation::Workspace(Prepared {
        source: input.name.clone(),
        documents,
    }))
}

// Cuts the seams from the subjects the model named: one seam over every
// subject's document within the inline budget, one per stem past it, and
// the directory cut — told no subject was found — when it named none.
pub fn seams(
    input: &SourceInput, prepared: &Prepared, inventory: &Inventory,
) -> Result<Vec<Seam>, Error> {
    let source = &prepared.source;
    let subjects = model::build(prepared, inventory);
    tracing::info!(
        %source,
        surfaces = subjects.len(),
        unplaced = inventory.unreached.len(),
        "placed by model"
    );
    logged(source, &subjects);

    if subjects.is_empty() {
        tracing::info!(%source, "no subject named; the tree is cut by directory");
        let seams = super::survey(input)?;
        return Ok(seams
            .into_iter()
            .map(|seam| Seam {
                text: UNSURVEYED.to_owned(),
                ..seam
            })
            .collect());
    }

    // the tree's size decides the cut, as the SDK's code survey sums every
    // module: one call within the inline budget, one per stem past it
    let size: usize = prepared.documents.iter().map(|doc| doc.text.len()).sum();
    let fits = u64::try_from(size).unwrap_or(u64::MAX) <= INLINE_BYTES;
    let stems: Vec<&str> = {
        let set: BTreeSet<&str> = subjects.iter().map(|subject| subject.stem.as_str()).collect();
        set.into_iter().collect()
    };

    Ok(if fits {
        vec![cut(prepared, &subjects, &stems, None, &inventory.unreached)]
    } else {
        stems
            .iter()
            .map(|stem| {
                let under: Vec<Subject> =
                    subjects.iter().filter(|subject| subject.stem == *stem).cloned().collect();
                cut(prepared, &under, &stems, Some(stem), &inventory.unreached)
            })
            .collect()
    })
}

const UNSURVEYED: &str = "No subject was found in this tree: the survey named none, so it is \
                          mined by directory. Read the documents as a reference is read — each \
                          rule where it is stated, under the domain noun the documents spell for \
                          what it governs — and lead every id with that noun.";

// One seam: over the documents the subjects anchor in, in path order,
// held to its stems, led by the subjects and the rules of the turn.
fn cut(
    prepared: &Prepared, subjects: &[Subject], stems: &[&str], stem: Option<&str>,
    unreached: &[String],
) -> Seam {
    let files: Vec<String> = prepared
        .documents
        .iter()
        .filter(|doc| subjects.iter().any(|subject| subject.entry == doc.path))
        .map(|doc| doc.path.clone())
        .collect();
    let held: Vec<String> = stem.map_or_else(
        || stems.iter().map(|stem| (*stem).to_owned()).collect(),
        |stem| vec![stem.to_owned()],
    );
    let anchors = anchors(prepared, subjects);
    let text = text(&prepared.source, subjects, &files, stems, stem, unreached, &anchors);
    Seam {
        text,
        files,
        stems: held,
        anchors: anchors
            .iter()
            .flat_map(|(path, spans)| {
                spans.iter().map(move |span| format!("{path}#{}", span.anchor()))
            })
            .collect(),
    }
}

// The brief: the subjects and their spans, the documents under no
// subject, the other stems where the seam is one of several, the anchor
// rule, and — under `anchors` — the lines a requirement may anchor at.
fn text(
    source: &str, subjects: &[Subject], files: &[String], stems: &[&str], stem: Option<&str>,
    unreached: &[String], anchors: &[(String, Vec<Lines>)],
) -> String {
    let plural =
        |n: usize, one: &str, many: &str| if n == 1 { one.to_owned() } else { many.to_owned() };
    let counted = format!(
        "{} {} in {} {}",
        subjects.len(),
        plural(subjects.len(), "subject", "subjects"),
        files.len(),
        plural(files.len(), "document", "documents"),
    );
    let mut text = stem.map_or_else(
        || {
            format!(
                "Every subject of the source `{source}` — {counted} under the {} {}.",
                plural(stems.len(), "stem", "stems"),
                stems.iter().map(|stem| format!("`{stem}`")).collect::<Vec<_>>().join(", "),
            )
        },
        |stem| format!("Stem `{stem}` of the source `{source}` — {counted}."),
    );

    text.push_str(
        "\n\nEach subject the survey named, the lines it spans, its stem, and the id its \
         `requirement` and `criterion` claims lead with. Mine the spans listed and nothing \
         outside them:\n",
    );
    for subject in subjects {
        let _ = write!(
            text,
            "\n- `{}` — `{}#{}` — stem `{}` — ids lead with `{}`",
            subject.name,
            subject.entry,
            subject.span.anchor(),
            subject.stem,
            subject.lead
        );
    }

    if !unreached.is_empty() {
        let listed: Vec<String> = unreached.iter().map(|path| format!("`{path}`")).collect();
        let _ = write!(
            text,
            "\n\nThe survey placed {} under no subject — an index, a readme, a glossary, a \
             changelog: {}. {} in `$SOURCE_DIR` for context and never a `path`.",
            plural(unreached.len(), "this document", "these documents"),
            listed.join(", "),
            plural(unreached.len(), "It is", "They are"),
        );
    }

    if let Some(stem) = stem {
        let others: Vec<String> = stems
            .iter()
            .filter(|other| **other != stem)
            .map(|other| format!("`{other}`"))
            .collect();
        if !others.is_empty() {
            let _ = write!(
                text,
                "\n\nThe other {} of this tree — {} — {} other calls'; claim nothing under {}, \
                 and nothing from a document this call does not list.",
                plural(others.len(), "stem", "stems"),
                others.join(", "),
                plural(others.len(), "is", "are"),
                plural(others.len(), "it", "them"),
            );
        }
    }

    text.push_str(
        "\n\nA claim anchors at the lines that state it, never at a heading: a `requirement` \
         at the paragraph, item, row, step, or quotation that states the rule; a `criterion` \
         at the item that lists it — a bullet list beneath a rule, the cases a sentence \
         introduces, an acceptance list, are criteria under the rule's id, not requirements \
         of their own; a `decision` where the document records it. State each rule once, \
         where its subject states it.",
    );

    if !anchors.is_empty() {
        text.push_str(
            "\n\nThe lines a `requirement` may anchor at — every paragraph, list item, table \
             row, step, and quotation within the subjects above, by document:\n",
        );
        for (path, spans) in anchors {
            let listed: Vec<String> = spans.iter().map(|span| span.anchor()).collect();
            let _ = write!(text, "\n- `{path}`: {}", listed.join(", "));
        }
    }

    text
}

// Under `anchors`, the rule-bearing spans within the subjects' spans, by
// document in path order; none otherwise.
#[cfg(feature = "anchors")]
fn anchors(prepared: &Prepared, subjects: &[Subject]) -> Vec<(String, Vec<Lines>)> {
    let mut by_doc: std::collections::BTreeMap<&str, Vec<Lines>> =
        std::collections::BTreeMap::new();
    for subject in subjects {
        let Some(doc) = prepared.document(&subject.entry) else { continue };
        by_doc.entry(&doc.path).or_default().extend(doc.outline.stating(subject.span));
    }
    prepared
        .documents
        .iter()
        .filter_map(|doc| {
            let mut spans = by_doc.remove(doc.path.as_str())?;
            spans.sort_unstable();
            spans.dedup();
            Some((doc.path.clone(), spans))
        })
        .collect()
}

#[cfg(not(feature = "anchors"))]
const fn anchors(_prepared: &Prepared, _subjects: &[Subject]) -> Vec<(String, Vec<Lines>)> {
    Vec::new()
}

// The `surveyed` trace line the eval reads, in the shape the SDK logs
// for a parsed tree: each subject's name, entry, stem, span, and lead.
fn logged(source: &str, subjects: &[Subject]) {
    if !tracing::enabled!(tracing::Level::TRACE) {
        return;
    }
    let listed: Vec<serde_json::Value> = subjects
        .iter()
        .map(|subject| {
            serde_json::json!({
                "name": subject.name,
                "entry": subject.entry,
                "stem": subject.stem,
                "lines": subject.span.anchor(),
                "ids": [subject.lead],
            })
        })
        .collect();
    let json = serde_json::Value::Array(listed).to_string();
    tracing::trace!(%source, surfaces = %json, "surveyed");
}
