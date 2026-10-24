//! Runs `emery specify` over a staged case and reads the run back from its log and envelope.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter};
use std::path::Path;
use std::time::{Duration, Instant};
use std::{fs, io};

use emery_sdk::{Anchor, Claim, ClaimKind, Evidence, Lexicon};
use serde::Deserialize;
use serde_json::Value;

use crate::build::{Build, apply, build, strings};
use crate::case::{Behaviour, Case, ExpectedClaim};
use crate::grade::{Holders, Stated, citations, cites, held, mechanism, stem, under};
use crate::stage::retarget;
use crate::{Rung, Settings, Turns, emery};

const BOOTSTRAP: &str = "start";
const SPECIFY: &[&str] = &["specify", "--config", "emery.toml"];

pub struct Run {
    pub n: usize,
    pub rung: Rung,
    pub wall: Duration,
    pub exit: Option<i32>,
    pub tail: String,
    // the staged plan's slices grouped into the sets ready to build at
    // once, as the specify envelope reports them
    pub waves: Option<Value>,
    // each seam's accepted evidence, under the source it was mined from
    pub evidence: Vec<(String, Evidence)>,
    pub completions: Vec<Completion>,
    // the surfaces each source's seams were cut from, as its adapter logged them
    pub surveyed: BTreeMap<String, Vec<Surveyed>>,
    // the modules each source's survey placed under no surface, by the
    // adapter's own count (`placed by model`), else as the model listed them
    // (the SDK's `surveyed by model` line)
    pub unreached: BTreeMap<String, usize>,
    pub spec: Option<Value>,
    pub design: Option<Value>,
    pub plan: Option<Value>,
    // the summary of a stillborn attempt this run was put again after
    pub dead: Option<String>,
    // the build of the plan, sealed with `apply` first, on `--build` after a
    // specify that landed
    pub build: Option<Build>,
}

// One surface as an adapter's `surveyed` line spells it; `lines` is its
// registration or declaration, `L<n>` or `L<n>-L<n>`, where the adapter
// logs one.
#[derive(Deserialize)]
pub struct Surveyed {
    pub name: String,
    pub entry: String,
    pub stem: String,
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    lines: Option<String>,
}

impl Surveyed {
    fn span(&self) -> Option<(u64, u64)> {
        let text = self.lines.as_deref()?.strip_prefix('L')?;
        let (start, end) = text.split_once("-L").unwrap_or((text, text));
        Some((start.parse().ok()?, end.parse().ok()?))
    }
}

impl Run {
    pub fn summary(&self) -> String {
        let specified = match self.exit {
            Some(0) => format!(
                "ok in {}s, {} completions, {} claims",
                self.wall.as_secs(),
                self.completions.len(),
                self.evidence.iter().map(|(_, evidence)| evidence.claims.len()).sum::<usize>()
            ),
            Some(code) => {
                let died = self.died_in().map_or(String::new(), |label| format!(" in `{label}`"));
                format!("exit {code}{died} after {}s", self.wall.as_secs())
            }
            None => format!("killed after {}s", self.wall.as_secs()),
        };
        match &self.build {
            Some(build) => format!("{specified}; build {}", build.summary(self.slices())),
            None => specified,
        }
    }

    // Whether the backend's cap ended a completion: what the next rung of
    // the ladder is for.
    pub fn starved(&self) -> bool {
        self.completions
            .iter()
            .any(|completion| matches!(completion.outcome.as_str(), "timeout" | "inactive"))
    }

    // Whether the backend refused the run before any claim was accepted:
    // `bad_gateway` exits 4, and with no evidence there is nothing to grade.
    pub fn stillborn(&self) -> bool {
        self.exit == Some(4) && self.evidence.is_empty()
    }

    pub fn passed(&self) -> bool {
        self.exit == Some(0) && !self.starved()
    }

    // The label of the completion a failed run died in: the one the backend
    // or the gate ended, the siblings it aborted set aside.
    pub fn died_in(&self) -> Option<&str> {
        self.completions
            .iter()
            .rev()
            .find(|completion| !matches!(completion.outcome.as_str(), "ok" | "corrected" | "abort"))
            .or_else(|| self.completions.last())
            .map(|completion| completion.label.as_str())
    }

    // Where the accepted requirement claims anchor against the surfaces the
    // surveys decided, each source's against its own, the bootstrap set
    // aside — its span is its module or its guard, not a registration or a
    // declaration; none when no adapter logged a surface span.
    pub fn landing(&self) -> Option<Landing> {
        let spans: Vec<(&str, &str, (u64, u64))> = self
            .surveyed
            .iter()
            .flat_map(|(source, surfaces)| {
                surfaces.iter().filter(|surface| surface.stem != BOOTSTRAP).filter_map(|surface| {
                    Some((source.as_str(), surface.entry.as_str(), surface.span()?))
                })
            })
            .collect();
        if spans.is_empty() {
            return None;
        }
        let mut landing = Landing::default();
        for (source, claim) in self.sourced(ClaimKind::Requirement) {
            let Some(Ok(anchor)) = claim.anchor() else { continue };
            let Some((start, end)) = anchor.lines else { continue };
            let at = |held: &dyn Fn((u64, u64)) -> bool| {
                spans
                    .iter()
                    .any(|(of, entry, span)| *of == source && *entry == anchor.path && held(*span))
            };
            if at(&|(first, _)| start <= first && first <= end) {
                landing.head += 1;
            } else if at(&|(first, last)| first < start && end <= last) {
                landing.inside += 1;
            } else {
                landing.outside += 1;
            }
        }
        Some(landing)
    }

    // Every accepted claim of `kind`, with the source it was mined from.
    fn sourced(&self, kind: ClaimKind) -> impl Iterator<Item = (&str, &Claim)> {
        self.evidence
            .iter()
            .flat_map(|(source, evidence)| {
                evidence.claims.iter().map(move |claim| (source.as_str(), claim))
            })
            .filter(move |(_, claim)| claim.kind == kind)
    }

    pub fn claims(&self, kind: ClaimKind) -> impl Iterator<Item = &Claim> {
        self.sourced(kind).map(|(_, claim)| claim)
    }

    pub fn claims_of<'r>(
        &'r self, source: &'r str, kind: ClaimKind,
    ) -> impl Iterator<Item = &'r Claim> {
        self.sourced(kind).filter(move |(of, _)| *of == source).map(|(_, claim)| claim)
    }

    // The id of the accepted criterion through which `requirement` covers
    // `claim`, when one does: a criterion of the claim's source held to its
    // lines, under a claim id the requirement cites from that source — the
    // engine's own reading of `covered`.
    fn covers(&self, requirement: &Value, claim: &ExpectedClaim) -> Option<String> {
        let Ok(wanted) = Anchor::parse(&claim.anchor) else { return None };
        let parents: Vec<&str> = requirement["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|cited| cited["source"].as_str() == Some(claim.source.as_str()))
            .filter_map(|cited| cited["claim"].as_str())
            .collect();
        self.claims_of(&claim.source, ClaimKind::Criterion).find_map(|criterion| {
            let Some(Ok(anchor)) = criterion.anchor() else { return None };
            let id = criterion.id.as_deref()?;
            (held(anchor, wanted) && parents.iter().any(|parent| under(id, parent)))
                .then(|| id.to_owned())
        })
    }

    pub fn meets(&self, requirement: &Value, claim: &ExpectedClaim) -> bool {
        cites(requirement, claim) || self.covers(requirement, claim).is_some()
    }

    // The requirements holding `behaviour`: those citing one of its claims,
    // and those holding one as a criterion alone.
    pub fn holders(&self, behaviour: &Behaviour) -> Holders {
        let requirements = self.requirements();
        let citing: Vec<usize> = (0..requirements.len())
            .filter(|&index| {
                behaviour.claims.iter().any(|claim| cites(&requirements[index], claim))
            })
            .collect();
        let covering = (0..requirements.len())
            .filter(|index| !citing.contains(index))
            .filter(|&index| {
                behaviour
                    .claims
                    .iter()
                    .any(|claim| self.covers(&requirements[index], claim).is_some())
            })
            .collect();
        Holders { citing, covering }
    }

    // The criteria through which the requirements at `indices` hold
    // `behaviour`, each under its requirement's subject.
    pub fn criteria_under(&self, indices: &[usize], behaviour: &Behaviour) -> Vec<String> {
        indices
            .iter()
            .flat_map(|&index| {
                behaviour.claims.iter().filter_map(move |claim| {
                    self.covers(&self.requirements()[index], claim)
                        .map(|id| format!("`{id}` under `{}`", self.subject(index)))
                })
            })
            .collect()
    }

    pub fn subject(&self, index: usize) -> String {
        self.requirements()[index]["subject"]
            .as_str()
            .map_or_else(|| index.to_string(), str::to_owned)
    }

    // The stems each source's requirement claims lead with, as (source, stem).
    pub fn stems(&self) -> BTreeSet<(&str, &str)> {
        self.sourced(ClaimKind::Requirement)
            .filter_map(|(source, claim)| Some((source, stem(claim.id.as_deref()?)?)))
            .collect()
    }

    pub fn stems_of<'r>(&'r self, source: &'r str) -> BTreeSet<&'r str> {
        self.stems().into_iter().filter(|(of, _)| *of == source).map(|(_, stem)| stem).collect()
    }

    pub fn ids(&self) -> BTreeSet<(&str, &str)> {
        self.sourced(ClaimKind::Requirement)
            .filter_map(|(source, claim)| Some((source, claim.id.as_deref()?)))
            .collect()
    }

    // The (source, entry, stem) triples the surveys decided, however many
    // surfaces share one.
    pub fn pairs(&self) -> BTreeSet<(&str, &str, &str)> {
        self.surveyed
            .iter()
            .flat_map(|(source, surfaces)| {
                surfaces.iter().map(move |surface| {
                    (source.as_str(), surface.entry.as_str(), surface.stem.as_str())
                })
            })
            .collect()
    }

    pub fn pairs_of<'r>(&'r self, source: &'r str) -> BTreeSet<(&'r str, &'r str)> {
        self.pairs()
            .into_iter()
            .filter(|(of, _, _)| *of == source)
            .map(|(_, entry, stem)| (entry, stem))
            .collect()
    }

    pub fn requirements(&self) -> &[Value] {
        self.spec
            .as_ref()
            .and_then(|spec| spec["requirements"].as_array())
            .map_or(&[], Vec::as_slice)
    }

    pub fn subjects(&self) -> BTreeSet<&str> {
        self.requirements()
            .iter()
            .filter_map(|requirement| requirement["subject"].as_str())
            .collect()
    }

    pub fn status(&self, status: &str) -> usize {
        self.requirements().iter().filter(|requirement| requirement["status"] == status).count()
    }

    pub fn covered(&self) -> usize {
        self.requirements().iter().filter(|requirement| requirement["covered"] == true).count()
    }

    pub fn unknown_outcomes(&self) -> usize {
        self.requirements()
            .iter()
            .filter_map(|requirement| requirement["scenarios"].as_array())
            .flatten()
            .filter(|scenario| scenario["then"] == "[unknown]")
            .count()
    }

    pub fn design_blocks(&self) -> (usize, usize) {
        let blocks: Vec<&Value> = self
            .design
            .as_ref()
            .and_then(|design| design["sections"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|section| section["blocks"].as_array())
            .flatten()
            .collect();
        let types = blocks.iter().filter(|block| block.get("type").is_some()).count();
        (blocks.len(), types)
    }

    // The design's data type names: each `data` `type` block's key, the name
    // alone where the key carries an anchor beside it. A `mechanism` type —
    // a client, a retriever, a bootstrap, a settings carrier — is left out,
    // so a requirement naming one counts as stated through the mechanism.
    pub fn design_types(&self) -> BTreeSet<&str> {
        self.design
            .as_ref()
            .and_then(|design| design["sections"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|section| section["blocks"].as_array())
            .flatten()
            .filter(|block| block["type"]["role"].as_str() != Some("mechanism"))
            .filter_map(|block| block["type"]["key"].as_str())
            .filter_map(|key| key.split([' ', '(']).next())
            .collect()
    }

    pub fn mechanism(&self) -> Vec<Stated> {
        mechanism(self.requirements(), &self.design_types())
    }

    // The lexicon the run's specification is held to, as the engine holds
    // its spec draft: the fixed names, and the packages the accepted `call`
    // claims spell before the `:` of their `callee`.
    pub fn lexicon(&self) -> Lexicon {
        Lexicon::from_packages(
            self.claims(ClaimKind::Call)
                .filter_map(|claim| claim.extras.get("callee")?.as_str())
                .filter_map(|callee| callee.split_once(':').map(|(package, _)| package)),
        )
    }

    // Every place the specification's own words name a technology, in the
    // order the gates read them: each accepted `requirement` and
    // `criterion` claim's id and body, then the staged specification's
    // preamble paragraphs and scenario lines. Each is one the SDK's or the
    // engine's gate should have returned for correction, so the list is
    // expected empty.
    pub fn leaks(&self) -> Vec<String> {
        let lexicon = self.lexicon();
        let mut leaks = Vec::new();
        let mut leak = |label: String, text: &str| {
            let named = lexicon.names(text);
            if !named.is_empty() {
                let named: Vec<String> = named.iter().map(|name| format!("`{name}`")).collect();
                leaks.push(format!("{label} names {}", named.join(", ")));
            }
        };

        // the accepted claims
        for (kind, field) in
            [(ClaimKind::Requirement, "statement"), (ClaimKind::Criterion, "criterion")]
        {
            for (source, claim) in self.sourced(kind) {
                let id = claim.id.as_deref().unwrap_or("<unnamed>");
                leak(format!("{source}:`{id}` id"), id);
                if let Some(text) = claim.extras.get(field).and_then(Value::as_str) {
                    leak(format!("{source}:`{id}` {field}"), text);
                }
            }
        }

        // the specification
        let preamble = self.spec.as_ref().map(|spec| strings(&spec["preamble"]));
        for (index, paragraph) in preamble.iter().flatten().enumerate() {
            leak(format!("preamble paragraph {}", index + 1), paragraph);
        }
        for requirement in self.requirements() {
            let subject = requirement["subject"].as_str().unwrap_or("?");
            for scenario in requirement["scenarios"].as_array().into_iter().flatten() {
                let name = scenario["name"].as_str().unwrap_or("");
                leak(format!("`{subject}` scenario `{name}` name"), name);
                for field in ["given", "when", "then", "and"] {
                    let lines = match &scenario[field] {
                        Value::String(line) => vec![line.clone()],
                        value => strings(value),
                    };
                    for line in lines {
                        leak(format!("`{subject}` scenario `{name}` {field}"), &line);
                    }
                }
            }
        }

        leaks
    }

    // The requirements citing `source`.
    pub fn citing(&self, source: &str) -> usize {
        self.requirements()
            .iter()
            .filter(|requirement| citations(requirement).any(|(of, _)| of == source))
            .count()
    }

    pub fn slices(&self) -> usize {
        self.plan.as_ref().and_then(|plan| plan["slices"].as_array()).map_or(0, Vec::len)
    }

    pub fn wave_count(&self) -> usize {
        self.waves.as_ref().and_then(Value::as_array).map_or(0, Vec::len)
    }

    // The plan's shape: how many waves, then each wave's width, so `3 (1/2/1)`
    // is three waves of one, two, and one slices; `0` where no plan was staged.
    pub fn waves(&self) -> String {
        let widths: Vec<String> = self
            .waves
            .as_ref()
            .and_then(Value::as_array)
            .map(|waves| {
                waves.iter().map(|wave| wave.as_array().map_or(0, Vec::len).to_string()).collect()
            })
            .unwrap_or_default();
        if widths.is_empty() {
            "0".to_owned()
        } else {
            format!("{} ({})", widths.len(), widths.join("/"))
        }
    }

    pub fn tokens(&self) -> Tokens {
        self.completions
            .iter()
            .map(|completion| completion.tokens)
            .fold(Tokens::default(), Tokens::add)
    }
}

// Where the accepted requirement claims land against the registered or
// declared surfaces: covering a surface's own first line — the span a
// requirement over a whole handler takes, from its decorator or
// registration — inside a surface's span past that line, or outside every
// surface (the bootstrap's module among the outside). What an arm over the
// anchor set is read by.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Landing {
    pub head: usize,
    pub inside: usize,
    pub outside: usize,
}

impl Display for Landing {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} covering a surface's head, {} inside a surface past it, {} outside every surface",
            self.head, self.inside, self.outside
        )
    }
}

#[derive(Clone, Copy, Default)]
pub struct Tokens {
    pub input: u64,
    pub cached: u64,
    pub output: u64,
    pub reasoning: u64,
}

impl Tokens {
    pub const fn add(self, other: Self) -> Self {
        Self {
            input: self.input + other.input,
            cached: self.cached + other.cached,
            output: self.output + other.output,
            reasoning: self.reasoning + other.reasoning,
        }
    }
}

pub struct Completion {
    pub label: String,
    pub outcome: String,
    pub duration: Duration,
    pub tokens: Tokens,
    pub shell: ShellUse,
    pub writes: Writes,
}

// The shell calls one completion made: how many, their summed wall time
// from the call to its completion, and the bytes their results carried back.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShellUse {
    pub calls: usize,
    pub time: Duration,
    pub bytes: u64,
}

// What one turn's `write_files` and `edit_files` calls did, read from the
// SDK's `responded` lines: the `write_files` calls answered, and among the
// files they wrote those the turn had written before, those written empty,
// and those rewritten at under half their last size; the `edit_files` calls
// answered; and the calls of either tool refused.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Writes {
    pub calls: usize,
    pub rewrites: usize,
    pub empty: usize,
    pub shrinking: usize,
    pub edits: usize,
    pub refused: usize,
}

impl Writes {
    pub const fn any(&self) -> bool {
        self.calls + self.edits + self.refused > 0
    }
}

impl Display for Writes {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "write_files {} calls, {} rewrites, {} empty, {} shrinking",
            self.calls, self.rewrites, self.empty, self.shrinking
        )?;
        if self.edits > 0 {
            write!(f, "; edit_files {} calls", self.edits)?;
        }
        if self.refused > 0 {
            write!(f, "; {} refused", self.refused)?;
        }
        Ok(())
    }
}

impl Completion {
    // Whether the turn closed with an answer the gate accepted, on its
    // first round or a correction.
    pub fn answered(&self) -> bool {
        matches!(self.outcome.as_str(), "ok" | "corrected")
    }
}

#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
pub fn run(
    project: &Path, settings: &Settings, case: &Case, n: usize, rung: Rung, tag: &str,
) -> io::Result<Run> {
    let started = Instant::now();
    let rust_log = settings.rust_log(case);
    let output = emery(project, settings, &rust_log, rung, SPECIFY, Turns::Live)?;
    let wall = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    fs::write(project.join(format!("{tag}.stderr")), stderr.as_bytes())?;
    fs::write(project.join(format!("{tag}.stdout")), stdout.as_bytes())?;
    let facts = survey_facts(&stderr);
    if !facts.is_empty() {
        fs::write(project.join(format!("{tag}.facts.md")), facts_text(&facts))?;
    }

    // what the run staged, read back through `show --staged`
    let envelope: Option<Value> = serde_json::from_str(&stdout).ok();
    let mut run = Run {
        n,
        rung,
        wall,
        exit: output.status.code(),
        tail: tail(&stderr),
        waves: envelope.as_ref().and_then(|out| out.get("waves").cloned()),
        evidence: accepted(&stderr),
        completions: completions(&stderr),
        surveyed: surveyed(&stderr),
        unreached: unreached(&stderr),
        spec: None,
        design: None,
        plan: None,
        dead: None,
        build: None,
    };
    if run.exit == Some(0) {
        for (artifact, slot) in
            [("spec", &mut run.spec), ("design", &mut run.design), ("plan", &mut run.plan)]
        {
            let shown = emery(
                project,
                settings,
                &rust_log,
                rung,
                &["show", artifact, "--staged"],
                Turns::Live,
            )?;
            let envelope: Value = serde_json::from_slice(&shown.stdout)?;
            fs::write(
                project.join(format!("{tag}.{artifact}.json")),
                serde_json::to_string_pretty(&envelope["document"])?,
            )?;
            *slot = Some(envelope["document"].clone());
        }
    }
    // the claims are graded whether or not the engine's turns landed after them
    for (index, (source, evidence)) in run.evidence.iter().enumerate() {
        fs::write(
            project.join(format!("{tag}.evidence-{source}-{index}.json")),
            serde_json::to_string_pretty(evidence)?,
        )?;
    }

    // the plan is sealed and built once the specify landed, as the ladder
    // reads it: a starved run climbs a rung instead, and a failed one has no
    // plan
    let revision = envelope.as_ref().and_then(|out| out["revision"].as_str());
    if let Some(revision) = revision
        && let Some(target) = settings.target
        && run.passed()
    {
        retarget(project, settings, target, n)?;
        apply(project, settings, case, tag, revision)?;
        eprintln!("eval: `{}` run {n} building at {}", case.name, settings.build_rung);
        let built = build(project, settings, case, n, tag, revision, run.spec.as_ref())?;
        run.build = Some(built);
    }

    Ok(run)
}

// The survey facts a staged project's adapters render, read from one run that
// spends no turn: the run fails at the first completion, after the facts
// are logged.
pub fn facts(project: &Path, settings: &Settings, case: &Case) -> io::Result<String> {
    let rust_log = settings.rust_log(case);
    let rung =
        settings.ladder.first().copied().ok_or_else(|| io::Error::other("EVAL_LADDER: no rung"))?;
    let output = emery(project, settings, &rust_log, rung, SPECIFY, Turns::None)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    fs::write(project.join("facts.stderr"), stderr.as_bytes())?;
    let facts = survey_facts(&stderr);
    if facts.is_empty() {
        let adapters: Vec<&str> = case.expected.adapters().collect();
        return Err(io::Error::other(format!(
            "no `survey facts` line from {adapters:?} (does any survey? is each at `trace` in \
             RUST_LOG?); the run ended:\n{}",
            tail(&stderr)
        )));
    }
    Ok(facts_text(&facts))
}

// The last five lines of a log, in order, for a failed run's summary.
pub fn tail(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().rev().take(5).collect();
    lines.reverse();
    lines.join("\n")
}

// One source's facts bare; several each under a heading naming the source.
fn facts_text(facts: &BTreeMap<String, String>) -> String {
    if let [(_, text)] = facts.iter().collect::<Vec<_>>().as_slice() {
        return (*text).clone();
    }
    facts
        .iter()
        .map(|(source, text)| format!("# source `{source}`\n\n{text}\n"))
        .collect::<Vec<_>>()
        .join("\n")
}

// The value a log line carries under `<key>=`, bare up to the next space.
fn field<'l>(line: &'l str, key: &str) -> Option<&'l str> {
    let (_, rest) = line.split_once(key)?;
    rest.split_whitespace().next()
}

// An adapter's `survey facts` trace line carries the text rendered before the
// model as one JSON string at the end of the line, per source.
fn survey_facts(stderr: &str) -> BTreeMap<String, String> {
    stderr
        .lines()
        .filter(|line| line.contains(" survey facts source="))
        .filter_map(|line| {
            let source = field(line, " source=")?;
            let (_, json) = line.split_once(" facts=")?;
            Some((source.to_owned(), serde_json::from_str(json.trim()).ok()?))
        })
        .collect()
}

// The SDK's `accepted` trace line carries each seam's evidence as one JSON
// object at the end of the line, under the source it was mined from.
pub fn accepted(stderr: &str) -> Vec<(String, Evidence)> {
    stderr
        .lines()
        .filter(|line| line.contains(" accepted source="))
        .filter_map(|line| {
            let source = field(line, " source=")?;
            let (_, json) = line.split_once("evidence=")?;
            Some((source.to_owned(), serde_json::from_str(json.trim()).ok()?))
        })
        .collect()
}

// An adapter's `surveyed` trace line carries the surfaces as one JSON array
// at the end of the line, per source.
pub fn surveyed(stderr: &str) -> BTreeMap<String, Vec<Surveyed>> {
    stderr
        .lines()
        .filter(|line| line.contains(" surveyed source="))
        .filter_map(|line| {
            let source = field(line, " source=")?;
            let (_, json) = line.split_once("surfaces=")?;
            Some((source.to_owned(), serde_json::from_str(json.trim()).ok()?))
        })
        .collect()
}

// An adapter's `placed by model` info line carries how many modules no
// surface reaches by code's own count; the SDK's `surveyed by model` line,
// how many the accepted inventory listed as `unreached`. The count is the
// adapter's where it logs one, per source.
pub fn unreached(stderr: &str) -> BTreeMap<String, usize> {
    let count = |mark: &str, key: &str| {
        stderr
            .lines()
            .filter(|line| line.contains(mark))
            .filter_map(|line| {
                Some((field(line, " source=")?.to_owned(), field(line, key)?.parse().ok()?))
            })
            .collect::<BTreeMap<String, usize>>()
    };
    let mut counts = count(" surveyed by model source=", "unreached=");
    counts.extend(count(" placed by model source=", "unplaced="));
    counts
}

// The backend's one line per completion: `label="…"` in the span, then
// `key=value` pairs after the message. The lines before it fill the rest:
// the completion's shell calls, under its `complete{n=…}` span, paired
// start to completion by subject; and the SDK's `responded` lines of the
// turn it ran, `turn=<slice id>` for a build and `turn=verify` for a
// verify, which the outcome line closes, since a wave's verifies never
// overlap.
pub fn completions(stderr: &str) -> Vec<Completion> {
    let mut completions = Vec::new();
    let mut shells: BTreeMap<u64, ShellUse> = BTreeMap::new();
    let mut started: BTreeMap<(u64, String), Vec<u64>> = BTreeMap::new();
    let mut open: BTreeMap<String, (Writes, BTreeMap<String, Option<u64>>)> = BTreeMap::new();
    for line in stderr.lines() {
        if let Some((head, tail)) = line.split_once(": completion outcome=") {
            let label = head
                .split_once("label=\"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map_or("", |(label, _)| label);
            let fields: BTreeMap<&str, &str> = tail
                .split(' ')
                .filter_map(|pair| pair.split_once('='))
                .map(|(key, value)| (key, value.trim_matches('"')))
                .collect();
            let count = |key: &str| {
                fields.get(key).and_then(|value| value.parse::<u64>().ok()).unwrap_or(0)
            };
            let turn = label.strip_prefix("build-").unwrap_or(label);
            completions.push(Completion {
                label: label.to_owned(),
                outcome: tail.split(' ').next().unwrap_or("").trim_matches('"').to_owned(),
                duration: Duration::from_millis(count("duration_ms")),
                tokens: Tokens {
                    input: count("input_tokens"),
                    cached: count("cache_read_tokens"),
                    output: count("output_tokens"),
                    reasoning: count("reasoning_tokens"),
                },
                shell: span_n(head).and_then(|n| shells.remove(&n)).unwrap_or_default(),
                writes: open.remove(turn).map(|(writes, _)| writes).unwrap_or_default(),
            });
        } else if let Some((head, tail)) = line.split_once(" tool call tool=shell") {
            if let (Some(n), Some(at)) = (span_n(head), instant(line)) {
                let subject = field(tail, "subject=").unwrap_or("").to_owned();
                started.entry((n, subject)).or_default().push(at);
            }
        } else if let Some((head, tail)) = line.split_once(" tool call completed tool=shell") {
            if let (Some(n), Some(at)) = (span_n(head), instant(line)) {
                let subject = field(tail, "subject=").unwrap_or("").to_owned();
                let use_ = shells.entry(n).or_default();
                use_.calls += 1;
                use_.bytes +=
                    field(tail, "result_bytes=").and_then(|b| b.parse::<u64>().ok()).unwrap_or(0);
                if let Some(from) = started
                    .get_mut(&(n, subject))
                    .and_then(|starts| (!starts.is_empty()).then(|| starts.remove(0)))
                {
                    use_.time += Duration::from_micros(at.saturating_sub(from));
                }
            }
        } else if let Some((_, tail)) = line.split_once("target::write: responded turn=") {
            let (turn, rest) = tail.split_once(' ').unwrap_or((tail, ""));
            let (writes, sizes) = open.entry(turn.to_owned()).or_default();
            responded(rest, writes, sizes);
        }
    }
    completions
}

// The `n` of the `complete{n=…}` span a backend line sits under.
fn span_n(head: &str) -> Option<u64> {
    field(head, "complete{n=")?.trim_end_matches(|c: char| !c.is_ascii_digit()).parse().ok()
}

// One `responded` line of the SDK's writer, after `turn=<id> `, folded into
// the turn's counts: a `write_files` call (`wrote=Some(["path"=bytes, ..])`,
// or `files=Some(["path", ..]) … bytes=<total>` before the SDK logged each
// file, where a one-file call's total is the file's), an `edit_files` call
// (`edited=Some([..])`), or either refused (`error=Some(..)`).
fn responded(rest: &str, writes: &mut Writes, sizes: &mut BTreeMap<String, Option<u64>>) {
    if rest.contains("error=Some(") {
        writes.refused += 1;
        return;
    }
    if rest.contains("tool=\"edit_files\"") {
        writes.edits += 1;
        return;
    }
    writes.calls += 1;
    for (path, bytes) in written_in(rest) {
        if let Some(last) = sizes.get(&path) {
            writes.rewrites += 1;
            if let (Some(bytes), Some(last)) = (bytes, *last)
                && last > 0
                && bytes * 2 < last
            {
                writes.shrinking += 1;
            }
        }
        if bytes == Some(0) {
            writes.empty += 1;
        }
        sizes.insert(path, bytes);
    }
}

// The files a `write_files` line wrote, each with its bytes where the line
// says: `wrote=Some(["path"=bytes, ..])`, or the earlier `files=Some(["path",
// ..]) … bytes=<total>`, whose total is the file's when the call wrote one.
fn written_in(rest: &str) -> Vec<(String, Option<u64>)> {
    if let Some(list) = between(rest, "wrote=Some([") {
        return list
            .split(", ")
            .filter_map(|entry| {
                let (path, bytes) = entry.trim().rsplit_once("\"=")?;
                Some((path.trim_start_matches('"').to_owned(), bytes.trim().parse().ok()))
            })
            .collect();
    }
    let Some(list) = between(rest, "files=Some([") else {
        return Vec::new();
    };
    let paths: Vec<String> = list
        .split(',')
        .map(|path| path.trim().trim_matches('"').to_owned())
        .filter(|path| !path.is_empty())
        .collect();
    let total = field(rest, "bytes=").and_then(|b| b.parse::<u64>().ok());
    let one = (paths.len() == 1).then_some(total).flatten();
    paths.into_iter().map(|path| (path, one)).collect()
}

// The text between `key` and the `])` closing its list.
fn between<'l>(line: &'l str, key: &str) -> Option<&'l str> {
    let (_, rest) = line.split_once(key)?;
    let end = rest.find("])")?;
    Some(&rest[..end])
}

// The microseconds since the epoch a backend line's leading timestamp
// names, `2026-10-12T07:13:44.434540Z`, for differences within one run.
fn instant(line: &str) -> Option<u64> {
    let stamp = line.split_whitespace().next()?.strip_suffix('Z')?;
    let (date, time) = stamp.split_once('T')?;
    let mut ymd = date.split('-').map(str::parse::<i64>);
    let (year, month, day) = (ymd.next()?.ok()?, ymd.next()?.ok()?, ymd.next()?.ok()?);
    let mut hms = time.split(':');
    let hour: u64 = hms.next()?.parse().ok()?;
    let minute: u64 = hms.next()?.parse().ok()?;
    let seconds = hms.next()?;
    let (second, fraction) = seconds.split_once('.').unwrap_or((seconds, "0"));
    let second: u64 = second.parse().ok()?;
    let micros: u64 = format!("{fraction:0<6}")[..6].parse().ok()?;
    // days from civil, proleptic Gregorian
    let (y, m) = if month <= 2 { (year - 1, month + 9) } else { (year, month - 3) };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days: u64 = (era * 146_097 + doe - 719_468).try_into().ok()?;
    Some((((days * 24 + hour) * 60 + minute) * 60 + second) * 1_000_000 + micros)
}
