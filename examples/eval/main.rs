//! Grades the shipped `typescript` component end to end over the live model.
//! Operator-invoked, never CI.
//!
//! `cargo run --example eval -- [case..]` stages every case under
//! `examples/eval/cases/` (or the named ones) as its own project beneath
//! `target/eval/`, runs `emery specify` over it three times, reads the committed
//! documents back through `emery show`, grades the accepted claims against
//! the case's `expected.toml`, and writes a dated scorecard to `target/eval/`.
//! A case whose fixture the checkout lacks is skipped unless it is named.
//!
//! Environment: `EMERY_BIN` (`../emery/target/release/emery`),
//! `TYPESCRIPT_WASM` (`target/wasm32-wasip2/release/typescript.wasm`),
//! `EVAL_RUNS` (`3`), and the runtime's own `CURSOR_*` knobs. `RUST_LOG` is
//! set for the run unless the caller sets it: the scorecard needs the SDK's
//! `accepted` trace lines and the backend's `completion` lines.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, io};

use emery_sdk::{Anchor, Claim, ClaimKind, Evidence};
use serde::Deserialize;
use serde_json::Value;

const CASES: &str = "examples/eval/cases";
const RUST_LOG: &str = "emery_sdk=trace,omnia_cursor=info,omnia_core=off";

fn main() -> ExitCode {
    match eval() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("eval: {error}");
            ExitCode::FAILURE
        }
    }
}

#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
fn eval() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let settings = Settings::from_env(root)?;
    let filter: BTreeSet<String> = env::args().skip(1).collect();
    let cases = cases(root, &filter)?;
    if cases.is_empty() {
        return Err(format!("no case under `{CASES}` matches").into());
    }

    // run and grade every case
    let started = now();
    let mut reports = Vec::with_capacity(cases.len());
    for case in &cases {
        eprintln!("eval: case `{}`", case.name);
        let project = stage(root, case, &settings)?;
        let mut runs = Vec::with_capacity(settings.runs);
        for n in 1..=settings.runs {
            eprintln!("eval: `{}` run {n}", case.name);
            let run = run(&project, &settings, n)?;
            eprintln!("eval: `{}` run {n} {}", case.name, run.summary());
            runs.push(run);
        }
        reports.push(Report {
            case,
            grades: runs.iter().map(|run| grade(&case.expected, run)).collect(),
            runs,
        });
    }

    // write the scorecard
    let card = scorecard(&settings, &reports, &started);
    let out = root.join("target/eval").join(format!("{}.md", started.replace(':', "-")));
    fs::write(&out, &card)?;
    print!("{card}");
    eprintln!("eval: scorecard at {}", out.display());

    Ok(())
}

struct Settings {
    emery: PathBuf,
    wasm: PathBuf,
    runs: usize,
    model: String,
    rust_log: String,
}

impl Settings {
    fn from_env(root: &Path) -> Result<Self, String> {
        let emery = env::var_os("EMERY_BIN")
            .map_or_else(|| root.join("../emery/target/release/emery"), PathBuf::from);
        let wasm = env::var_os("TYPESCRIPT_WASM").map_or_else(
            || root.join("target/wasm32-wasip2/release/typescript.wasm"),
            PathBuf::from,
        );
        for (name, path) in [("EMERY_BIN", &emery), ("TYPESCRIPT_WASM", &wasm)] {
            if !path.is_file() {
                return Err(format!("{name}: no file at `{}`", path.display()));
            }
        }
        let runs = match env::var("EVAL_RUNS") {
            Ok(runs) => runs.parse().map_err(|error| format!("EVAL_RUNS: `{runs}`: {error}"))?,
            Err(_) => 3,
        };

        Ok(Self {
            emery,
            wasm,
            runs,
            model: env::var("CURSOR_MODEL").unwrap_or_else(|_| "auto".to_owned()),
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| RUST_LOG.to_owned()),
        })
    }
}

// --- cases ---

struct Case {
    name: String,
    expected: Expected,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    // the fixture tree, relative to the repository root
    fixture: String,
    // the stems the accepted requirement claims lead with, exactly
    stems: Vec<String>,
    #[serde(default, rename = "requirement")]
    requirements: Vec<Item>,
    #[serde(default, rename = "criterion")]
    criteria: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    #[serde(default)]
    stem: Option<String>,
    anchor: String,
    gloss: String,
}

fn cases(root: &Path, filter: &BTreeSet<String>) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for entry in fs::read_dir(root.join(CASES))? {
        let dir = entry?.path();
        let Some(name) = dir.file_name().and_then(|name| name.to_str()) else { continue };
        if !dir.is_dir() || (!filter.is_empty() && !filter.contains(name)) {
            continue;
        }
        let expected = fs::read_to_string(dir.join("expected.toml"))
            .map_err(|error| format!("{name}: expected.toml: {error}"))?;
        let expected: Expected =
            toml::from_str(&expected).map_err(|error| format!("{name}: {error}"))?;
        // a fixture the checkout lacks (`r9k` is gitignored) skips its case
        // unless the case was asked for by name
        if !root.join(&expected.fixture).is_dir() {
            if filter.contains(name) {
                return Err(format!("{name}: no fixture at `{}`", expected.fixture).into());
            }
            eprintln!("eval: `{name}` skipped, no fixture at `{}`", expected.fixture);
            continue;
        }
        cases.push(Case {
            name: name.to_owned(),
            expected,
        });
    }
    cases.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(cases)
}

// --- staging ---

// Copies the fixture and the component into a project of their own: `emery`
// mounts its invocation directory, so both must sit inside it, and the
// revision store the run commits under stays with the case.
fn stage(root: &Path, case: &Case, settings: &Settings) -> io::Result<PathBuf> {
    let project = root.join("target/eval").join(&case.name);
    if project.exists() {
        fs::remove_dir_all(&project)?;
    }
    fs::create_dir_all(&project)?;
    copy_tree(&root.join(&case.expected.fixture), &project.join("source"))?;
    fs::copy(&settings.wasm, project.join("typescript.wasm"))?;
    fs::write(
        project.join("emery.toml"),
        "[[source]]\nadapter = \"typescript.wasm\"\npath = \"source/\"\n",
    )?;

    Ok(project)
}

const UNCOPIED: &[&str] = &["node_modules", ".git", ".omnia", "dist"];

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if UNCOPIED.iter().any(|skip| name == *skip) {
            continue;
        }
        let target = to.join(&name);
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    Ok(())
}

// --- one run ---

struct Run {
    n: usize,
    wall: Duration,
    exit: Option<i32>,
    tail: String,
    diff: Option<Value>,
    evidence: Vec<Evidence>,
    completions: Vec<Completion>,
    spec: Option<Value>,
    design: Option<Value>,
    plan: Option<Value>,
}

impl Run {
    fn summary(&self) -> String {
        match self.exit {
            Some(0) => format!(
                "ok in {}s, {} completions, {} claims",
                self.wall.as_secs(),
                self.completions.len(),
                self.evidence.iter().map(|evidence| evidence.claims.len()).sum::<usize>()
            ),
            Some(code) => format!("exit {code} after {}s", self.wall.as_secs()),
            None => format!("killed after {}s", self.wall.as_secs()),
        }
    }

    fn claims(&self, kind: ClaimKind) -> impl Iterator<Item = &Claim> {
        self.evidence
            .iter()
            .flat_map(|evidence| &evidence.claims)
            .filter(move |claim| claim.kind == kind)
    }

    fn stems(&self) -> BTreeSet<&str> {
        self.claims(ClaimKind::Requirement).filter_map(|claim| stem(claim.id.as_deref()?)).collect()
    }

    fn ids(&self) -> BTreeSet<&str> {
        self.claims(ClaimKind::Requirement).filter_map(|claim| claim.id.as_deref()).collect()
    }

    fn requirements(&self) -> &[Value] {
        self.spec
            .as_ref()
            .and_then(|spec| spec["requirements"].as_array())
            .map_or(&[], Vec::as_slice)
    }

    fn subjects(&self) -> BTreeSet<&str> {
        self.requirements()
            .iter()
            .filter_map(|requirement| requirement["subject"].as_str())
            .collect()
    }

    fn status(&self, status: &str) -> usize {
        self.requirements().iter().filter(|requirement| requirement["status"] == status).count()
    }

    fn covered(&self) -> usize {
        self.requirements().iter().filter(|requirement| requirement["covered"] == true).count()
    }

    fn unknown_outcomes(&self) -> usize {
        self.requirements()
            .iter()
            .filter_map(|requirement| requirement["scenarios"].as_array())
            .flatten()
            .filter(|scenario| scenario["then"] == "[unknown]")
            .count()
    }

    fn design_blocks(&self) -> (usize, usize) {
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

    fn slices(&self) -> usize {
        self.plan.as_ref().and_then(|plan| plan["slices"].as_array()).map_or(0, Vec::len)
    }

    fn tokens(&self) -> Tokens {
        self.completions
            .iter()
            .map(|completion| completion.tokens)
            .fold(Tokens::default(), Tokens::add)
    }
}

#[derive(Clone, Copy, Default)]
struct Tokens {
    input: u64,
    cached: u64,
    output: u64,
    reasoning: u64,
}

impl Tokens {
    const fn add(self, other: Self) -> Self {
        Self {
            input: self.input + other.input,
            cached: self.cached + other.cached,
            output: self.output + other.output,
            reasoning: self.reasoning + other.reasoning,
        }
    }
}

struct Completion {
    label: String,
    outcome: String,
    duration: Duration,
    tokens: Tokens,
}

#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
fn run(project: &Path, settings: &Settings, n: usize) -> io::Result<Run> {
    let started = Instant::now();
    let output = emery(project, settings, &["specify", "--config", "emery.toml"])?;
    let wall = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    fs::write(project.join(format!("run-{n}.stderr")), stderr.as_bytes())?;
    fs::write(project.join(format!("run-{n}.stdout")), stdout.as_bytes())?;
    let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    tail.reverse();

    // what the run committed, read back through `show`
    let mut run = Run {
        n,
        wall,
        exit: output.status.code(),
        tail: tail.join("\n"),
        diff: serde_json::from_str::<Value>(&stdout).ok().and_then(|out| out.get("diff").cloned()),
        evidence: accepted(&stderr),
        completions: completions(&stderr),
        spec: None,
        design: None,
        plan: None,
    };
    if run.exit == Some(0) {
        for (artifact, slot) in
            [("spec", &mut run.spec), ("design", &mut run.design), ("plan", &mut run.plan)]
        {
            let shown = emery(project, settings, &["show", artifact])?;
            let envelope: Value = serde_json::from_slice(&shown.stdout)?;
            fs::write(
                project.join(format!("run-{n}.{artifact}.json")),
                serde_json::to_string_pretty(&envelope["document"])?,
            )?;
            *slot = Some(envelope["document"].clone());
        }
        for (index, evidence) in run.evidence.iter().enumerate() {
            fs::write(
                project.join(format!("run-{n}.evidence-{index}.json")),
                serde_json::to_string_pretty(evidence)?,
            )?;
        }
    }

    Ok(run)
}

#[expect(clippy::disallowed_types, reason = "the eval runs the shipped `emery` binary natively")]
fn emery(project: &Path, settings: &Settings, args: &[&str]) -> io::Result<std::process::Output> {
    std::process::Command::new(&settings.emery)
        .arg("--format")
        .arg("json")
        .args(args)
        .current_dir(project)
        .env("NO_COLOR", "1")
        .env("RUST_LOG", &settings.rust_log)
        .output()
}

// The SDK's `accepted` trace line carries each seam's evidence as one JSON
// object at the end of the line.
fn accepted(stderr: &str) -> Vec<Evidence> {
    stderr
        .lines()
        .filter(|line| line.contains(" accepted source="))
        .filter_map(|line| line.split_once("evidence="))
        .filter_map(|(_, json)| serde_json::from_str(json.trim()).ok())
        .collect()
}

// The backend's one line per completion: `label="…"` in the span, then
// `key=value` pairs after the message.
fn completions(stderr: &str) -> Vec<Completion> {
    stderr
        .lines()
        .filter_map(|line| line.split_once(": completion outcome="))
        .map(|(head, tail)| {
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
            Completion {
                label: label.to_owned(),
                outcome: tail.split(' ').next().unwrap_or("").trim_matches('"').to_owned(),
                duration: Duration::from_millis(count("duration_ms")),
                tokens: Tokens {
                    input: count("input_tokens"),
                    cached: count("cache_read_tokens"),
                    output: count("output_tokens"),
                    reasoning: count("reasoning_tokens"),
                },
            }
        })
        .collect()
}

// --- grading ---

struct Report<'a> {
    case: &'a Case,
    runs: Vec<Run>,
    grades: Vec<Grade>,
}

struct Grade {
    requirements: Recall,
    criteria: Recall,
    missing_stems: Vec<String>,
    extra_stems: Vec<String>,
}

#[derive(Default)]
struct Recall {
    matched: usize,
    // of the matched items naming a stem, those met by a claim under it
    placed: usize,
    expected: usize,
    missed: Vec<String>,
    misplaced: Vec<String>,
}

fn grade(expected: &Expected, run: &Run) -> Grade {
    let observed: BTreeSet<&str> = run.stems();
    let wanted: BTreeSet<&str> = expected.stems.iter().map(String::as_str).collect();

    Grade {
        requirements: recall(&expected.requirements, run, ClaimKind::Requirement),
        criteria: recall(&expected.criteria, run, ClaimKind::Criterion),
        missing_stems: wanted.difference(&observed).map(|stem| (*stem).to_owned()).collect(),
        extra_stems: observed.difference(&wanted).map(|stem| (*stem).to_owned()).collect(),
    }
}

// An expected item is met by a claim of its kind whose anchor overlaps its
// own — same file, meeting line ranges, a whole-file anchor meeting any.
// Paraphrase never enters into it; the stem is counted apart, so a survey
// that shapes the estate differently still shows what it mined.
fn recall(items: &[Item], run: &Run, kind: ClaimKind) -> Recall {
    let mut recall = Recall {
        expected: items.len(),
        ..Recall::default()
    };
    for item in items {
        let Ok(wanted) = Anchor::parse(&item.anchor) else {
            recall.missed.push(format!("{} (unparseable anchor)", item.anchor));
            continue;
        };
        let meeting: Vec<&Claim> = run
            .claims(kind)
            .filter(|claim| {
                claim.anchor().and_then(Result::ok).is_some_and(|got| overlaps(got, wanted))
            })
            .collect();
        let label = || {
            let stem = item.stem.as_deref().map_or(String::new(), |stem| format!("{stem} · "));
            format!("{stem}`{}` — {}", item.anchor, item.gloss)
        };
        if meeting.is_empty() {
            recall.missed.push(label());
            continue;
        }
        recall.matched += 1;
        if let Some(stem) = item.stem.as_deref() {
            let placed =
                meeting.iter().any(|claim| claim.id.as_deref().and_then(self::stem) == Some(stem));
            if placed {
                recall.placed += 1;
            } else {
                let under: BTreeSet<&str> = meeting
                    .iter()
                    .filter_map(|claim| claim.id.as_deref().and_then(self::stem))
                    .collect();
                recall.misplaced.push(format!("{} (under {:?})", label(), under));
            }
        }
    }

    recall
}

fn stem(id: &str) -> Option<&str> {
    id.split('.').next().filter(|stem| !stem.is_empty())
}

// Two anchors meet in one file when either names no lines or their ranges
// overlap.
fn overlaps(a: Anchor<'_>, b: Anchor<'_>) -> bool {
    a.path == b.path
        && match (a.lines, b.lines) {
            (Some((a, b)), Some((c, d))) => a <= d && c <= b,
            _ => true,
        }
}

// --- the scorecard ---

fn scorecard(settings: &Settings, reports: &[Report<'_>], started: &str) -> String {
    let mut card = format!(
        "# Eval {started}\n\nmodel `{}` · emery `{}` · adapter `{}` · {} runs per case\n",
        settings.model,
        settings.emery.display(),
        settings.wasm.display(),
        settings.runs
    );
    for report in reports {
        let _ = write!(card, "{}", Card(report));
    }

    card
}

struct Card<'a>(&'a Report<'a>);

impl Display for Card<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Report { case, runs, grades } = self.0;
        writeln!(f, "\n## {}\n\nfixture `{}`\n", case.name, case.expected.fixture)?;

        // one row per run
        writeln!(
            f,
            "| run | exit | wall | completions | input | cached | output | reasoning | claims | \
             req | crit | recall req | recall crit | stems | conflicts | unknown | covered | \
             then [unknown] | slices | design blocks (types) |"
        )?;
        writeln!(f, "|{}", " --- |".repeat(20))?;
        for (run, grade) in runs.iter().zip(grades) {
            row(f, run, grade)?;
        }

        // what each run missed and spent
        for (run, grade) in runs.iter().zip(grades) {
            notes(f, run, grade)?;
        }

        // stability across runs
        if let [first, second, ..] = runs.as_slice()
            && first.exit == Some(0)
            && second.exit == Some(0)
        {
            stability(f, first, second)?;
        }

        Ok(())
    }
}

fn row(f: &mut Formatter<'_>, run: &Run, grade: &Grade) -> fmt::Result {
    let tokens = run.tokens();
    let (blocks, types) = run.design_blocks();
    let stems = if grade.missing_stems.is_empty() && grade.extra_stems.is_empty() {
        "exact".to_owned()
    } else {
        format!("-{} +{}", grade.missing_stems.len(), grade.extra_stems.len())
    };

    writeln!(
        f,
        "| {} | {} | {}s | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | \
         {} | {} ({}) |",
        run.n,
        run.exit.map_or_else(|| "killed".to_owned(), |code| code.to_string()),
        run.wall.as_secs(),
        run.completions.len(),
        tokens.input,
        tokens.cached,
        tokens.output,
        tokens.reasoning,
        run.evidence.iter().map(|evidence| evidence.claims.len()).sum::<usize>(),
        run.claims(ClaimKind::Requirement).count(),
        run.claims(ClaimKind::Criterion).count(),
        grade.requirements,
        grade.criteria,
        stems,
        run.status("conflict"),
        run.status("unknown"),
        run.covered(),
        run.unknown_outcomes(),
        run.slices(),
        blocks,
        types,
    )
}

fn notes(f: &mut Formatter<'_>, run: &Run, grade: &Grade) -> fmt::Result {
    if run.exit != Some(0) {
        return writeln!(f, "\nRun {} failed:\n\n```\n{}\n```", run.n, run.tail);
    }
    writeln!(f, "\nRun {}: stems {:?}", run.n, run.stems())?;
    if !grade.missing_stems.is_empty() {
        writeln!(f, "- stems missing: {:?}", grade.missing_stems)?;
    }
    if !grade.extra_stems.is_empty() {
        writeln!(f, "- stems extra: {:?}", grade.extra_stems)?;
    }
    for (kind, recall) in [("requirement", &grade.requirements), ("criterion", &grade.criteria)] {
        for missed in &recall.missed {
            writeln!(f, "- {kind} missed: {missed}")?;
        }
        for misplaced in &recall.misplaced {
            writeln!(f, "- {kind} misplaced: {misplaced}")?;
        }
    }
    for completion in &run.completions {
        writeln!(
            f,
            "- completion `{}` {} in {}s: {} input, {} cached, {} output ({} reasoning)",
            completion.label,
            completion.outcome,
            completion.duration.as_secs(),
            completion.tokens.input,
            completion.tokens.cached,
            completion.tokens.output,
            completion.tokens.reasoning,
        )?;
    }

    Ok(())
}

fn stability(f: &mut Formatter<'_>, first: &Run, second: &Run) -> fmt::Result {
    writeln!(
        f,
        "\nStability: requirement ids {}, spec subjects {}",
        Jaccard::of(&first.ids(), &second.ids()),
        Jaccard::of(&first.subjects(), &second.subjects()),
    )?;
    if let Some(diff) = &second.diff {
        writeln!(
            f,
            "- run 2 diff vs run 1: spec +{} -{} ~{}, design +{} -{} ~{}, plan +{} -{} ~{}",
            len(&diff["spec"]["added"]),
            len(&diff["spec"]["removed"]),
            len(&diff["spec"]["changed"]),
            len(&diff["design"]["added"]),
            len(&diff["design"]["removed"]),
            len(&diff["design"]["changed"]),
            len(&diff["plan"]["added"]),
            len(&diff["plan"]["removed"]),
            len(&diff["plan"]["changed"]),
        )?;
    }

    Ok(())
}

impl Display for Recall {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.expected == 0 {
            return f.write_str("—");
        }
        write!(f, "{}/{} ({}%)", self.matched, self.expected, self.matched * 100 / self.expected)?;
        if self.placed + self.misplaced.len() > 0 {
            write!(f, ", {} in stem", self.placed)?;
        }
        Ok(())
    }
}

struct Jaccard {
    shared: usize,
    union: usize,
}

impl Jaccard {
    fn of(a: &BTreeSet<&str>, b: &BTreeSet<&str>) -> Self {
        Self {
            shared: a.intersection(b).count(),
            union: a.union(b).count(),
        }
    }
}

impl Display for Jaccard {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.union == 0 {
            return f.write_str("—");
        }
        write!(f, "{}/{} ({}%)", self.shared, self.union, self.shared * 100 / self.union)
    }
}

fn len(value: &Value) -> usize {
    value.as_array().map_or(0, Vec::len)
}

// --- time ---

// The UTC wall clock as `YYYY-MM-DDTHH:MM:SSZ`, for the scorecard's name and title.
#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
fn now() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    let (year, month, day) = civil(days);

    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

// Days since 1970-01-01 to a proleptic Gregorian date (Howard Hinnant's algorithm).
fn civil(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);

    (year, month, day)
}
