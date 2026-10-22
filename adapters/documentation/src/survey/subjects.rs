//! Reads a documentation workspace and cuts its surveyed subjects into seams.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use emery_sdk::survey::Inventory;
use emery_sdk::{Error, INLINE_BYTES, Seam, SourceContent, SourceInput, serde_json, tracing};

use super::model::Subject;
use super::structure::Outline;

pub enum Preparation<'i> {
    Value,
    Workspace(Prepared<'i>),
}

impl<'i> Preparation<'i> {
    pub fn read(input: &'i SourceInput) -> Result<Self, Error> {
        let SourceContent::Workspace(root) = &input.content else {
            return Ok(Self::Value);
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
        Ok(Self::Workspace(Prepared {
            source: &input.name,
            root,
            documents,
        }))
    }
}

pub struct Prepared<'i> {
    pub source: &'i str,
    pub root: &'i str,
    pub documents: Vec<Document>,
}

pub struct Document {
    pub path: String,
    pub text: String,
    pub outline: Outline,
}

impl Prepared<'_> {
    pub const fn source(&self) -> &str {
        self.source
    }

    pub fn document(&self, path: &str) -> Option<&Document> {
        self.documents.iter().find(|doc| doc.path == path)
    }

    pub fn seams(&self, inventory: &Inventory) -> Result<Vec<Seam>, Error> {
        let source = self.source();
        let subjects = self.subjects(inventory);
        tracing::info!(
            %source,
            surfaces = subjects.len(),
            unplaced = inventory.unreached.len(),
            "placed by model"
        );
        trace_subjects(source, &subjects);

        if subjects.is_empty() {
            tracing::info!(%source, "no subject named; the tree is cut by directory");
            let seams = super::directories(self.root)?;
            return Ok(seams
                .into_iter()
                .map(|seam| Seam {
                    text: UNSURVEYED.to_owned(),
                    ..seam
                })
                .collect());
        }

        let size: usize = self.documents.iter().map(|doc| doc.text.len()).sum();
        let fits = u64::try_from(size).unwrap_or(u64::MAX) <= INLINE_BYTES;
        let stems: Vec<&str> = subjects
            .iter()
            .map(|subject| subject.stem.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let cut = Cut {
            prepared: self,
            stems: &stems,
            unreached: &inventory.unreached,
        };

        Ok(if fits {
            let all: Vec<&Subject> = subjects.iter().collect();
            vec![cut.seam(&all, None)]
        } else {
            stems
                .iter()
                .map(|stem| {
                    let under: Vec<&Subject> =
                        subjects.iter().filter(|subject| subject.stem == *stem).collect();
                    cut.seam(&under, Some(stem))
                })
                .collect()
        })
    }
}

const UNSURVEYED: &str = "No subject was found in this tree: the survey named none, so it is \
                          mined by directory. Read the documents as a reference is read — each \
                          rule where it is stated, under the domain noun the documents spell for \
                          what it governs — and lead every id with that noun.";

struct Cut<'a> {
    prepared: &'a Prepared<'a>,
    stems: &'a [&'a str],
    unreached: &'a [String],
}

impl Cut<'_> {
    fn seam(&self, subjects: &[&Subject], stem: Option<&str>) -> Seam {
        let files: Vec<String> = self
            .prepared
            .documents
            .iter()
            .filter(|doc| subjects.iter().any(|subject| subject.entry == doc.path))
            .map(|doc| doc.path.clone())
            .collect();
        let held: Vec<String> = stem.map_or_else(
            || self.stems.iter().map(|stem| (*stem).to_owned()).collect(),
            |stem| vec![stem.to_owned()],
        );
        Seam {
            text: self.text(subjects, &files, stem),
            files,
            stems: held,
            ..Seam::default()
        }
    }

    fn text(&self, subjects: &[&Subject], files: &[String], stem: Option<&str>) -> String {
        let source = self.prepared.source();
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
                    plural(self.stems.len(), "stem", "stems"),
                    quoted(self.stems.iter().copied()),
                )
            },
            |stem| format!("Stem `{stem}` of the source `{source}` — {counted}."),
        );

        text.push_str(
            "\n\nEach subject the survey named, the lines it spans, its stem, and the id its \
             `requirement`, `criterion`, and `retire` claims lead with. Mine the spans listed and \
             nothing outside them:\n",
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

        if !self.unreached.is_empty() {
            let _ = write!(
                text,
                "\n\nThe survey placed {} under no subject — an index, a readme, a glossary, a \
                 changelog: {}. {} in `$SOURCE_DIR` for context and never a `path`.",
                plural(self.unreached.len(), "this document", "these documents"),
                quoted(self.unreached.iter().map(String::as_str)),
                plural(self.unreached.len(), "It is", "They are"),
            );
        }

        if let Some(stem) = stem {
            let others: Vec<&str> =
                self.stems.iter().copied().filter(|&other| other != stem).collect();
            if !others.is_empty() {
                let _ = write!(
                    text,
                    "\n\nThe other {} of this tree — {} — {} other calls'; claim nothing under \
                     {}, and nothing from a document this call does not list.",
                    plural(others.len(), "stem", "stems"),
                    quoted(others.iter().copied()),
                    plural(others.len(), "is", "are"),
                    plural(others.len(), "it", "them"),
                );
            }
        }

        text.push_str(
            "\n\nA claim anchors at the lines that state it, never at a heading, and keeps the \
             kind the prompt gives it whatever line it anchors at.",
        );

        text
    }
}

const fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

fn quoted<'s>(items: impl IntoIterator<Item = &'s str>) -> String {
    items.into_iter().map(|item| format!("`{item}`")).collect::<Vec<_>>().join(", ")
}

// Matches the `surveyed` trace event shape that `evals` reads.
fn trace_subjects(source: &str, subjects: &[Subject]) {
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
