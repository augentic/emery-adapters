//! What a turn's candidate is held to: the seed unchanged, the code's own
//! rules, and a fresh capture of it scored against the original facts.
//!
//! A build turn has at most [`MAX_ATTEMPTS`] scored attempts: a failing one
//! comes back with what differs, written to `layout/feedback/attempt-<n>.json`,
//! and the last is accepted whatever it scores, the turn's best attempt then
//! put back in the tree.

use std::fmt::Write as _;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use emery_sdk::target::Verdict;
use emery_sdk::{Error, server_error, tracing};
use serde_json::Value;
use web_rendered_layout::{Facts, PASS_MARK, Review, Width, assemble, review, stamped};

use crate::tree::{
    CAPTURE, REVIEW, SEED, SHOTS, SOURCES, VERIFIED, clear_sources, copy_tree, remove_dir,
};

/// The scored attempts a build turn has.
///
/// The model backends check at most two candidates a completion (the Cursor
/// backend's round limit, genai's default `GENAI_MAX_CANDIDATES`), and a
/// refused last candidate fails the turn; so the last attempt must be one
/// the check accepts, and no more can be reached.
pub const MAX_ATTEMPTS: usize = 2;
/// Where a failing attempt's feedback is written, one file per attempt.
pub const FEEDBACK: &str = "layout/feedback";
/// Where each scored attempt's code, screenshots, and score are kept, by turn.
pub const ATTEMPTS: &str = "layout/attempts";

/// The build turn a candidate answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Turn {
    /// One static `src/Page.vue`.
    Extract,
    /// `src/Page.vue` and its components.
    Refactor,
}

impl Turn {
    const fn name(self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Refactor => "refactor",
        }
    }

    // The facts the turn reads, as its feedback names their node ids.
    const fn facts(self) -> &'static str {
        match self {
            Self::Extract => "layout/facts/",
            Self::Refactor => "layout/refactor/facts/",
        }
    }
}

// The page errors a finding quotes.
const QUOTED_ERRORS: usize = 3;

/// One scored attempt of a turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Attempt {
    /// Its number, from 1.
    pub number: usize,
    /// Whether it reached the pass mark.
    pub pass: bool,
    /// The lowest score of any kind over every viewport.
    pub lowest_score: f64,
    /// The mean of each kind's mean score.
    pub mean_score: f64,
}

impl Attempt {
    fn to_json(&self) -> Value {
        serde_json::json!({
            "attempt": self.number,
            "pass": self.pass,
            "lowest_score": self.lowest_score,
            "mean_score": self.mean_score,
        })
    }
}

/// A build turn's scored attempts, and the check each candidate meets.
pub struct Attempts<'a> {
    root: &'a Path,
    original: &'a Facts,
    turn: Turn,
    scored: Vec<Attempt>,
}

impl<'a> Attempts<'a> {
    /// No attempt yet of `turn` in the tree at `root`, scored against `original`.
    pub const fn new(root: &'a Path, original: &'a Facts, turn: Turn) -> Self {
        Self {
            root,
            original,
            turn,
            scored: Vec::new(),
        }
    }

    /// The findings on a candidate, empty when it is accepted.
    ///
    /// The code's rules are checked first, and a capture is scored only once
    /// they hold, since a capture of code that breaks them is not the answer;
    /// a refusal before the score is no attempt. A scored attempt is kept
    /// under `layout/attempts/`; it is accepted when it passes or is the
    /// last, and otherwise answered with what differs.
    ///
    /// # Errors
    ///
    /// When the score, its overlays, the feedback, or the kept attempt cannot be written.
    pub fn check(&mut self) -> Result<Vec<String>, Error> {
        let root = self.root;
        let mut findings = seed_findings(root);
        findings.extend(code_findings(root, self.turn));
        if !findings.is_empty() {
            return Ok(findings);
        }
        let render = match rendered(root, CAPTURE, self.original) {
            Ok(render) => render,
            Err(found) => return Ok(found),
        };
        if !holds_png(&root.join(SHOTS)) {
            return Ok(vec![format!(
                "`{SHOTS}/` holds no screenshot; capture with `--screenshots {SHOTS}` as references/capture.md says"
            )]);
        }
        let reviewed = match review(self.original, &render) {
            Ok(reviewed) => reviewed,
            Err(err) => {
                return Ok(vec![format!("the capture in `{CAPTURE}` cannot be scored: {err}")]);
            }
        };
        write_review(root, REVIEW, &reviewed)?;

        let comparison = &reviewed.comparison;
        let attempt = Attempt {
            number: self.scored.len() + 1,
            pass: comparison.pass,
            lowest_score: comparison.lowest(),
            mean_score: js_mean(comparison.mean()),
        };
        self.keep(&attempt)?;
        tracing::info!(
            turn = self.turn.name(),
            attempt = attempt.number,
            pass = attempt.pass,
            lowest = attempt.lowest_score,
            mean = attempt.mean_score,
            summary = %serde_json::to_string(&comparison.summary).unwrap_or_default(),
            "scored an attempt"
        );
        let number = attempt.number;
        self.scored.push(attempt);
        self.write_record()?;
        if comparison.pass {
            return Ok(Vec::new());
        }
        if number == MAX_ATTEMPTS {
            tracing::warn!(
                turn = self.turn.name(),
                attempts = MAX_ATTEMPTS,
                "no attempt reached the pass mark; the turn ends on its best attempt"
            );
            return Ok(Vec::new());
        }

        let feedback = format!("{FEEDBACK}/attempt-{number}.json");
        let body = serde_json::to_string_pretty(&reviewed.feedback(number, self.turn.facts()))
            .map_err(|err| server_error!("writing `{feedback}`: {err}"))?;
        write(&root.join(&feedback), (body + "\n").as_bytes())?;
        let mut findings = vec![format!(
            "attempt {number} of {MAX_ATTEMPTS}: the check scored your capture against the original page and it did not pass: its lowest score is {} against the pass mark {PASS_MARK}. `{feedback}` lists what differs. Fix the causes in `{SOURCES}/`, capture again, then answer again.",
            comparison.lowest()
        )];
        findings.extend(
            reviewed
                .overlays
                .iter()
                .map(|(width, _)| format!("overlay at {width}: {}", overlay_path(REVIEW, *width))),
        );
        Ok(findings)
    }

    /// The best scored attempt: a passing one, then the highest lowest
    /// score, then the highest mean, the earliest of equals.
    pub fn best(&self) -> Option<&Attempt> {
        self.scored.iter().min_by(|a, b| {
            b.pass
                .cmp(&a.pass)
                .then(b.lowest_score.total_cmp(&a.lowest_score))
                .then(b.mean_score.total_cmp(&a.mean_score))
        })
    }

    /// Puts the best attempt's code, screenshots, and score back in the
    /// tree where a later attempt replaced them, and returns it.
    ///
    /// # Errors
    ///
    /// When the kept attempt cannot be copied back.
    pub fn restore_best(&self) -> Result<Option<Attempt>, Error> {
        let Some(best) = self.best().cloned() else {
            return Ok(None);
        };
        if self.scored.last().map(|last| last.number) != Some(best.number) {
            let kept = self.kept(best.number);
            clear_sources(self.root)?;
            copy_tree(&self.root.join(format!("{kept}/src")), &self.root.join(SOURCES))?;
            for (from, to) in [("shots", SHOTS), ("review", REVIEW)] {
                remove_dir(self.root, to)?;
                copy_tree(&self.root.join(format!("{kept}/{from}")), &self.root.join(to))?;
            }
            tracing::info!(
                turn = self.turn.name(),
                attempt = best.number,
                "put the best attempt back"
            );
        }
        Ok(Some(best))
    }

    fn kept(&self, attempt: usize) -> String {
        format!("{ATTEMPTS}/{}/{attempt}", self.turn.name())
    }

    // The attempt's code, screenshots, and score, kept for the best to be put back.
    fn keep(&self, attempt: &Attempt) -> Result<(), Error> {
        let kept = self.kept(attempt.number);
        remove_dir(self.root, &kept)?;
        for (from, to) in [(SOURCES, "src"), (SHOTS, "shots"), (REVIEW, "review")] {
            copy_tree(&self.root.join(from), &self.root.join(format!("{kept}/{to}")))?;
        }
        Ok(())
    }

    /// Writes every scored attempt and the best to `layout/attempts/<turn>/attempts.json`,
    /// as the demo harness's `attempts.json`.
    ///
    /// # Errors
    ///
    /// When the record cannot be written.
    pub fn write_record(&self) -> Result<(), Error> {
        let record = serde_json::json!({
            "attempts": self.scored.iter().map(Attempt::to_json).collect::<Vec<_>>(),
            "best_attempt": self.best().map(Attempt::to_json),
        });
        let path = format!("{ATTEMPTS}/{}/attempts.json", self.turn.name());
        let body = serde_json::to_string_pretty(&record)
            .map_err(|err| server_error!("writing `{path}`: {err}"))?;
        write(&self.root.join(path), (body + "\n").as_bytes())
    }
}

/// The findings on a verdict: the score of a fresh capture of the tree, written to `layout/verify/`.
///
/// The verdict passes exactly when the score passes, and a failing
/// verdict's `failures` are the score's findings, each naming its overlay.
///
/// # Errors
///
/// When the score or its overlays cannot be written.
pub fn verdict(root: &Path, original: &Facts, verdict: &Verdict) -> Result<Vec<String>, Error> {
    let mut findings = seed_findings(root);
    if !findings.is_empty() {
        return Ok(findings);
    }
    let capture = format!("{VERIFIED}/capture");
    let render = match rendered(root, &capture, original) {
        Ok(render) => render,
        Err(found) => return Ok(found),
    };
    let reviewed = match review(original, &render) {
        Ok(reviewed) => reviewed,
        Err(err) => return Ok(vec![format!("the capture in `{capture}` cannot be scored: {err}")]),
    };
    write_review(root, VERIFIED, &reviewed)?;
    let passed = reviewed.comparison.pass;
    let expected = reviewed.findings(|width| overlay_path(VERIFIED, width));
    if verdict.passed != passed {
        findings.push(format!(
            "the capture scores {} against the pass mark, so the verdict is `passed: {passed}`",
            reviewed.comparison.lowest()
        ));
    }
    if !passed && verdict.failures != expected {
        let mut lines = String::new();
        for line in &expected {
            let _ = write!(lines, "\n- {line}");
        }
        findings.push(format!(
            "`failures` are the score's findings, each verbatim, in this order:{lines}"
        ));
    }
    Ok(findings)
}

// A mean to four places, as the scores are written.
fn js_mean(mean: f64) -> f64 {
    mean.mul_add(10_000.0, 0.5).floor() / 10_000.0
}

fn seed_findings(root: &Path) -> Vec<String> {
    SEED.iter()
        .filter(|(path, _)| *path != ".gitignore")
        .filter(|(path, seeded)| {
            fs::read_to_string(root.join(path)).ok().as_deref() != Some(*seeded)
        })
        .map(|(path, _)| {
            format!(
                "`{path}` is not as the seed wrote it; put it back as references/tree.md spells it"
            )
        })
        .collect()
}

fn code_findings(root: &Path, turn: Turn) -> Vec<String> {
    let mut findings = Vec::new();
    let mut files = Vec::new();
    if let Err(err) = walk(&root.join(SOURCES), "", &mut files) {
        return vec![format!("`{SOURCES}/` cannot be read: {err}")];
    }
    if !files.iter().any(|(path, _)| path == "Page.vue") {
        findings.push(format!("`{SOURCES}/Page.vue` is missing; it is the page's root component"));
    }
    for (path, body) in &files {
        for (number, line) in body.lines().enumerate() {
            let squeezed: String =
                line.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
            if turn == Turn::Refactor
                && (squeezed.contains("position:absolute") || squeezed.contains("position:fixed"))
            {
                findings.push(format!(
                    "`{SOURCES}/{path}:{}` positions an element out of the flow; lay it out with flex or grid",
                    number + 1
                ));
            }
            if squeezed.contains("layout/") {
                findings.push(format!(
                    "`{SOURCES}/{path}:{}` names `layout/`, which the page never reads",
                    number + 1
                ));
            }
        }
    }
    if turn == Turn::Refactor {
        let components: Vec<_> = files
            .iter()
            .filter(|(path, _)| path.starts_with("components/") && has_extension(path, "vue"))
            .collect();
        if components.is_empty() {
            findings.push(format!("`{SOURCES}/components/` holds no component"));
        }
        for (path, body) in components {
            if body
                .match_indices("<style")
                .any(|(at, _)| !body[at..].split('>').next().unwrap_or_default().contains("scoped"))
            {
                findings.push(format!("`{SOURCES}/{path}` carries a `<style>` that is not scoped"));
            }
        }
    }
    findings
}

// The render a capture under `dir` holds, or why it is not an answer.
fn rendered(root: &Path, dir: &str, original: &Facts) -> Result<Facts, Vec<String>> {
    let capture = root.join(dir);
    let meta = match fs::read(capture.join("meta.json")) {
        Ok(meta) => meta,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(vec![format!(
                "`{dir}/` holds no capture; capture as references/capture.md says"
            )]);
        }
        Err(err) => return Err(vec![format!("`{dir}/meta.json` cannot be read: {err}")]),
    };
    stamped(&capture, &root.join(SOURCES)).map_err(|err| vec![format!("`{dir}`: {err}")])?;
    let meta: Value = serde_json::from_slice(&meta)
        .map_err(|err| vec![format!("`{dir}/meta.json` is not JSON: {err}")])?;
    if let Some(errors) = meta["page_errors"].as_array().filter(|errors| !errors.is_empty()) {
        let quoted: Vec<String> = errors.iter().take(QUOTED_ERRORS).map(Value::to_string).collect();
        return Err(vec![format!(
            "the page raised {} errors while captured, first {}",
            errors.len(),
            quoted.join("; ")
        )]);
    }
    let folder = assemble(&capture)
        .map_err(|err| vec![format!("the capture in `{dir}` does not assemble: {err}")])?;
    let render = Facts::read(&folder)
        .map_err(|err| vec![format!("the capture in `{dir}` reads as no facts: {err}")])?;
    if let Some(browser) = original.browser()
        && render.browser() != Some(browser)
    {
        return Err(vec![format!(
            "the capture in `{dir}` ran in the browser {} and the original's facts in {browser}; a capture is scored only against facts the same browser took",
            render.browser().map_or_else(|| "none recorded".to_owned(), Value::to_string),
        )]);
    }
    if !sweeps_alike(&render, original) {
        return Err(vec![format!(
            "the capture in `{dir}` sweeps widths {:?} and heights {:?} at height {}; capture the original's {:?}, {:?} at {}",
            render.width_range(),
            render.height_range(),
            render.height(),
            original.width_range(),
            original.height_range(),
            original.height()
        )]);
    }
    Ok(render)
}

fn write_review(root: &Path, dir: &str, reviewed: &Review) -> Result<(), Error> {
    let out = root.join(dir);
    fs::create_dir_all(&out).map_err(|err| server_error!("creating `{dir}`: {err}"))?;
    let entries = fs::read_dir(&out).map_err(|err| server_error!("reading `{dir}`: {err}"))?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("overlay-") && has_extension(&name, "svg") {
            fs::remove_file(entry.path())
                .map_err(|err| server_error!("removing `{dir}/{name}`: {err}"))?;
        }
    }
    let score = serde_json::to_string_pretty(&reviewed.comparison)
        .map_err(|err| server_error!("writing `{dir}/score.json`: {err}"))?;
    fs::write(out.join("score.json"), score + "\n")
        .map_err(|err| server_error!("writing `{dir}/score.json`: {err}"))?;
    for (width, svg) in &reviewed.overlays {
        fs::write(root.join(overlay_path(dir, *width)), svg)
            .map_err(|err| server_error!("writing `{}`: {err}", overlay_path(dir, *width)))?;
    }
    Ok(())
}

fn write(file: &Path, body: &[u8]) -> Result<(), Error> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| server_error!("creating {}: {err}", parent.display()))?;
    }
    fs::write(file, body).map_err(|err| server_error!("writing {}: {err}", file.display()))
}

#[expect(clippy::float_cmp, reason = "both sweeps are whole pixels read from JSON")]
fn sweeps_alike(render: &Facts, original: &Facts) -> bool {
    render.width_range() == original.width_range()
        && render.height_range() == original.height_range()
        && render.height() == original.height()
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path).extension().is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

fn overlay_path(dir: &str, width: Width) -> String {
    format!("{dir}/overlay-{}.svg", width.0)
}

fn holds_png(dir: &Path) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| entry.path().extension().is_some_and(|ext| ext == "png"))
    })
}

// Every `.vue`, `.js`, and `.css` file under `dir`, with its path below it.
fn walk(dir: &Path, prefix: &str, files: &mut Vec<(String, String)>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.file_type()?.is_dir() {
            walk(&entry.path(), &format!("{path}/"), files)?;
        } else if ["vue", "js", "css"].iter().any(|ext| has_extension(&name, ext)) {
            files.push((path, fs::read_to_string(entry.path())?));
        }
    }
    Ok(())
}
