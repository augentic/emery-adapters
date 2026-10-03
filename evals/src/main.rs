//! Grades the shipped components end to end over the live model.
//! Operator-invoked, never CI.
//!
//! `cargo run -p evals -- [case|adapter..]` stages every case under
//! `evals/cases/` (or the named ones, and every case of a named adapter) as
//! its own project beneath `target/eval/<card>/`, runs `emery specify` over
//! it three times, reads the committed documents back through `emery show`,
//! grades the accepted claims and the surveyed surfaces against the case's
//! `expected.toml`, and writes the scorecard to `target/eval/<card>.md` —
//! `<card>` the run's UTC start, so every run's files outlive the next
//! invocation and a card can be read back to its anchors. A case names the
//! adapter it runs under (`adapter = "python"`; `typescript` when it names
//! none). Every path — a case's `fixture`, the binaries' defaults and a
//! relative `EMERY_BIN` or `<ADAPTER>_WASM`, the scorecard's — is relative
//! to the repository root. A case whose fixture the checkout lacks is
//! skipped unless it is named.
//!
//! A run that hits the backend's time cap — a `timeout` or `inactive`
//! completion — is put again one rung up the budget ladder, so the card says
//! at which budget it landed; the comparison columns are the first rung's. A
//! run the backend refuses before any claim is accepted — a `bad_gateway` at
//! a turn's opening — is put again once at the same rung, the dead attempt's
//! log kept as `run-N.dead.stderr`. A run that fails after its claims are
//! accepted — in the engine's own turns — is graded on them, and the card
//! names the turn it died in.
//!
//! `cargo run -p evals -- --facts [case|adapter..]` stages the named cases
//! and runs each once with no `CURSOR_API_KEY` in the environment, so the
//! adapter reads the tree and lays its survey facts — the text the model
//! would be asked to name surfaces from — and the first turn fails before
//! any is spent; the facts are printed and written to
//! `target/eval/facts/<case>/facts.md`. A graded run writes the same beside
//! its log as `run-N.facts.md`.
//!
//! Environment: `EMERY_BIN` (`../emery/target/release/emery`), one
//! `<ADAPTER>_WASM` per adapter a selected case runs under —
//! `TYPESCRIPT_WASM` (`target/wasm32-wasip2/release/typescript.wasm`),
//! `PYTHON_WASM` (`target/wasm32-wasip2/release/python.wasm`) — `EVAL_RUNS`
//! (`3`), `EVAL_LADDER` (`600/120,1200/240,2400/480`: each rung
//! `CURSOR_TIMEOUT_SECS/CURSOR_INACTIVITY_SECS`), and the runtime's other
//! `CURSOR_*` knobs. `RUST_LOG` is set for the run unless the caller sets it:
//! the scorecard needs the SDK's `accepted` trace lines and `surveyed by
//! model` line, the adapter's `survey facts` and `surveyed` trace lines and
//! `placed by model` line, and the backend's `completion` lines.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "operator-facing CLI; stdout and stderr are the interface"
)]

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, io};

use emery_sdk::{Anchor, Claim, ClaimKind, Evidence};
use serde::Deserialize;
use serde_json::Value;

const CASES: &str = "evals/cases";
const LADDER: &str = "600/120,1200/240,2400/480";
const DEFAULT_ADAPTER: &str = "typescript";
const BOOTSTRAP: &str = "start";

// The run's log filter unless the caller sets one: the adapter's own crate
// at trace beside the SDK's.
fn rust_log(adapter: &str) -> String {
    format!("emery_sdk=trace,{adapter}=trace,omnia_cursor=info,omnia_core=off")
}

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
    // the crate sits one level beneath the repository root, which every
    // fixture, binary, and scorecard path is relative to
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("CARGO_MANIFEST_DIR: no parent directory")?;
    let mut filter: BTreeSet<String> = env::args().skip(1).collect();
    let facts_only = filter.remove("--facts");
    if facts_only && filter.is_empty() {
        return Err("`--facts` names a case or an adapter".into());
    }
    let cases = cases(root, &filter)?;
    if cases.is_empty() {
        return Err(format!("no case under `{CASES}` matches").into());
    }
    let adapters: BTreeSet<&str> =
        cases.iter().map(|case| case.expected.adapter.as_str()).collect();
    let settings = Settings::from_env(root, &adapters)?;

    let eval_dir = root.join("target/eval");
    if facts_only {
        for case in &cases {
            let project = stage(&eval_dir.join("facts"), case, &settings)?;
            let text = facts(&project, &settings, &case.expected.adapter)?;
            fs::write(project.join("facts.md"), &text)?;
            println!("# {} — survey facts ({})\n\n{text}\n", case.name, case.expected.adapter);
            eprintln!("eval: `{}` facts at {}", case.name, project.join("facts.md").display());
        }
        return Ok(());
    }

    // run and grade every case, each run climbing the ladder while the cap
    // is what fails it
    let started = now();
    let card_name = started.replace(':', "-");
    let mut reports = Vec::with_capacity(cases.len());
    for case in &cases {
        eprintln!("eval: case `{}`", case.name);
        let project = stage(&eval_dir.join(&card_name), case, &settings)?;
        let mut runs = Vec::with_capacity(settings.runs);

        for n in 1..=settings.runs {
            let mut attempts = Vec::new();

            for (index, rung) in settings.ladder.iter().enumerate() {
                eprintln!("eval: `{}` run {n} at {rung}", case.name);
                let tag = tag(n, index, *rung);
                let run = attempt(&project, &settings, case, n, *rung, &tag)?;
                eprintln!("eval: `{}` run {n} at {rung} {}", case.name, run.summary());
                let climb = run.starved() && index + 1 < settings.ladder.len();
                attempts.push(Attempt {
                    grade: grade(&case.expected, &run),
                    run,
                });

                if !climb {
                    break;
                }
            }

            runs.push(attempts);
        }

        reports.push(Report { case, runs });
    }

    // write the scorecard
    let card = scorecard(&settings, &reports, &started, &card_name);
    let out = eval_dir.join(format!("{card_name}.md"));
    fs::write(&out, &card)?;
    print!("{card}");
    eprintln!("eval: scorecard at {}", out.display());

    Ok(())
}

// The name a run's files carry beneath its case: `run-N` at the first rung,
// `run-N@<timeout>-<inactivity>` up the ladder — the rung's `/` as a `-`,
// since the tag is a file name.
fn tag(n: usize, index: usize, rung: Rung) -> String {
    if index == 0 {
        format!("run-{n}")
    } else {
        format!("run-{n}@{}-{}", rung.timeout, rung.inactivity)
    }
}

// One run at one rung, put again once when the backend refused it before
// any claim was accepted: the failure is the host's, not the arm's, and a
// second opening is cheaper than a run short. The dead attempt's files are
// kept as `<tag>.dead.*`.
fn attempt(
    project: &Path, settings: &Settings, case: &Case, n: usize, rung: Rung, tag: &str,
) -> io::Result<Run> {
    let adapter = &case.expected.adapter;
    let first = run(project, settings, adapter, n, rung, tag)?;
    if !first.stillborn() {
        return Ok(first);
    }
    eprintln!("eval: `{}` run {n} at {rung} {} — put again", case.name, first.summary());
    for suffix in ["stderr", "stdout", "facts.md"] {
        let from = project.join(format!("{tag}.{suffix}"));
        if from.exists() {
            fs::rename(from, project.join(format!("{tag}.dead.{suffix}")))?;
        }
    }
    let mut again = run(project, settings, adapter, n, rung, tag)?;
    again.dead = Some(first.summary());

    Ok(again)
}

struct Settings {
    // the repository root, which every fixture path is relative to
    root: PathBuf,
    emery: PathBuf,
    // each adapter a selected case runs under, with its built component
    wasm: BTreeMap<String, PathBuf>,
    runs: usize,
    ladder: Vec<Rung>,
    model: String,
    // the caller's `RUST_LOG`, when set, over the per-adapter default
    rust_log: Option<String>,
}

impl Settings {
    fn from_env(root: &Path, adapters: &BTreeSet<&str>) -> Result<Self, String> {
        // a relative path is from the repository root, the directory the
        // runner is run from; the binary runs with each staged project as
        // its directory, so the path it is spawned by must be absolute
        let given = |name: &str, default: String| {
            root.join(env::var_os(name).map_or_else(|| PathBuf::from(default), PathBuf::from))
        };
        let emery = given("EMERY_BIN", "../emery/target/release/emery".to_owned());
        if !emery.is_file() {
            return Err(format!("EMERY_BIN: no file at `{}`", emery.display()));
        }
        let mut wasm = BTreeMap::new();
        for adapter in adapters {
            let name = format!("{}_WASM", adapter.to_uppercase());
            let path = given(&name, format!("target/wasm32-wasip2/release/{adapter}.wasm"));
            if !path.is_file() {
                return Err(format!("{name}: no file at `{}`", path.display()));
            }
            wasm.insert((*adapter).to_owned(), path);
        }
        let runs = match env::var("EVAL_RUNS") {
            Ok(runs) => runs.parse().map_err(|error| format!("EVAL_RUNS: `{runs}`: {error}"))?,
            Err(_) => 3,
        };
        let ladder = env::var("EVAL_LADDER").unwrap_or_else(|_| LADDER.to_owned());
        let ladder: Vec<Rung> = ladder.split(',').map(Rung::parse).collect::<Result<_, _>>()?;

        Ok(Self {
            root: root.to_path_buf(),
            emery,
            wasm,
            runs,
            ladder,
            model: env::var("CURSOR_MODEL").unwrap_or_else(|_| "auto".to_owned()),
            rust_log: env::var("RUST_LOG").ok(),
        })
    }

    // The component a case's adapter was built to: every selected case's is
    // read at start.
    fn component(&self, adapter: &str) -> io::Result<&Path> {
        self.wasm.get(adapter).map(PathBuf::as_path).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("no component for adapter `{adapter}`"))
        })
    }

    fn rust_log(&self, adapter: &str) -> String {
        self.rust_log.clone().unwrap_or_else(|| rust_log(adapter))
    }
}

// One rung of the budget ladder: the backend's cap per completion and its
// inactivity cut-off, in seconds.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Rung {
    timeout: u64,
    inactivity: u64,
}

impl Rung {
    fn parse(text: &str) -> Result<Self, String> {
        let parsed = text.trim().split_once('/').and_then(|(timeout, inactivity)| {
            Some(Self {
                timeout: timeout.parse().ok()?,
                inactivity: inactivity.parse().ok()?,
            })
        });
        parsed.ok_or_else(|| {
            format!("EVAL_LADDER: `{text}`: a rung is `<timeout>/<inactivity>` in seconds")
        })
    }
}

impl Display for Rung {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.timeout, self.inactivity)
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
    // the adapter the case runs under, by crate name
    #[serde(default = "default_adapter")]
    adapter: String,
    // the fixture tree, relative to the repository root
    fixture: String,
    // the stems the accepted requirement claims lead with, exactly
    stems: Vec<String>,
    // the (entry, stem) pairs the survey decides, exactly
    #[serde(default, rename = "surface")]
    surfaces: Vec<ExpectedSurface>,
    #[serde(default, rename = "requirement")]
    requirements: Vec<Item>,
    #[serde(default, rename = "criterion")]
    criteria: Vec<Item>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedSurface {
    entry: String,
    stem: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Item {
    // the stem — or any one of the stems — a meeting claim's id leads with
    // for the match to count as `in stem`; none grades the anchor alone
    #[serde(default)]
    stem: Option<Stems>,
    anchor: String,
    gloss: String,
}

// One stem, or a list where the code is shared — a guard two routers
// declare, one declared in the bootstrap's module and applied per route —
// and a requirement at it is rightly placed under either.
#[derive(Deserialize)]
#[serde(untagged)]
enum Stems {
    One(String),
    Any(Vec<String>),
}

impl Stems {
    fn holds(&self, stem: &str) -> bool {
        match self {
            Self::One(one) => one == stem,
            Self::Any(any) => any.iter().any(|one| one == stem),
        }
    }
}

impl Display for Stems {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::One(one) => f.write_str(one),
            Self::Any(any) => f.write_str(&any.join("|")),
        }
    }
}

fn default_adapter() -> String {
    DEFAULT_ADAPTER.to_owned()
}

// Every case under `evals/cases/`, or the ones the filter names — a name is
// a case's or an adapter's, which selects every case running under it.
fn cases(root: &Path, filter: &BTreeSet<String>) -> Result<Vec<Case>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();
    for entry in fs::read_dir(root.join(CASES))? {
        let dir = entry?.path();
        let Some(name) = dir.file_name().and_then(|name| name.to_str()) else { continue };
        if !dir.is_dir() {
            continue;
        }
        let expected = fs::read_to_string(dir.join("expected.toml"))
            .map_err(|error| format!("{name}: expected.toml: {error}"))?;
        let expected: Expected =
            toml::from_str(&expected).map_err(|error| format!("{name}: {error}"))?;
        if !filter.is_empty() && !filter.contains(name) && !filter.contains(&expected.adapter) {
            continue;
        }
        // a fixture the checkout lacks (the vendored ones are gitignored until
        // fetched) skips its case unless the case was asked for by name
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
    if cases.is_empty() {
        return Ok(cases);
    }
    let unmatched: Vec<&String> = filter
        .iter()
        .filter(|name| {
            !cases.iter().any(|case| case.name == **name || case.expected.adapter == **name)
        })
        .collect();
    if !unmatched.is_empty() {
        return Err(format!("no case or adapter under `{CASES}` named {unmatched:?}").into());
    }

    Ok(cases)
}

// --- staging ---

// Copies the fixture and the component into a project of their own beneath
// `dir`: `emery` mounts its invocation directory, so both must sit inside
// it, and the revision store the run commits under stays with the case.
fn stage(dir: &Path, case: &Case, settings: &Settings) -> io::Result<PathBuf> {
    let project = dir.join(&case.name);
    if project.exists() {
        fs::remove_dir_all(&project)?;
    }
    fs::create_dir_all(&project)?;
    copy_tree(&settings.root.join(&case.expected.fixture), &project.join("source"))?;
    let adapter = &case.expected.adapter;
    fs::copy(settings.component(adapter)?, project.join(format!("{adapter}.wasm")))?;
    fs::write(
        project.join("emery.toml"),
        format!("[[source]]\nadapter = \"{adapter}.wasm\"\npath = \"source/\"\n"),
    )?;

    Ok(project)
}

// What a fixture's checkout may hold that no run reads: dependencies,
// environments, caches, build output, and the revision store of an earlier
// run; `*.egg-info` by its suffix.
const UNCOPIED: &[&str] = &[
    "node_modules",
    ".git",
    ".omnia",
    "dist",
    "__pycache__",
    ".venv",
    "venv",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
];

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let skipped = UNCOPIED.iter().any(|skip| name == *skip)
            || name.to_str().is_some_and(|name| name.ends_with(".egg-info"));
        if skipped {
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
    rung: Rung,
    wall: Duration,
    exit: Option<i32>,
    tail: String,
    diff: Option<Value>,
    evidence: Vec<Evidence>,
    completions: Vec<Completion>,
    // the surfaces the seams were cut from, as the adapter logged them
    surveyed: Vec<Surveyed>,
    // the modules the survey placed under no surface, by the adapter's own
    // count (`placed by model`), else as the model listed them (the SDK's
    // `surveyed by model` line)
    unreached: Option<usize>,
    spec: Option<Value>,
    design: Option<Value>,
    plan: Option<Value>,
    // the summary of a stillborn attempt this run was put again after
    dead: Option<String>,
}

// One surface as the adapter's `surveyed` line spells it; `lines` is its
// registration or declaration, `L<n>` or `L<n>-L<n>`, where the adapter
// logs one.
#[derive(Deserialize)]
struct Surveyed {
    name: String,
    entry: String,
    stem: String,
    #[serde(default)]
    ids: Vec<String>,
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
    fn summary(&self) -> String {
        match self.exit {
            Some(0) => format!(
                "ok in {}s, {} completions, {} claims",
                self.wall.as_secs(),
                self.completions.len(),
                self.evidence.iter().map(|evidence| evidence.claims.len()).sum::<usize>()
            ),
            Some(code) => {
                let died = self.died_in().map_or(String::new(), |label| format!(" in `{label}`"));
                format!("exit {code}{died} after {}s", self.wall.as_secs())
            }
            None => format!("killed after {}s", self.wall.as_secs()),
        }
    }

    // Whether the backend's cap ended a completion: what the next rung of
    // the ladder is for.
    fn starved(&self) -> bool {
        self.completions
            .iter()
            .any(|completion| matches!(completion.outcome.as_str(), "timeout" | "inactive"))
    }

    // Whether the backend refused the run before any claim was accepted:
    // `bad_gateway` exits 4, and with no evidence there is nothing to grade.
    fn stillborn(&self) -> bool {
        self.exit == Some(4) && self.evidence.is_empty()
    }

    fn passed(&self) -> bool {
        self.exit == Some(0) && !self.starved()
    }

    // The label of the completion a failed run died in: the one the backend
    // or the gate ended, the siblings it aborted set aside.
    fn died_in(&self) -> Option<&str> {
        self.completions
            .iter()
            .rev()
            .find(|completion| !matches!(completion.outcome.as_str(), "ok" | "corrected" | "abort"))
            .or_else(|| self.completions.last())
            .map(|completion| completion.label.as_str())
    }

    // Where the accepted requirement claims anchor against the surfaces the
    // survey decided, the bootstrap set aside — its span is its module or
    // its guard, not a registration or a declaration; none when the adapter
    // logged no surface span.
    fn landing(&self) -> Option<Landing> {
        let spans: Vec<(&str, (u64, u64))> = self
            .surveyed
            .iter()
            .filter(|surface| surface.stem != BOOTSTRAP)
            .filter_map(|surface| Some((surface.entry.as_str(), surface.span()?)))
            .collect();
        if spans.is_empty() {
            return None;
        }
        let mut landing = Landing::default();
        for claim in self.claims(ClaimKind::Requirement) {
            let Some(Ok(anchor)) = claim.anchor() else { continue };
            let Some((start, end)) = anchor.lines else { continue };
            let at = |held: &dyn Fn((u64, u64)) -> bool| {
                spans.iter().any(|(entry, span)| *entry == anchor.path && held(*span))
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

    // The (entry, stem) pairs the survey decided, however many surfaces share one.
    fn pairs(&self) -> BTreeSet<(&str, &str)> {
        self.surveyed
            .iter()
            .map(|surface| (surface.entry.as_str(), surface.stem.as_str()))
            .collect()
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

// Where the accepted requirement claims land against the registered or
// declared surfaces: covering a surface's own first line — the span a
// requirement over a whole handler takes, from its decorator or
// registration — inside a surface's span past that line, or outside every
// surface (the bootstrap's module among the outside). What an arm over the
// anchor set is read by.
#[derive(Debug, Default, PartialEq, Eq)]
struct Landing {
    head: usize,
    inside: usize,
    outside: usize,
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
fn run(
    project: &Path, settings: &Settings, adapter: &str, n: usize, rung: Rung, tag: &str,
) -> io::Result<Run> {
    let started = Instant::now();
    let rust_log = settings.rust_log(adapter);
    let output = emery(project, settings, &rust_log, rung, SPECIFY, Turns::Live)?;
    let wall = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    fs::write(project.join(format!("{tag}.stderr")), stderr.as_bytes())?;
    fs::write(project.join(format!("{tag}.stdout")), stdout.as_bytes())?;
    if let Some(facts) = survey_facts(&stderr) {
        fs::write(project.join(format!("{tag}.facts.md")), facts)?;
    }
    let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    tail.reverse();

    // what the run committed, read back through `show`
    let mut run = Run {
        n,
        rung,
        wall,
        exit: output.status.code(),
        tail: tail.join("\n"),
        diff: serde_json::from_str::<Value>(&stdout).ok().and_then(|out| out.get("diff").cloned()),
        evidence: accepted(&stderr),
        completions: completions(&stderr),
        surveyed: surveyed(&stderr).unwrap_or_default(),
        unreached: unreached(&stderr),
        spec: None,
        design: None,
        plan: None,
        dead: None,
    };
    if run.exit == Some(0) {
        for (artifact, slot) in
            [("spec", &mut run.spec), ("design", &mut run.design), ("plan", &mut run.plan)]
        {
            let shown =
                emery(project, settings, &rust_log, rung, &["show", artifact], Turns::Live)?;
            let envelope: Value = serde_json::from_slice(&shown.stdout)?;
            fs::write(
                project.join(format!("{tag}.{artifact}.json")),
                serde_json::to_string_pretty(&envelope["document"])?,
            )?;
            *slot = Some(envelope["document"].clone());
        }
    }
    // the claims are graded whether or not the engine's turns landed after them
    for (index, evidence) in run.evidence.iter().enumerate() {
        fs::write(
            project.join(format!("{tag}.evidence-{index}.json")),
            serde_json::to_string_pretty(evidence)?,
        )?;
    }

    Ok(run)
}

const SPECIFY: &[&str] = &["specify", "--config", "emery.toml"];

// Whether a run may put a turn to the model: `None` strips `CURSOR_API_KEY`
// from the binary's environment, so the backend refuses the first turn
// after the adapter has read the tree and logged its facts.
#[derive(Clone, Copy)]
enum Turns {
    Live,
    None,
}

#[expect(clippy::disallowed_types, reason = "the eval runs the shipped `emery` binary natively")]
fn emery(
    project: &Path, settings: &Settings, rust_log: &str, rung: Rung, args: &[&str], turns: Turns,
) -> io::Result<std::process::Output> {
    let mut command = std::process::Command::new(&settings.emery);
    command
        .arg("--format")
        .arg("json")
        .args(args)
        .current_dir(project)
        .env("NO_COLOR", "1")
        .env("RUST_LOG", rust_log)
        .env("CURSOR_TIMEOUT_SECS", rung.timeout.to_string())
        .env("CURSOR_INACTIVITY_SECS", rung.inactivity.to_string());
    if matches!(turns, Turns::None) {
        command.env_remove("CURSOR_API_KEY");
    }
    command.output()
}

// The survey facts a staged project's adapter lays, read from one run that
// spends no turn: the run fails at the first completion, after the facts
// are logged.
fn facts(project: &Path, settings: &Settings, adapter: &str) -> io::Result<String> {
    let rust_log = settings.rust_log(adapter);
    let rung =
        settings.ladder.first().copied().ok_or_else(|| io::Error::other("EVAL_LADDER: no rung"))?;
    let output = emery(project, settings, &rust_log, rung, SPECIFY, Turns::None)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    fs::write(project.join("facts.stderr"), stderr.as_bytes())?;
    survey_facts(&stderr).ok_or_else(|| {
        let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
        tail.reverse();
        io::Error::other(format!(
            "no `survey facts` line from adapter `{adapter}` (does it survey? is `{adapter}=trace` \
             in RUST_LOG?); the run ended:\n{}",
            tail.join("\n")
        ))
    })
}

// The adapter's `survey facts` trace line carries the text laid before the
// model as one JSON string at the end of the line.
fn survey_facts(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .filter(|line| line.contains(" survey facts source="))
        .filter_map(|line| line.split_once(" facts="))
        .find_map(|(_, json)| serde_json::from_str(json.trim()).ok())
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

// The adapter's `surveyed` trace line carries the surfaces as one JSON array
// at the end of the line.
fn surveyed(stderr: &str) -> Option<Vec<Surveyed>> {
    stderr
        .lines()
        .filter(|line| line.contains(" surveyed source="))
        .filter_map(|line| line.split_once("surfaces="))
        .find_map(|(_, json)| serde_json::from_str(json.trim()).ok())
}

// The adapter's `placed by model` info line carries how many modules no
// surface reaches by code's own count; the SDK's `surveyed by model` line,
// how many the accepted inventory listed as `unreached`. The count is the
// adapter's where it logs one.
fn unreached(stderr: &str) -> Option<usize> {
    let count = |mark: &str, key: &str| {
        stderr
            .lines()
            .filter(|line| line.contains(mark))
            .filter_map(|line| line.split_once(key))
            .find_map(|(_, rest)| rest.split_whitespace().next()?.parse().ok())
    };
    count(" placed by model source=", "unplaced=")
        .or_else(|| count(" surveyed by model source=", "unreached="))
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
    // one entry per run: its attempts up the ladder, the first rung's first
    runs: Vec<Vec<Attempt>>,
}

struct Attempt {
    run: Run,
    grade: Grade,
}

struct Grade {
    requirements: Recall,
    criteria: Recall,
    surfaces: Surfaces,
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

// The (entry, stem) pairs the survey decided against the ones expected: the
// pairs met, the entries found under another stem, the pairs met by nothing
// at their entry, and the surveyed surfaces no expected entry accounts for.
#[derive(Default)]
struct Surfaces {
    matched: usize,
    expected: usize,
    restemmed: Vec<String>,
    missed: Vec<String>,
    extra: Vec<String>,
}

fn grade(expected: &Expected, run: &Run) -> Grade {
    let observed: BTreeSet<&str> = run.stems();
    let wanted: BTreeSet<&str> = expected.stems.iter().map(String::as_str).collect();

    Grade {
        requirements: recall(&expected.requirements, run, ClaimKind::Requirement),
        criteria: recall(&expected.criteria, run, ClaimKind::Criterion),
        surfaces: surfaces(&expected.surfaces, run),
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
            let stem = item.stem.as_ref().map_or(String::new(), |stems| format!("{stems} · "));
            format!("{stem}`{}` — {}", item.anchor, item.gloss)
        };
        if meeting.is_empty() {
            recall.missed.push(label());
            continue;
        }
        recall.matched += 1;
        if let Some(stems) = &item.stem {
            let placed = meeting.iter().any(|claim| {
                claim.id.as_deref().and_then(self::stem).is_some_and(|s| stems.holds(s))
            });
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

// An expected surface is met when the survey decided a surface at its entry
// under its stem; one the survey decided at the entry under another stem is
// restemmed — found, and stemmed differently, which is how a survey that is
// present but wrong reads apart from one that is absent. The names are the
// survey's own and never graded.
fn surfaces(expected: &[ExpectedSurface], run: &Run) -> Surfaces {
    let wanted: BTreeSet<(&str, &str)> =
        expected.iter().map(|surface| (surface.entry.as_str(), surface.stem.as_str())).collect();
    let observed = run.pairs();
    let label = |(entry, stem): &(&str, &str)| format!("`{entry}` · {stem}");

    let mut open: Vec<(&str, &str)> = observed.difference(&wanted).copied().collect();
    let mut restemmed = Vec::new();
    let mut missed = Vec::new();
    for pair in wanted.difference(&observed) {
        match open.iter().position(|(entry, _)| *entry == pair.0) {
            Some(at) => {
                let (_, got) = open.remove(at);
                restemmed.push(format!("{} (as {got})", label(pair)));
            }
            None => missed.push(label(pair)),
        }
    }

    Surfaces {
        matched: wanted.intersection(&observed).count(),
        expected: wanted.len(),
        restemmed,
        missed,
        extra: open.iter().map(label).collect(),
    }
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

fn scorecard(settings: &Settings, reports: &[Report<'_>], started: &str, name: &str) -> String {
    let ladder: Vec<String> = settings.ladder.iter().map(Rung::to_string).collect();
    let adapters: Vec<String> = settings
        .wasm
        .iter()
        .map(|(adapter, path)| format!("adapter `{adapter}` `{}`", path.display()))
        .collect();
    let mut card = format!(
        "# Eval {started}\n\nmodel `{}` · emery `{}` · {} · {} runs per case · ladder {} \
         (`CURSOR_TIMEOUT_SECS`/`CURSOR_INACTIVITY_SECS`, climbed on a `timeout` or `inactive` \
         completion; the columns are the first rung's) · runs under `target/eval/{name}/`\n",
        settings.model,
        settings.emery.display(),
        adapters.join(" · "),
        settings.runs,
        ladder.join(" → "),
    );
    for report in reports {
        let _ = write!(
            card,
            "{}",
            Card {
                report,
                ladder: &settings.ladder,
            }
        );
    }

    card
}

struct Card<'a> {
    report: &'a Report<'a>,
    ladder: &'a [Rung],
}

impl Display for Card<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Report { case, runs } = self.report;
        writeln!(
            f,
            "\n## {}\n\nadapter `{}` · fixture `{}`\n",
            case.name, case.expected.adapter, case.expected.fixture
        )?;

        // one row per run, at the first rung
        writeln!(
            f,
            "| run | exit | wall | completions | input | cached | output | reasoning | claims | \
             req | crit | recall req | recall crit | surfaces | stems | conflicts | unknown | \
             covered | then [unknown] | slices | design blocks (types) |"
        )?;
        writeln!(f, "|{}", " --- |".repeat(21))?;
        for attempts in runs {
            if let Some(first) = attempts.first() {
                row(f, first)?;
            }
        }

        // what each attempt missed and spent
        for attempts in runs {
            for (index, attempt) in attempts.iter().enumerate() {
                notes(f, attempt, index > 0)?;
            }
        }

        // where each run landed on the ladder
        if self.ladder.len() > 1 {
            ladder(f, runs, self.ladder)?;
        }

        // stability across runs: at the first rung when two landed there,
        // else across the attempts that landed anywhere
        let first: Vec<&Run> = runs
            .iter()
            .filter_map(|attempts| attempts.first())
            .map(|attempt| &attempt.run)
            .collect();
        let landed: Vec<&Run> = first.iter().copied().filter(|run| run.exit == Some(0)).collect();
        if landed.len() >= 2 {
            let label = format!("across {} runs at {}", landed.len(), self.ladder[0]);
            stability(f, &label, &landed)?;
        } else {
            let landed: Vec<&Run> = runs
                .iter()
                .filter_map(|attempts| attempts.last())
                .map(|attempt| &attempt.run)
                .filter(|run| run.exit == Some(0))
                .collect();
            if landed.len() >= 2 {
                let label = format!("across the {} attempts that landed", landed.len());
                stability(f, &label, &landed)?;
            }
        }

        Ok(())
    }
}

fn row(f: &mut Formatter<'_>, attempt: &Attempt) -> fmt::Result {
    let Attempt { run, grade } = attempt;
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
         {} | {} | {} ({}) |",
        run.n,
        match (run.exit, run.died_in()) {
            (Some(0), _) => "0".to_owned(),
            (Some(code), Some(label)) => format!("{code} in `{label}`"),
            (Some(code), None) => code.to_string(),
            (None, _) => "killed".to_owned(),
        },
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
        grade.surfaces,
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

fn notes(f: &mut Formatter<'_>, attempt: &Attempt, climbed: bool) -> fmt::Result {
    let Attempt { run, grade } = attempt;
    let at = if climbed { format!(" @ {}", run.rung) } else { String::new() };
    let graded = !run.evidence.is_empty();
    if run.exit == Some(0) {
        writeln!(f, "\nRun {}{at}: stems {:?}", run.n, run.stems())?;
    } else {
        // the turn the run died in, and whether it left claims to grade
        let died = run.died_in().map_or(String::new(), |label| format!(" in `{label}`"));
        let left = if graded {
            format!(", {} seams accepted — the claims are graded below", run.evidence.len())
        } else {
            ", no claim accepted".to_owned()
        };
        writeln!(f, "\nRun {}{at} failed{died}{left}:\n\n```\n{}\n```", run.n, run.tail)?;
    }
    if let Some(dead) = &run.dead {
        writeln!(f, "- put again once: the first attempt {dead}, before any claim")?;
    }

    // the survey
    if !run.surveyed.is_empty() {
        let listed: Vec<String> = run
            .surveyed
            .iter()
            .map(|surface| {
                let ids: Vec<String> = surface.ids.iter().map(|id| format!("`{id}`")).collect();
                let led = if ids.is_empty() {
                    String::new()
                } else {
                    format!(" (id {})", ids.join(", "))
                };
                format!("`{}` @ `{}` as `{}`{led}", surface.name, surface.entry, surface.stem)
            })
            .collect();
        writeln!(f, "- surfaces: {}", listed.join(", "))?;
    }
    if let Some(unreached) = run.unreached {
        writeln!(f, "- unreached: {unreached} modules the survey placed under no surface")?;
    }
    for restemmed in &grade.surfaces.restemmed {
        writeln!(f, "- surface restemmed: {restemmed}")?;
    }
    for missed in &grade.surfaces.missed {
        writeln!(f, "- surface missed: {missed}")?;
    }
    if grade.surfaces.expected > 0 {
        for extra in &grade.surfaces.extra {
            writeln!(f, "- surface extra: {extra}")?;
        }
    }
    // the claims
    if graded {
        if let Some(landing) = run.landing() {
            writeln!(f, "- requirement anchors: {landing}")?;
        }
        if !grade.missing_stems.is_empty() {
            writeln!(f, "- stems missing: {:?}", grade.missing_stems)?;
        }
        if !grade.extra_stems.is_empty() {
            writeln!(f, "- stems extra: {:?}", grade.extra_stems)?;
        }
        for (kind, recall) in [("requirement", &grade.requirements), ("criterion", &grade.criteria)]
        {
            for missed in &recall.missed {
                writeln!(f, "- {kind} missed: {missed}")?;
            }
            for misplaced in &recall.misplaced {
                writeln!(f, "- {kind} misplaced: {misplaced}")?;
            }
        }
    }

    // the completions
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

// One row per run: pass or fail at each rung attempted, `—` past the rung
// it landed on.
fn ladder(f: &mut Formatter<'_>, runs: &[Vec<Attempt>], ladder: &[Rung]) -> fmt::Result {
    writeln!(
        f,
        "\nBudget ladder (`CURSOR_TIMEOUT_SECS`/`CURSOR_INACTIVITY_SECS`), climbed on a `timeout` \
         or `inactive` completion:\n"
    )?;
    let heads: Vec<String> = ladder.iter().map(Rung::to_string).collect();
    writeln!(f, "| run | {} |", heads.join(" | "))?;
    writeln!(f, "|{}", " --- |".repeat(ladder.len() + 1))?;
    for attempts in runs {
        let Some(first) = attempts.first() else { continue };
        let cells: Vec<String> = (0..ladder.len())
            .map(|index| attempts.get(index).map_or_else(|| "—".to_owned(), verdict))
            .collect();
        writeln!(f, "| {} | {} |", first.run.n, cells.join(" | "))?;
    }

    Ok(())
}

fn verdict(attempt: &Attempt) -> String {
    let Attempt { run, grade } = attempt;
    let wall = run.wall.as_secs();
    if run.passed() {
        return format!(
            "pass (req {}/{}, {wall}s)",
            grade.requirements.matched, grade.requirements.expected
        );
    }
    let starved: Vec<String> = ["timeout", "inactive"]
        .into_iter()
        .filter_map(|outcome| {
            let count = run.completions.iter().filter(|c| c.outcome == outcome).count();
            (count > 0).then(|| format!("{count} {outcome}"))
        })
        .collect();
    let why = if starved.is_empty() {
        let failed: BTreeSet<&str> = run
            .completions
            .iter()
            .filter(|c| !matches!(c.outcome.as_str(), "ok" | "corrected" | "abort"))
            .map(|c| c.outcome.as_str())
            .collect();
        let died = run.died_in().map_or(String::new(), |label| format!(" in `{label}`"));
        match (run.exit, failed.is_empty()) {
            (Some(code), true) => format!("exit {code}{died}"),
            (Some(code), false) => {
                format!("exit {code}, {}{died}", Vec::from_iter(failed).join(", "))
            }
            (None, _) => "killed".to_owned(),
        }
    } else {
        starved.join(", ")
    };

    format!("fail: {why} ({wall}s)")
}

fn stability(f: &mut Formatter<'_>, label: &str, runs: &[&Run]) -> fmt::Result {
    writeln!(f, "\nStability {label}:")?;

    // the stems each run's requirements lead with, as sets
    let sets: BTreeSet<BTreeSet<&str>> = runs.iter().map(|run| run.stems()).collect();
    if sets.len() == 1 {
        writeln!(f, "- stem sets: 1 distinct")?;
    } else {
        writeln!(f, "- stem sets: {} distinct — {:?}", sets.len(), sets)?;
    }

    // every pair of runs
    pairwise(f, "surfaces (entry, stem)", runs, Run::pairs)?;
    pairwise(f, "requirement ids", runs, Run::ids)?;
    pairwise(f, "spec subjects", runs, Run::subjects)?;
    anchored(f, runs)?;

    // what each later revision changed
    for run in runs.iter().skip(1) {
        if let Some(diff) = &run.diff {
            writeln!(
                f,
                "- run {} diff vs the previous revision: spec +{} -{} ~{}, design +{} -{} ~{}, plan \
                 +{} -{} ~{}",
                run.n,
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
    }

    Ok(())
}

// The Jaccard index of `of` over every pair of runs, as one line.
fn pairwise<'r, T: Ord>(
    f: &mut Formatter<'_>, label: &str, runs: &[&'r Run], of: impl Fn(&'r Run) -> BTreeSet<T>,
) -> fmt::Result {
    let sets: Vec<BTreeSet<T>> = runs.iter().map(|run| of(run)).collect();
    let mut cells = Vec::new();
    for (left, first) in sets.iter().enumerate() {
        for (right, second) in sets.iter().enumerate().skip(left + 1) {
            cells.push(format!(
                "{}↔{} {}",
                runs[left].n,
                runs[right].n,
                Jaccard::of(first, second)
            ));
        }
    }
    writeln!(f, "- {label}: {}", cells.join(" · "))
}

// The requirements every pair of runs anchors alike, as one line in the
// `pairwise` shape: the pairs the engine's re-mine diff would match — the
// same stem and a cited `path` in common, one to one, the pair sharing the
// most anchors first — over the requirements the two revisions hold together.
// Reads the `sources[].path` that `emery show --format json` carries from
// grammar `4`; a revision stored under an earlier grammar matches none.
fn anchored(f: &mut Formatter<'_>, runs: &[&Run]) -> fmt::Result {
    let mut cells = Vec::new();
    for (left, first) in runs.iter().enumerate() {
        for second in runs.iter().skip(left + 1) {
            let shared = matched(first.requirements(), second.requirements());
            let union = first.requirements().len() + second.requirements().len() - shared;
            cells.push(format!("{}↔{} {}", first.n, second.n, Jaccard { shared, union }));
        }
    }
    writeln!(f, "- anchor-matched requirements: {}", cells.join(" · "))
}

// How many one-to-one pairs of `outgoing` and `incoming` requirements share a
// stem and cite an anchor in common: every pair scored by the anchors it
// shares, the highest taken first, ties in position order — the engine's
// pairing, less its fallback to id.
fn matched(outgoing: &[Value], incoming: &[Value]) -> usize {
    let mut scored: Vec<(usize, usize, usize)> = Vec::new();
    for (out, before) in outgoing.iter().enumerate() {
        let Some(under) = before["subject"].as_str().and_then(stem) else { continue };
        for (into, after) in incoming.iter().enumerate() {
            if after["subject"].as_str().and_then(stem) != Some(under) {
                continue;
            }
            let shared = citations(before)
                .flat_map(|cited| citations(after).filter(move |other| cited_alike(cited, *other)))
                .count();
            if shared > 0 {
                scored.push((shared, out, into));
            }
        }
    }
    scored.sort_by_key(|&(shared, out, into)| (Reverse(shared), out, into));
    let mut outs = BTreeSet::new();
    let mut intos = BTreeSet::new();
    for (_, out, into) in scored {
        if !outs.contains(&out) && !intos.contains(&into) {
            outs.insert(out);
            intos.insert(into);
        }
    }
    outs.len()
}

// A requirement's citations that anchor somewhere: the source and the parsed
// `path`, as `emery show --format json` spells them.
fn citations(requirement: &Value) -> impl Iterator<Item = (&str, Anchor<'_>)> {
    requirement["sources"].as_array().into_iter().flatten().filter_map(|cited| {
        let source = cited["source"].as_str()?;
        let anchor = Anchor::parse(cited["path"].as_str()?).ok()?;
        Some((source, anchor))
    })
}

// Two citations anchor at one place when they come from the same source and
// name the same file, with line ranges that meet, or neither naming lines —
// the engine's rule, stricter than `overlaps`, which lets a whole-file anchor
// meet any.
fn cited_alike((source, a): (&str, Anchor<'_>), (other, b): (&str, Anchor<'_>)) -> bool {
    source == other
        && a.path == b.path
        && match (a.lines, b.lines) {
            (Some((start, end)), Some((other_start, other_end))) => {
                start <= other_end && other_start <= end
            }
            (None, None) => true,
            _ => false,
        }
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

impl Display for Surfaces {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.expected == 0 {
            return f.write_str("—");
        }
        write!(f, "{}/{}", self.matched, self.expected)?;
        if !self.restemmed.is_empty() {
            write!(f, " ~{}", self.restemmed.len())?;
        }
        if !self.extra.is_empty() {
            write!(f, " +{}", self.extra.len())?;
        }
        Ok(())
    }
}

struct Jaccard {
    shared: usize,
    union: usize,
}

impl Jaccard {
    fn of<T: Ord>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    const RUNG: Rung = Rung {
        timeout: 600,
        inactivity: 120,
    };

    // A run read from the lines the runner parses, as the backend and the
    // SDK log them.
    fn run_from(exit: Option<i32>, stderr: &str) -> Run {
        Run {
            n: 1,
            rung: RUNG,
            wall: Duration::ZERO,
            exit,
            tail: String::new(),
            diff: None,
            evidence: accepted(stderr),
            completions: completions(stderr),
            surveyed: surveyed(stderr).unwrap_or_default(),
            unreached: None,
            spec: None,
            design: None,
            plan: None,
            dead: None,
        }
    }

    fn completion(label: &str, outcome: &str) -> String {
        format!(
            "INFO complete{{n=1 model=\"m\" format=schema label=\"{label}\"}}: \
             omnia_cursor::model::observe: completion outcome=\"{outcome}\" attempts=1 \
             duration_ms=10 input_tokens=5 cache_read_tokens=0 output_tokens=2 \
             reasoning_tokens=0\n"
        )
    }

    fn accepted_line(claims: &str) -> String {
        format!("TRACE emery_sdk: accepted source=python evidence={{\"claims\":[{claims}]}}\n")
    }

    fn requirement(id: &str, path: &str) -> String {
        format!(
            "{{\"kind\":\"requirement\",\"id\":\"{id}\",\"path\":\"{path}\",\"statement\":\"s\"}}"
        )
    }

    #[test]
    fn first_rung_tag() {
        assert_eq!(tag(2, 0, RUNG), "run-2");
    }

    #[test]
    fn climbed_tag() {
        let rung = Rung {
            timeout: 1200,
            inactivity: 240,
        };
        let tag = tag(1, 1, rung);
        assert_eq!(tag, "run-1@1200-240");
        assert!(!tag.contains('/'), "a tag names files beneath the case");
    }

    #[test]
    fn ladder_rungs() {
        let ladder: Vec<Rung> =
            LADDER.split(',').map(Rung::parse).collect::<Result<_, _>>().unwrap();
        assert_eq!(ladder.len(), 3);
        assert_eq!(ladder[1].to_string(), "1200/240");
        assert!(Rung::parse("1200").is_err());
        assert!(Rung::parse("a/b").is_err());
    }

    #[test]
    fn starved_run() {
        let starved = run_from(
            Some(3),
            &(completion("survey-python", "ok") + &completion("evidence-python-0", "timeout")),
        );
        assert!(starved.starved());
        assert!(!starved.passed());
        let landed = run_from(Some(0), &completion("evidence-python-0", "ok"));
        assert!(!landed.starved());
        assert!(landed.passed());
    }

    #[test]
    fn stillborn_run() {
        let refused = run_from(Some(4), &completion("survey-python", "error"));
        assert!(refused.stillborn());
        assert_eq!(refused.died_in(), Some("survey-python"));

        let after_claims = run_from(
            Some(4),
            &(completion("evidence-python-0", "ok")
                + &accepted_line(&requirement("orders.create", "app.py#L3-L9"))
                + &completion("grouping", "error")),
        );
        assert!(!after_claims.stillborn(), "a claim accepted is a run to grade");
        assert_eq!(after_claims.died_in(), Some("grouping"));

        let engine = run_from(Some(1), &completion("survey-python", "ok"));
        assert!(!engine.stillborn(), "only the backend's refusal is put again");
    }

    #[test]
    fn died_in_engine_turn() {
        let stderr = completion("survey-python", "ok")
            + &completion("evidence-python-0", "corrected")
            + &completion("spec-draft", "ok")
            + &completion("spec-draft", "exhausted")
            + &completion("design-draft", "abort")
            + &completion("slicing", "abort");
        let run = run_from(Some(1), &stderr);
        assert_eq!(run.died_in(), Some("spec-draft"), "the aborted siblings are not where it died");
    }

    #[test]
    fn anchor_landing() {
        let surveyed = "TRACE python: surveyed source=python surfaces=[\
            {\"name\":\"GET /items\",\"entry\":\"app/items.py\",\"stem\":\"items\",\"ids\":[\"items.get\"],\"lines\":\"L10-L30\"},\
            {\"name\":\"start\",\"entry\":\"app/main.py\",\"stem\":\"start\",\"ids\":[\"start\"],\"lines\":\"L5\"}]\n";
        let claims = [
            requirement("items.get.whole", "app/items.py#L10-L30"),
            requirement("items.get.decorator", "app/items.py#L8-L11"),
            requirement("items.get.guard", "app/items.py#L14-L16"),
            requirement("items.get.last", "app/items.py#L30"),
            requirement("items.get.helper", "app/items.py#L40-L42"),
            requirement("start.boot", "app/main.py#L5"),
            requirement("start.config", "app/config.py#L1-L3"),
        ]
        .join(",");
        let run = run_from(Some(0), &(surveyed.to_owned() + &accepted_line(&claims)));
        assert_eq!(
            run.landing(),
            Some(Landing {
                head: 2,
                inside: 2,
                outside: 3
            }),
            "the bootstrap's span is set aside, so `start.boot` at its line is outside"
        );

        let unlined = run_from(Some(0), &accepted_line(&claims));
        assert_eq!(unlined.landing(), None, "no surface span logged, nothing to land against");
    }
}
