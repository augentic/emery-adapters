//! Grades the shipped components end to end over the live model.
//! Operator-invoked, never CI.
//!
//! `cargo run -p evals -- [case|adapter..]` stages every case under
//! `evals/cases/` (or the named ones, and every case of a named adapter) as
//! its own project beneath `target/eval/<card>/`, runs `emery specify` over
//! it three times, reads the committed documents back through `emery show`,
//! grades the accepted claims, the surveyed surfaces, and the reconciled
//! requirements against the case's `expected.toml`, and writes the scorecard
//! to `target/eval/<card>.md` — `<card>` the run's UTC start, so every run's
//! files outlive the next invocation and a card can be read back to its
//! anchors. A case names one `[[source]]` or several, each under the adapter
//! it runs (`adapter = "python"`) over a `fixture` tree or an inline
//! `description`; a case of several sources grades the specification's
//! `[[behaviour]]`s — which requirements the engine reconciled from which
//! sources, under which status. Every path — a source's `fixture`, the
//! binaries' defaults and a relative `EMERY_BIN` or `<ADAPTER>_WASM`, the
//! scorecard's — is relative to the repository root. A case whose fixture
//! the checkout lacks is skipped unless it is named.
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
//! and runs each once with no `CURSOR_API_KEY` in the environment, so each
//! adapter reads its tree and lays its survey facts — the text the model
//! would be asked to name surfaces from — and the first turn fails before
//! any is spent; the facts are printed and written to
//! `target/eval/facts/<case>/facts.md`. A graded run writes the same beside
//! its log as `run-N.facts.md`.
//!
//! `cargo run -p evals -- --build [case|adapter..]` builds each plan a run
//! commits: the project gains a `[target]` table naming the `typescript-target`
//! component and a greenfield repository the runner lays beside the sources,
//! `target.git`, bare, its `main` one root commit; after a specify that
//! landed and its `show`s, `emery build --config emery.toml` runs over it,
//! its log and envelope kept as `run-N.build.stderr` / `.stdout`, and the
//! card gains five columns — what the build merged, verified, took, saved
//! over a serial walk of the same turns, and covered — with the waves, the
//! conflicts, the verify's refusals, the uncovered ids, and the chain the
//! repository holds beneath the run's notes.
//!
//! Environment: `EMERY_BIN` (`../emery/target/release/emery`), one
//! `<ADAPTER>_WASM` per adapter a selected case runs under —
//! `TYPESCRIPT_WASM` (`target/wasm32-wasip2/release/typescript.wasm`),
//! `PYTHON_WASM`, `DOCUMENTATION_WASM`, `INTENT_WASM`, and under `--build`
//! `TYPESCRIPT_TARGET_WASM` (`typescript_target.wasm`, as cargo spells the
//! crate) — each copied into the binary's store, `~/.emery/adapters`, as the
//! package `eval:<adapter>@<version>` (the file `eval_<adapter>@<version>.wasm`,
//! `<version>` this workspace's), which every staged project names, so a
//! card never stands over a published release or a developer's own build
//! and `rm ~/.emery/adapters/eval_*` is the cleanup — `EVAL_RUNS` (`3`),
//! `EVAL_JOBS` (unset: the width cap `--build` passes as `--jobs`),
//! `EVAL_LADDER` (`600/120,1200/240,2400/480`: each rung
//! `CURSOR_TIMEOUT_SECS/CURSOR_INACTIVITY_SECS`), and the runtime's other
//! `CURSOR_*` knobs. `RUST_LOG` is set for the run unless the caller sets it:
//! the scorecard needs the SDK's `accepted` trace lines and `surveyed by
//! model` line, the adapters' `survey facts` and `surveyed` trace lines and
//! `placed by model` line, and the backend's `completion` lines.

#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "operator-facing CLI; stdout and stderr are the interface"
)]

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};
use std::num::{NonZeroU32, NonZeroUsize};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{env, fs, io};

use emery_sdk::{Anchor, Claim, ClaimKind, Evidence};
use serde::Deserialize;
use serde_json::Value;

const CASES: &str = "evals/cases";
const LADDER: &str = "600/120,1200/240,2400/480";
const BOOTSTRAP: &str = "start";
// The target adapter a `--build` run builds through, by its crate's name.
const TARGET: &str = "typescript-target";

// The run's log filter unless the caller sets one: every adapter the case
// runs under at trace beside the SDK's.
fn rust_log<'a>(adapters: impl IntoIterator<Item = &'a str>) -> String {
    let mut filter = "emery_sdk=trace".to_owned();
    for adapter in adapters {
        let _ = write!(filter, ",{adapter}=trace");
    }
    filter + ",omnia_cursor=info,omnia_core=off"
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
    let build = filter.remove("--build");
    if facts_only && filter.is_empty() {
        return Err("`--facts` names a case or an adapter".into());
    }
    if facts_only && build {
        return Err("`--facts` spends no turn, so it cannot `--build`".into());
    }
    let cases = cases(root, &filter)?;
    if cases.is_empty() {
        return Err(format!("no case under `{CASES}` matches").into());
    }
    let adapters: BTreeSet<&str> = cases.iter().flat_map(|case| case.expected.adapters()).collect();
    let settings = Settings::from_env(root, &adapters, build)?;

    let eval_dir = root.join("target/eval");
    if facts_only {
        for case in &cases {
            let project = stage(&eval_dir.join("facts"), case, &settings)?;
            let text = facts(&project, &settings, case)?;
            fs::write(project.join("facts.md"), &text)?;
            println!("# {} — survey facts\n\n{text}\n", case.name);
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
    let first = run(project, settings, case, n, rung, tag)?;
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
    let mut again = run(project, settings, case, n, rung, tag)?;
    again.dead = Some(first.summary());

    Ok(again)
}

struct Settings {
    // the repository root, which every fixture path is relative to
    root: PathBuf,
    emery: PathBuf,
    // the binary's package store, where each component is staged under its
    // `eval:` reference
    store: PathBuf,
    // every component the run stages: each adapter a selected case runs
    // under and, on `--build`, the target
    wasm: BTreeMap<String, PathBuf>,
    // the target adapter a `--build` run builds every plan through
    target: Option<String>,
    // the width cap a `--build` run passes as `--jobs`; none leaves the
    // engine's default
    jobs: Option<NonZeroUsize>,
    runs: usize,
    ladder: Vec<Rung>,
    model: String,
    // the caller's `RUST_LOG`, when set, over the per-case default
    rust_log: Option<String>,
}

impl Settings {
    fn from_env(root: &Path, adapters: &BTreeSet<&str>, build: bool) -> Result<Self, String> {
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
        let store = env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".emery/adapters"))
            .ok_or("HOME: unset, so the binary's store `~/.emery/adapters` has no home")?;
        let target = build.then(|| TARGET.to_owned());
        let mut wasm = BTreeMap::new();
        for component in adapters.iter().copied().chain(target.as_deref()) {
            // cargo spells a hyphenated crate's artifact with underscores
            let artifact = component.replace('-', "_");
            let name = format!("{}_WASM", artifact.to_uppercase());
            let path = given(&name, format!("target/wasm32-wasip2/release/{artifact}.wasm"));
            if !path.is_file() {
                return Err(format!("{name}: no file at `{}`", path.display()));
            }
            wasm.insert(component.to_owned(), path);
        }
        let runs = match env::var("EVAL_RUNS") {
            Ok(runs) => runs.parse().map_err(|error| format!("EVAL_RUNS: `{runs}`: {error}"))?,
            Err(_) => 3,
        };
        let jobs = match env::var("EVAL_JOBS") {
            Ok(jobs) => {
                Some(jobs.parse().map_err(|error| format!("EVAL_JOBS: `{jobs}`: {error}"))?)
            }
            Err(_) => None,
        };
        let ladder = env::var("EVAL_LADDER").unwrap_or_else(|_| LADDER.to_owned());
        let ladder: Vec<Rung> = ladder.split(',').map(Rung::parse).collect::<Result<_, _>>()?;

        Ok(Self {
            root: root.to_path_buf(),
            emery,
            store,
            wasm,
            target,
            jobs,
            runs,
            ladder,
            model: env::var("CURSOR_MODEL").unwrap_or_else(|_| "auto".to_owned()),
            rust_log: env::var("RUST_LOG").ok(),
        })
    }

    // The component an adapter was built to: every selected case's is read
    // at start.
    fn component(&self, adapter: &str) -> io::Result<&Path> {
        self.wasm.get(adapter).map(PathBuf::as_path).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("no component for adapter `{adapter}`"))
        })
    }

    // Stages an adapter's component in the store as the `eval:` package
    // every case names it by, replacing the last run's, and answers the
    // reference: the store serves the file before any registry is asked.
    fn stage_component(&self, adapter: &str) -> io::Result<String> {
        let reference = format!("eval:{adapter}@{}", env!("CARGO_PKG_VERSION"));
        fs::create_dir_all(&self.store)?;
        fs::copy(
            self.component(adapter)?,
            self.store.join(format!("eval_{adapter}@{}.wasm", env!("CARGO_PKG_VERSION"))),
        )?;
        Ok(reference)
    }

    fn rust_log(&self, case: &Case) -> String {
        self.rust_log.clone().unwrap_or_else(|| rust_log(case.expected.adapters()))
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
    // the sources the run binds, one or several
    #[serde(rename = "source")]
    sources: Vec<Source>,
    #[serde(default, rename = "requirement")]
    requirements: Vec<Item>,
    #[serde(default, rename = "criterion")]
    criteria: Vec<Item>,
    #[serde(default, rename = "decision")]
    decisions: Vec<Item>,
    // the requirements the engine reconciles across the sources, exactly
    #[serde(default, rename = "behaviour")]
    behaviours: Vec<Behaviour>,
}

// One source of the run: the adapter it runs under and what it is bound to,
// with the stems its accepted requirement claims lead with, exactly, and
// the (entry, stem) pairs its survey decides, exactly.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    // the name the specification cites the source by; the adapter's when
    // the line is absent, as `emery` names it
    name: Option<String>,
    // the adapter the source runs under, by crate name
    adapter: String,
    // the fixture tree, relative to the repository root
    fixture: Option<String>,
    // an inline value in the fixture's place
    description: Option<String>,
    // the `[[source]] rank` the staged `emery.toml` carries; absent, the
    // adapter's kind decides
    rank: Option<NonZeroU32>,
    #[serde(default)]
    stems: Vec<String>,
    #[serde(default, rename = "surface")]
    surfaces: Vec<ExpectedSurface>,
}

impl Source {
    fn name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.adapter)
    }
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
    // the source the item is read from; the case's one source when absent
    #[serde(default)]
    source: Option<String>,
    // the stem — or any one of the stems — a meeting claim's id leads with
    // for the match to count as `in stem`; none grades the anchor alone
    #[serde(default)]
    stem: Option<Stems>,
    anchor: String,
    gloss: String,
}

// One behaviour the specification states once, however many sources state
// it: the claims that contribute, each at its source and anchor, the status
// the engine's authority rule gives the requirement, and, where the status
// is a disagreement, the source whose statement loses — so a `divergence`
// between two other sources does not pass for the one planted.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Behaviour {
    gloss: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    loser: Option<String>,
    claims: Vec<ExpectedClaim>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedClaim {
    source: String,
    anchor: String,
}

impl Behaviour {
    fn label(&self) -> String {
        let claims: Vec<String> = self
            .claims
            .iter()
            .map(|claim| format!("{}:`{}`", claim.source, claim.anchor))
            .collect();
        format!("{} — {}", claims.join(", "), self.gloss)
    }

    // The status expected, with the loser where one is named: `divergence`,
    // `divergence losing faq`.
    fn expected_status(&self) -> Option<String> {
        let status = self.status.as_ref()?;
        Some(
            self.loser
                .as_ref()
                .map_or_else(|| status.clone(), |loser| format!("{status} losing {loser}")),
        )
    }

    // Whether a requirement holding the behaviour carries the status
    // expected and, where a loser is named, a loser note from that source.
    fn decided(&self, requirement: &Value) -> bool {
        let Some(status) = &self.status else { return true };
        requirement["status"] == status.as_str()
            && self.loser.as_deref().is_none_or(|loser| losers_of(requirement).any(|s| s == loser))
    }

    // What a requirement holding the behaviour decided, read the way
    // `expected_status` spells it, so a mismatch reads against it.
    fn outcome(&self, requirement: &Value) -> Option<String> {
        let status = requirement["status"].as_str()?;
        if self.loser.is_none() {
            return Some(status.to_owned());
        }
        let losers: BTreeSet<&str> = losers_of(requirement).collect();
        if losers.is_empty() {
            return Some(status.to_owned());
        }
        let losers: Vec<&str> = losers.into_iter().collect();
        Some(format!("{status} losing {}", losers.join(", ")))
    }
}

// The sources whose statements a requirement lists as losers, as `emery show
// --format json` carries them.
fn losers_of(requirement: &Value) -> impl Iterator<Item = &str> {
    requirement["losers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|loser| loser["sources"].as_array())
        .flatten()
        .filter_map(Value::as_str)
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

impl Expected {
    fn adapters(&self) -> impl Iterator<Item = &str> {
        self.sources.iter().map(|source| source.adapter.as_str())
    }

    // The source an item names, or the case's one source when it names none.
    fn source(&self, named: Option<&str>) -> Result<&Source, String> {
        match named {
            Some(name) => self
                .sources
                .iter()
                .find(|source| source.name() == name)
                .ok_or_else(|| format!("names no source of the case: `{name}`")),
            None if self.sources.len() == 1 => Ok(&self.sources[0]),
            None => Err("names no source, and the case has several".to_owned()),
        }
    }

    // What the shape alone does not hold: one content key per source, each
    // source named once, and every item and claim at a source of the case.
    fn check(&self) -> Result<(), String> {
        if self.sources.is_empty() {
            return Err("names no `[[source]]`".to_owned());
        }
        let mut names = BTreeSet::new();
        for source in &self.sources {
            if source.fixture.is_some() == source.description.is_some() {
                return Err(format!(
                    "source `{}` has one of `fixture` and `description`, not both or neither",
                    source.name()
                ));
            }
            if !names.insert(source.name()) {
                return Err(format!("source `{}` is named twice", source.name()));
            }
        }
        for (kind, items) in [
            ("requirement", &self.requirements),
            ("criterion", &self.criteria),
            ("decision", &self.decisions),
        ] {
            for item in items {
                self.source(item.source.as_deref())
                    .map_err(|why| format!("{kind} `{}` {why}", item.anchor))?;
            }
        }
        for behaviour in &self.behaviours {
            if behaviour.claims.is_empty() {
                return Err(format!("behaviour `{}` names no claim", behaviour.gloss));
            }
            for claim in &behaviour.claims {
                self.source(Some(&claim.source))
                    .map_err(|why| format!("behaviour `{}` {why}", behaviour.gloss))?;
            }
            if let Some(status) = &behaviour.status
                && !matches!(status.as_str(), "agreed" | "unknown" | "divergence" | "conflict")
            {
                return Err(format!(
                    "behaviour `{}` expects status `{status}`; the statuses are `agreed`, \
                     `unknown`, `divergence`, and `conflict`",
                    behaviour.gloss
                ));
            }
            if let Some(loser) = &behaviour.loser {
                if !matches!(behaviour.status.as_deref(), Some("divergence" | "conflict")) {
                    return Err(format!(
                        "behaviour `{}` names a loser under no disagreement; `loser` goes with \
                         status `divergence` or `conflict`",
                        behaviour.gloss
                    ));
                }
                if !behaviour.claims.iter().any(|claim| &claim.source == loser) {
                    return Err(format!(
                        "behaviour `{}` names loser `{loser}`, which none of its claims is from",
                        behaviour.gloss
                    ));
                }
            }
        }
        Ok(())
    }
}

// Every case under `evals/cases/`, or the ones the filter names — a name is
// a case's or an adapter's, which selects every case with a source running
// under it.
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
        expected.check().map_err(|error| format!("{name}: expected.toml {error}"))?;
        let selected = filter.is_empty()
            || filter.contains(name)
            || expected.adapters().any(|adapter| filter.contains(adapter));
        if !selected {
            continue;
        }
        // a fixture the checkout lacks (the vendored ones are gitignored until
        // fetched) skips its case unless the case was asked for by name
        let absent = expected
            .sources
            .iter()
            .filter_map(|source| source.fixture.as_deref())
            .find(|fixture| !root.join(fixture).is_dir());
        if let Some(fixture) = absent {
            if filter.contains(name) {
                return Err(format!("{name}: no fixture at `{fixture}`").into());
            }
            eprintln!("eval: `{name}` skipped, no fixture at `{fixture}`");
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
            !cases.iter().any(|case| {
                case.name == **name || case.expected.adapters().any(|adapter| adapter == *name)
            })
        })
        .collect();
    if !unmatched.is_empty() {
        return Err(format!("no case or adapter under `{CASES}` named {unmatched:?}").into());
    }

    Ok(cases)
}

// --- staging ---

// Copies each source's fixture into a project of its own beneath `dir` —
// `emery` mounts its invocation directory, so the tree must sit inside it,
// and the revision store the run commits under stays with the case — and
// each adapter's component into the binary's store under the `eval:`
// reference the project names. Each source's tree sits under its name.
fn stage(dir: &Path, case: &Case, settings: &Settings) -> io::Result<PathBuf> {
    let project = dir.join(&case.name);
    if project.exists() {
        fs::remove_dir_all(&project)?;
    }
    fs::create_dir_all(&project)?;
    let mut config = String::new();
    for source in &case.expected.sources {
        let reference = settings.stage_component(&source.adapter)?;
        let _ = write!(
            config,
            "[[source]]\nname = \"{}\"\nadapter = \"{reference}\"\n",
            toml_escaped(source.name())
        );
        match (&source.fixture, &source.description) {
            (Some(fixture), _) => {
                copy_tree(&settings.root.join(fixture), &project.join(source.name()))?;
                let _ = writeln!(config, "path = \"{}/\"", toml_escaped(source.name()));
            }
            (None, Some(description)) => {
                let _ = writeln!(config, "description = \"{}\"", toml_escaped(description));
            }
            (None, None) => {}
        }
        if let Some(rank) = source.rank {
            let _ = writeln!(config, "rank = {rank}");
        }
        config.push('\n');
    }
    if let Some(target) = &settings.target {
        let reference = settings.stage_component(target)?;
        let url = greenfield(&project, &case.name)?;
        let _ = write!(
            config,
            "[target]\nadapter = \"{reference}\"\nrepository = \"{url}\"\nbranch = \"main\"\n\
             remote = \"origin\"\n"
        );
    }
    fs::write(project.join("emery.toml"), config)?;

    Ok(project)
}

fn toml_escaped(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

// The repository a `--build` run builds into, and the URL the project names
// it by: `target.git` beneath the project, bare, so the engine clones it
// and pushes the label back, its `main` one root commit holding an ignore
// rule for what a Node build leaves and a README naming the case. The
// engine never initialises a repository, so the commit is sealed here,
// through a seed clone the host's git pushes and the runner removes.
fn greenfield(project: &Path, case: &str) -> io::Result<String> {
    let bare = project.join("target.git");
    fs::create_dir_all(&bare)?;
    git(&bare, &["init", "--quiet", "--bare", "--initial-branch=main"])?;
    let url = format!("file://{}", bare.display());

    let seed = project.join("target.seed");
    fs::create_dir_all(&seed)?;
    fs::write(seed.join(".gitignore"), "node_modules/\ndist/\n.emery/\n")?;
    fs::write(
        seed.join("README.md"),
        format!("# {case}\n\nThe tree the eval's `--build` run builds the `{case}` plan into.\n"),
    )?;
    git(&seed, &["init", "--quiet", "--initial-branch=main"])?;
    git(&seed, &["add", "--all"])?;
    git(&seed, &["commit", "--quiet", "--message", "Greenfield"])?;
    git(&seed, &["push", "--quiet", "--", &url, "HEAD:refs/heads/main"])?;
    fs::remove_dir_all(&seed)?;

    Ok(url)
}

// One host git over a repository the runner laid or reads, under an
// identity of the runner's own so no operator configuration is needed;
// its stdout trimmed, its stderr the error when it fails.
#[expect(
    clippy::disallowed_types,
    reason = "the eval lays and reads its repositories with the host's git"
)]
fn git(at: &Path, args: &[&str]) -> io::Result<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(at)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "emery eval")
        .env("GIT_AUTHOR_EMAIL", "eval@emery.invalid")
        .env("GIT_COMMITTER_NAME", "emery eval")
        .env("GIT_COMMITTER_EMAIL", "eval@emery.invalid")
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "git {args:?} at `{}`: {}",
            at.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

// What a fixture's checkout may hold that no run reads: dependencies,
// environments, caches, build output, and the revision store of an earlier
// run; `*.egg-info` by its suffix.
const UNCOPIED: &[&str] = &[
    "node_modules",
    ".git",
    ".emery",
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
    // the committed plan's slices grouped into the sets ready to build at
    // once, as the specify envelope reports them
    waves: Option<Value>,
    // each seam's accepted evidence, under the source it was mined from
    evidence: Vec<(String, Evidence)>,
    completions: Vec<Completion>,
    // the surfaces each source's seams were cut from, as its adapter logged them
    surveyed: BTreeMap<String, Vec<Surveyed>>,
    // the modules each source's survey placed under no surface, by the
    // adapter's own count (`placed by model`), else as the model listed them
    // (the SDK's `surveyed by model` line)
    unreached: BTreeMap<String, usize>,
    spec: Option<Value>,
    design: Option<Value>,
    plan: Option<Value>,
    // the summary of a stillborn attempt this run was put again after
    dead: Option<String>,
    // the build of the committed plan, on `--build` after a specify that landed
    build: Option<Build>,
}

// One surface as an adapter's `surveyed` line spells it; `lines` is its
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
    // surveys decided, each source's against its own, the bootstrap set
    // aside — its span is its module or its guard, not a registration or a
    // declaration; none when no adapter logged a surface span.
    fn landing(&self) -> Option<Landing> {
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

    fn claims(&self, kind: ClaimKind) -> impl Iterator<Item = &Claim> {
        self.sourced(kind).map(|(_, claim)| claim)
    }

    fn claims_of<'r>(
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

    fn meets(&self, requirement: &Value, claim: &ExpectedClaim) -> bool {
        cites(requirement, claim) || self.covers(requirement, claim).is_some()
    }

    // The requirements holding `behaviour`: those citing one of its claims,
    // and those holding one as a criterion alone.
    fn holders(&self, behaviour: &Behaviour) -> Holders {
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
    fn criteria_under(&self, indices: &[usize], behaviour: &Behaviour) -> Vec<String> {
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

    fn subject(&self, index: usize) -> String {
        self.requirements()[index]["subject"]
            .as_str()
            .map_or_else(|| index.to_string(), str::to_owned)
    }

    // The stems each source's requirement claims lead with, as (source, stem).
    fn stems(&self) -> BTreeSet<(&str, &str)> {
        self.sourced(ClaimKind::Requirement)
            .filter_map(|(source, claim)| Some((source, stem(claim.id.as_deref()?)?)))
            .collect()
    }

    fn stems_of<'r>(&'r self, source: &'r str) -> BTreeSet<&'r str> {
        self.stems().into_iter().filter(|(of, _)| *of == source).map(|(_, stem)| stem).collect()
    }

    fn ids(&self) -> BTreeSet<(&str, &str)> {
        self.sourced(ClaimKind::Requirement)
            .filter_map(|(source, claim)| Some((source, claim.id.as_deref()?)))
            .collect()
    }

    // The (source, entry, stem) triples the surveys decided, however many
    // surfaces share one.
    fn pairs(&self) -> BTreeSet<(&str, &str, &str)> {
        self.surveyed
            .iter()
            .flat_map(|(source, surfaces)| {
                surfaces.iter().map(move |surface| {
                    (source.as_str(), surface.entry.as_str(), surface.stem.as_str())
                })
            })
            .collect()
    }

    fn pairs_of<'r>(&'r self, source: &'r str) -> BTreeSet<(&'r str, &'r str)> {
        self.pairs()
            .into_iter()
            .filter(|(of, _, _)| *of == source)
            .map(|(_, entry, stem)| (entry, stem))
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

    fn wave_count(&self) -> usize {
        self.waves.as_ref().and_then(Value::as_array).map_or(0, Vec::len)
    }

    // The plan's shape: how many waves, then each wave's width, so `3 (1/2/1)`
    // is three waves of one, two, and one slices; `0` where no plan was committed.
    fn waves(&self) -> String {
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
    let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    tail.reverse();

    // what the run committed, read back through `show`
    let envelope: Option<Value> = serde_json::from_str(&stdout).ok();
    let mut run = Run {
        n,
        rung,
        wall,
        exit: output.status.code(),
        tail: tail.join("\n"),
        diff: envelope.as_ref().and_then(|out| out.get("diff").cloned()),
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
    for (index, (source, evidence)) in run.evidence.iter().enumerate() {
        fs::write(
            project.join(format!("{tag}.evidence-{source}-{index}.json")),
            serde_json::to_string_pretty(evidence)?,
        )?;
    }

    // the plan is built once the revision it is read from landed
    let revision = envelope.as_ref().and_then(|out| out["revision"].as_str());
    if let (Some(_), Some(revision)) = (&settings.target, revision) {
        eprintln!("eval: `{}` run {n} at {rung} building", case.name);
        run.build = Some(build(project, settings, &rust_log, rung, tag, revision)?);
    }

    Ok(run)
}

const SPECIFY: &[&str] = &["specify", "--config", "emery.toml"];

// One `emery build` of the revision the run committed, into the case's
// greenfield repository; its log and envelope are kept as
// `<tag>.build.stderr` / `<tag>.build.stdout`.
#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
fn build(
    project: &Path, settings: &Settings, rust_log: &str, rung: Rung, tag: &str, revision: &str,
) -> io::Result<Build> {
    let jobs = settings.jobs.map(|jobs| jobs.to_string());
    let mut args = vec!["build", "--config", "emery.toml"];
    if let Some(jobs) = &jobs {
        args.extend(["--jobs", jobs]);
    }
    let started = Instant::now();
    let output = emery(project, settings, rust_log, rung, &args, Turns::Live)?;
    let wall = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    fs::write(project.join(format!("{tag}.build.stderr")), stderr.as_bytes())?;
    fs::write(project.join(format!("{tag}.build.stdout")), stdout.as_bytes())?;
    let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    tail.reverse();

    Ok(Build {
        wall,
        exit: output.status.code(),
        envelope: serde_json::from_str(&stdout).ok(),
        failure: failure(&stderr),
        completions: completions(&stderr),
        history: history(project, revision),
        tail: tail.join("\n"),
    })
}

// What one `emery build` did, read from its envelope, its log, and the
// repository it built into.
struct Build {
    wall: Duration,
    exit: Option<i32>,
    // the `build` envelope on success
    envelope: Option<Value>,
    // the failure envelope the binary wrote among its log lines
    failure: Option<Failure>,
    completions: Vec<Completion>,
    // the first-parent chain the build left, newest first: the label the
    // run pushed, or the integration copy a failed run left behind
    history: Vec<Landed>,
    tail: String,
}

// The failure envelope: its code and message.
struct Failure {
    code: String,
    message: String,
}

// One commit on the build's first-parent chain: a slice's merge, read
// from its trailers, a `Wave <k> verified` seal, or the greenfield root.
#[derive(Debug, PartialEq, Eq)]
struct Landed {
    commit: String,
    subject: String,
    slice: Option<String>,
    wave: Option<usize>,
    requirements: Vec<String>,
    covered: Vec<String>,
    files: Vec<String>,
}

// One slice as the build landed it: from the envelope when the run landed,
// else from the merge commit's trailers.
struct Integrated {
    id: String,
    name: String,
    wave: usize,
    requirements: Vec<String>,
    covered: Vec<String>,
    files: Vec<String>,
    commit: Option<String>,
    conflicts: Vec<String>,
}

impl Integrated {
    fn uncovered(&self) -> Vec<&str> {
        self.requirements
            .iter()
            .filter(|id| !self.covered.contains(id))
            .map(String::as_str)
            .collect()
    }
}

// The build turns of one wave with the verify that closed it: the
// completions between two `verify` completions, since a wave's builds all
// finish before its verify opens and the next wave's open after it.
#[derive(Default)]
struct WaveTurns<'a> {
    builds: Vec<&'a Completion>,
    verifies: Vec<&'a Completion>,
}

impl WaveTurns<'_> {
    // The wave's time on the critical path: the slowest build, every
    // round of it, then the verify.
    fn critical(&self) -> Duration {
        let mut per_slice: BTreeMap<&str, Duration> = BTreeMap::new();
        for build in &self.builds {
            *per_slice.entry(&build.label).or_default() += build.duration;
        }
        let slowest = per_slice.values().max().copied().unwrap_or_default();
        slowest + self.verifies.iter().map(|verify| verify.duration).sum::<Duration>()
    }
}

impl Build {
    fn summary(&self, slices: usize) -> String {
        match self.exit {
            Some(0) => format!(
                "ok in {}s, {}/{slices} merged in {} waves",
                self.wall.as_secs(),
                self.integrated().len(),
                self.waves()
            ),
            Some(code) => {
                let failed = self.failure.as_ref().map_or(String::new(), |failure| {
                    let wave =
                        self.wave_failed().map_or(String::new(), |k| format!(" in wave {k}"));
                    format!(" `{}`{wave}", failure.code)
                });
                format!("exit {code}{failed} after {}s", self.wall.as_secs())
            }
            None => format!("killed after {}s", self.wall.as_secs()),
        }
    }

    fn landed(&self) -> bool {
        self.exit == Some(0)
    }

    fn code(&self) -> Option<&str> {
        self.failure.as_ref().map(|failure| failure.code.as_str())
    }

    // The slices the build merged, in build order.
    fn integrated(&self) -> Vec<Integrated> {
        if let Some(slices) = self.envelope.as_ref().and_then(|out| out["slices"].as_array()) {
            return slices
                .iter()
                .map(|slice| {
                    let covered = strings(&slice["covered"]);
                    let mut requirements = covered.clone();
                    requirements.extend(strings(&slice["uncovered"]));
                    Integrated {
                        id: slice["id"].as_str().unwrap_or("").to_owned(),
                        name: slice["name"].as_str().unwrap_or("").to_owned(),
                        wave: slice["wave"]
                            .as_u64()
                            .and_then(|wave| wave.try_into().ok())
                            .unwrap_or(0),
                        requirements,
                        covered,
                        files: strings(&slice["written"]),
                        commit: slice["commit"].as_str().map(str::to_owned),
                        conflicts: strings(&slice["conflicts"]),
                    }
                })
                .collect();
        }
        let mut merged: Vec<Integrated> = self
            .history
            .iter()
            .rev()
            .filter_map(|landed| {
                let id = landed.slice.clone()?;
                let name = landed.subject.split_once(' ').map_or("", |(_, name)| name).to_owned();
                Some(Integrated {
                    id,
                    name,
                    wave: landed.wave.unwrap_or(0),
                    requirements: landed.requirements.clone(),
                    covered: landed.covered.clone(),
                    files: landed.files.clone(),
                    commit: Some(landed.commit.clone()),
                    conflicts: Vec::new(),
                })
            })
            .collect();
        merged.sort_by_key(|slice| (slice.wave, slice.id.clone()));
        merged
    }

    // How many waves the build ran: the last one merged in when it landed,
    // else the waves its turns fell into.
    fn waves(&self) -> usize {
        if self.landed() {
            return self.integrated().iter().map(|slice| slice.wave).max().unwrap_or(0);
        }
        self.wave_turns().len()
    }

    // The waves whose verify passed: every one the envelope lists, else
    // every wave before the one the failure names.
    fn verified(&self) -> usize {
        if let Some(verified) = self.envelope.as_ref().and_then(|out| out["verified"].as_array()) {
            return verified.len();
        }
        self.wave_failed().map_or(0, |wave| wave.saturating_sub(1))
    }

    // The wave the failure names, read from the engine's description:
    // `slice ... failed in wave <k>` or `wave <k> failed`.
    fn wave_failed(&self) -> Option<usize> {
        let message = &self.failure.as_ref()?.message;
        let (_, rest) = message.split_once("wave ")?;
        rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    }

    // The failures a `verify-failed` run quotes, one per `- ` line.
    fn verify_failures(&self) -> Vec<&str> {
        match &self.failure {
            Some(failure) if failure.code == "verify-failed" => {
                failure.message.lines().filter_map(|line| line.strip_prefix("- ")).collect()
            }
            _ => Vec::new(),
        }
    }

    // The completions grouped into the waves they were spent in; a wave
    // whose verify never opened, the one a failure ended, is the last.
    fn wave_turns(&self) -> Vec<WaveTurns<'_>> {
        let mut waves: Vec<WaveTurns<'_>> = Vec::new();
        let mut current = WaveTurns::default();
        for completion in &self.completions {
            if completion.label.starts_with("build-") {
                current.builds.push(completion);
            } else if completion.label == "verify" {
                match waves.last_mut() {
                    // a correction round of the verify that closed the last wave
                    Some(last) if current.builds.is_empty() => last.verifies.push(completion),
                    _ => {
                        current.verifies.push(completion);
                        waves.push(std::mem::take(&mut current));
                    }
                }
            }
        }
        if !current.builds.is_empty() {
            waves.push(current);
        }
        waves
    }

    // Every turn's time, as a serial build would have spent it.
    fn serial(&self) -> Duration {
        self.completions
            .iter()
            .filter(|c| c.label == "verify" || c.label.starts_with("build-"))
            .map(|completion| completion.duration)
            .sum()
    }

    // The time the waves took end to end, each at its slowest slice.
    fn critical(&self) -> Duration {
        self.wave_turns().iter().map(WaveTurns::critical).sum()
    }

    // How much shorter the waves ran than the same turns in a row, in
    // hundredths.
    fn speedup(&self) -> Option<usize> {
        let critical = self.critical().as_millis();
        if critical == 0 {
            return None;
        }
        usize::try_from(self.serial().as_millis() * 100 / critical).ok()
    }

    // The requirements covered over the requirements of the slices merged.
    fn covered(&self) -> (usize, usize) {
        self.integrated().iter().fold((0, 0), |(covered, all), slice| {
            (covered + slice.covered.len(), all + slice.requirements.len())
        })
    }

    fn tokens(&self) -> Tokens {
        self.completions
            .iter()
            .map(|completion| completion.tokens)
            .fold(Tokens::default(), Tokens::add)
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

// The failure envelope the binary prints among its log lines: pretty JSON,
// so its braces stand alone at the margin where no log line starts.
fn failure(stderr: &str) -> Option<Failure> {
    let lines: Vec<&str> = stderr.lines().collect();
    let open = lines.iter().rposition(|line| *line == "{")?;
    let close = open + lines[open..].iter().position(|line| *line == "}")?;
    let envelope: Value = serde_json::from_str(&lines[open..=close].join("\n")).ok()?;
    Some(Failure {
        code: envelope["error"].as_str()?.to_owned(),
        message: envelope["message"].as_str().unwrap_or("").to_owned(),
    })
}

// The chain the build left, read where it stands: the integration copy a
// failed run leaves for inspection, else the label a landed run pushed to
// `target.git`. A chain nowhere, a run that ended before any was cut, is
// empty.
fn history(project: &Path, revision: &str) -> Vec<Landed> {
    let integration = project.join(".emery/vcs/integration");
    let (at, tip) = if integration.is_dir() {
        (integration, "HEAD".to_owned())
    } else {
        (project.join("target.git"), format!("refs/heads/emery/{revision}"))
    };
    if !at.is_dir() {
        return Vec::new();
    }
    let format = "--format=%x1e%H%x1f%s%x1f%(trailers:key=Slice,valueonly)%x1f\
                  %(trailers:key=Wave,valueonly)%x1f%(trailers:key=Requirements,valueonly)%x1f\
                  %(trailers:key=Covered,valueonly)%x1f";
    let log = git(&at, &["log", "--first-parent", "--name-only", format, &tip]);
    log.map(|text| chain(&text)).unwrap_or_default()
}

// The chain as `history` asks git to spell it: one record per commit led
// by `\x1e`, its fields split by `\x1f`, the files it changed against its
// first parent after the last.
fn chain(text: &str) -> Vec<Landed> {
    text.split('\x1e')
        .filter(|record| !record.trim().is_empty())
        .filter_map(|record| {
            let fields: Vec<&str> = record.split('\x1f').collect();
            let [commit, subject, slice, wave, requirements, covered, files] = fields[..] else {
                return None;
            };
            let ids = |text: &str| {
                text.trim()
                    .split(", ")
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            let slice = slice.trim();
            Some(Landed {
                commit: commit.trim().to_owned(),
                subject: subject.trim().to_owned(),
                slice: (!slice.is_empty()).then(|| slice.to_owned()),
                wave: wave.trim().parse().ok(),
                requirements: ids(requirements),
                covered: ids(covered),
                files: files.lines().filter(|line| !line.is_empty()).map(str::to_owned).collect(),
            })
        })
        .collect()
}

// Whether a run may put a turn to the model: `None` strips `CURSOR_API_KEY`
// from the binary's environment, so the backend refuses the first turn
// after the adapters have read their trees and logged their facts.
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

// The survey facts a staged project's adapters lay, read from one run that
// spends no turn: the run fails at the first completion, after the facts
// are logged.
fn facts(project: &Path, settings: &Settings, case: &Case) -> io::Result<String> {
    let rust_log = settings.rust_log(case);
    let rung =
        settings.ladder.first().copied().ok_or_else(|| io::Error::other("EVAL_LADDER: no rung"))?;
    let output = emery(project, settings, &rust_log, rung, SPECIFY, Turns::None)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    fs::write(project.join("facts.stderr"), stderr.as_bytes())?;
    let facts = survey_facts(&stderr);
    if facts.is_empty() {
        let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
        tail.reverse();
        let adapters: Vec<&str> = case.expected.adapters().collect();
        return Err(io::Error::other(format!(
            "no `survey facts` line from {adapters:?} (does any survey? is each at `trace` in \
             RUST_LOG?); the run ended:\n{}",
            tail.join("\n")
        )));
    }
    Ok(facts_text(&facts))
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

// An adapter's `survey facts` trace line carries the text laid before the
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
fn accepted(stderr: &str) -> Vec<(String, Evidence)> {
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
fn surveyed(stderr: &str) -> BTreeMap<String, Vec<Surveyed>> {
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
fn unreached(stderr: &str) -> BTreeMap<String, usize> {
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
    decisions: Recall,
    behaviours: Behaviours,
    // what each source's survey and stems read against its expectations
    sources: Vec<SourceGrade>,
}

struct SourceGrade {
    name: String,
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
    // whether the source's adapter logged a survey at all: one that cuts its
    // seams without a turn decides no surface, and reads as `no survey`
    // rather than as a survey that missed every one
    surveyed: bool,
    matched: usize,
    expected: usize,
    restemmed: Vec<String>,
    missed: Vec<String>,
    extra: Vec<String>,
}

// The specification's requirements against the behaviours expected: each
// behaviour met by a requirement citing one of its claims — or holding one
// as a criterion under a claim it cites — split across two or more citing
// requirements, or met by none; a requirement citing two behaviours is a
// wrong merge; a met behaviour whose claims are not all held is partial; and
// the status the engine gave is read against the one expected. A criterion
// counts toward neither split nor wrong merge: it is in the specification
// under its requirement, hidden from no one, and no grouping can join it to
// a requirement of the other source.
#[derive(Debug, Default, PartialEq, Eq)]
struct Behaviours {
    expected: usize,
    met: usize,
    split: Vec<String>,
    wrong: Vec<String>,
    missed: Vec<String>,
    partial: Vec<String>,
    // the behaviours no requirement cites directly, met through a criterion
    // alone: the kind the model chose, which the count would otherwise hide
    by_criterion: Vec<String>,
    // the behaviours one requirement cites while another holds them as a
    // criterion: stated across kinds, which the engine cannot reconcile
    across: Vec<String>,
    status_expected: usize,
    status_met: usize,
    status_wrong: Vec<String>,
    // the requirements the engine pre-merged on a byte-equal id across
    // sources, by their subject and the id
    premerged: Vec<String>,
}

fn grade(expected: &Expected, run: &Run) -> Grade {
    let sources = expected
        .sources
        .iter()
        .map(|source| {
            let observed = run.stems_of(source.name());
            let wanted: BTreeSet<&str> = source.stems.iter().map(String::as_str).collect();
            SourceGrade {
                name: source.name().to_owned(),
                surfaces: surfaces(
                    &source.surfaces,
                    &run.pairs_of(source.name()),
                    run.surveyed.contains_key(source.name()),
                ),
                missing_stems: wanted
                    .difference(&observed)
                    .map(|stem| (*stem).to_owned())
                    .collect(),
                extra_stems: observed.difference(&wanted).map(|stem| (*stem).to_owned()).collect(),
            }
        })
        .collect();

    Grade {
        requirements: recall(&expected.requirements, expected, run, ClaimKind::Requirement),
        criteria: recall(&expected.criteria, expected, run, ClaimKind::Criterion),
        decisions: recall(&expected.decisions, expected, run, ClaimKind::Decision),
        behaviours: behaviours(&expected.behaviours, run),
        sources,
    }
}

// An expected item is met by a claim of its kind, from its source, whose
// anchor overlaps its own — same file, meeting line ranges, a whole-file
// anchor meeting any. Paraphrase never enters into it; the stem is counted
// apart, so a survey that shapes the estate differently still shows what it
// mined.
fn recall(items: &[Item], expected: &Expected, run: &Run, kind: ClaimKind) -> Recall {
    let mut recall = Recall {
        expected: items.len(),
        ..Recall::default()
    };
    for item in items {
        let label = || {
            let stem = item.stem.as_ref().map_or(String::new(), |stems| format!("{stems} · "));
            let source = match (&item.source, expected.sources.len()) {
                (Some(source), _) => format!("{source}:"),
                (None, 1) => String::new(),
                (None, _) => "?:".to_owned(),
            };
            format!("{stem}{source}`{}` — {}", item.anchor, item.gloss)
        };
        let Ok(source) = expected.source(item.source.as_deref()) else {
            recall.missed.push(format!("{} (no such source)", label()));
            continue;
        };
        let Ok(wanted) = Anchor::parse(&item.anchor) else {
            recall.missed.push(format!("{} (unparseable anchor)", label()));
            continue;
        };
        let meeting: Vec<&Claim> = run
            .claims_of(source.name(), kind)
            .filter(|claim| {
                claim.anchor().and_then(Result::ok).is_some_and(|got| overlaps(got, wanted))
            })
            .collect();
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
fn surfaces(
    expected: &[ExpectedSurface], observed: &BTreeSet<(&str, &str)>, surveyed: bool,
) -> Surfaces {
    let wanted: BTreeSet<(&str, &str)> =
        expected.iter().map(|surface| (surface.entry.as_str(), surface.stem.as_str())).collect();
    let label = |(entry, stem): &(&str, &str)| format!("`{entry}` · {stem}");

    let mut open: Vec<(&str, &str)> = observed.difference(&wanted).copied().collect();
    let mut restemmed = Vec::new();
    let mut missed = Vec::new();
    for pair in wanted.difference(observed) {
        match open.iter().position(|(entry, _)| *entry == pair.0) {
            Some(at) => {
                let (_, got) = open.remove(at);
                restemmed.push(format!("{} (as {got})", label(pair)));
            }
            None => missed.push(label(pair)),
        }
    }

    Surfaces {
        surveyed,
        matched: wanted.intersection(observed).count(),
        expected: wanted.len(),
        restemmed,
        missed,
        extra: open.iter().map(label).collect(),
    }
}

// The requirements holding one behaviour, by how: `citing` cite one of its
// claims, `covering` hold one as a criterion and cite none.
struct Holders {
    citing: Vec<usize>,
    covering: Vec<usize>,
}

impl Holders {
    fn all(&self) -> impl Iterator<Item = usize> + '_ {
        self.citing.iter().chain(&self.covering).copied()
    }
}

// A requirement cites a behaviour when one of its citations — a source and
// the `path` the claim anchors at, as `emery show --format json` carries
// them — is from the source of one of the behaviour's claims and is held to
// its anchor's lines (`held`); it covers one through a criterion
// (`Run::covers`). The split and wrong-merge counts read the citations alone;
// a behaviour met by covering alone, or cited by one requirement and covered
// by another, is noted, so the kind the model chose still shows.
fn behaviours(expected: &[Behaviour], run: &Run) -> Behaviours {
    let mut graded = Behaviours {
        expected: expected.len(),
        ..Behaviours::default()
    };
    let requirements = run.requirements();

    // which behaviours each requirement cites
    let mut citing_each: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); requirements.len()];
    for (b, behaviour) in expected.iter().enumerate() {
        let holders = run.holders(behaviour);
        for &index in &holders.citing {
            citing_each[index].insert(b);
        }
        let label = behaviour.label();
        if holders.citing.is_empty() && holders.covering.is_empty() {
            graded.missed.push(label);
            continue;
        }
        graded.met += 1;
        let subjects: Vec<String> =
            holders.citing.iter().map(|&index| run.subject(index)).collect();
        if subjects.len() > 1 {
            graded.split.push(format!("{label} (requirements {subjects:?})"));
        }
        let criteria = run.criteria_under(&holders.covering, behaviour).join(", ");
        if holders.citing.is_empty() {
            graded.by_criterion.push(format!("{label} ({criteria})"));
        } else if !holders.covering.is_empty() {
            graded.across.push(format!("{label} ({criteria}; cited by {subjects:?})"));
        }
        let unheld: Vec<String> = behaviour
            .claims
            .iter()
            .filter(|claim| !holders.all().any(|index| run.meets(&requirements[index], claim)))
            .map(|claim| format!("{}:`{}`", claim.source, claim.anchor))
            .collect();
        if !unheld.is_empty() {
            graded.partial.push(format!("{label} (uncited {})", unheld.join(", ")));
        }
        if let Some(status) = behaviour.expected_status() {
            graded.status_expected += 1;
            if holders.all().any(|index| behaviour.decided(&requirements[index])) {
                graded.status_met += 1;
            } else {
                let found: BTreeSet<String> = holders
                    .all()
                    .filter_map(|index| behaviour.outcome(&requirements[index]))
                    .collect();
                graded.status_wrong.push(format!("{label} (expected `{status}`, got {found:?})"));
            }
        }
    }
    for (index, cited) in citing_each.iter().enumerate() {
        if cited.len() > 1 {
            let glosses: Vec<&str> = cited.iter().map(|&b| expected[b].gloss.as_str()).collect();
            graded.wrong.push(format!("`{}` cites {glosses:?}", run.subject(index)));
        }
    }

    // the requirements the engine pre-merged on a byte-equal id across sources
    for (index, requirement) in requirements.iter().enumerate() {
        let mut by_id: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for cited in requirement["sources"].as_array().into_iter().flatten() {
            if let (Some(source), Some(claim)) = (cited["source"].as_str(), cited["claim"].as_str())
            {
                by_id.entry(claim).or_default().insert(source);
            }
        }
        for (id, sources) in by_id {
            if sources.len() > 1 {
                graded
                    .premerged
                    .push(format!("`{}` on `{id}` from {sources:?}", run.subject(index)));
            }
        }
    }

    graded
}

fn stem(id: &str) -> Option<&str> {
    id.split('.').next().filter(|stem| !stem.is_empty())
}

// `id` is `parent` or a dotted child of it, as the engine reads a criterion
// covering a requirement.
fn under(id: &str, parent: &str) -> bool {
    id.strip_prefix(parent).is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
}

// Whether a requirement cites `claim`: a citation from the claim's source
// held to its lines.
fn cites(requirement: &Value, claim: &ExpectedClaim) -> bool {
    let Ok(wanted) = Anchor::parse(&claim.anchor) else { return false };
    citations(requirement).any(|(source, anchor)| source == claim.source && held(anchor, wanted))
}

// A citation is held to a behaviour's lines when it shares at least as many
// lines with them as it spends outside them: a whole-method anchor is not a
// statement of each branch within it, while one a line wider than the
// behaviour's is. An anchor naming no lines is held to any in its file.
fn held(citation: Anchor<'_>, lines: Anchor<'_>) -> bool {
    citation.path == lines.path
        && match (citation.lines, lines.lines) {
            (Some((from, to)), Some((start, end))) => {
                let shared = to.min(end).saturating_add(1).saturating_sub(from.max(start));
                shared.saturating_mul(2) >= to.saturating_sub(from).saturating_add(1)
            }
            _ => true,
        }
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
    let built = settings.target.as_deref().map_or_else(String::new, |target| {
        let jobs = settings
            .jobs
            .map_or_else(|| "the engine's default".to_owned(), |jobs| format!("`--jobs {jobs}`"));
        format!(
            " · each landed plan built through `{target}` into a greenfield `target.git` under \
             {jobs}"
        )
    });
    let mut card = format!(
        "# Eval {started}\n\nmodel `{}` · emery `{}` · {} · {} runs per case · ladder {} \
         (`CURSOR_TIMEOUT_SECS`/`CURSOR_INACTIVITY_SECS`, climbed on a `timeout` or `inactive` \
         completion; the columns are the first rung's){built} · runs under `target/eval/{name}/`\n",
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
                built: settings.target.is_some(),
            }
        );
    }

    card
}

struct Card<'a> {
    report: &'a Report<'a>,
    ladder: &'a [Rung],
    // whether the run built each plan, so the rows carry the build columns
    built: bool,
}

impl Display for Card<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Report { case, runs } = self.report;
        let sources: Vec<String> = case
            .expected
            .sources
            .iter()
            .map(|source| {
                let bound = match (&source.fixture, &source.description) {
                    (Some(fixture), _) => format!("fixture `{fixture}`"),
                    _ => "inline".to_owned(),
                };
                format!("source `{}` (adapter `{}`, {bound})", source.name(), source.adapter)
            })
            .collect();
        writeln!(f, "\n## {}\n\n{}\n", case.name, sources.join(" · "))?;

        // one row per run, at the first rung
        let build_columns =
            if self.built { " built | verified | build wall | speedup | covered |" } else { "" };
        writeln!(
            f,
            "| run | exit | wall | completions | input | cached | output | reasoning | claims | \
             req | crit | recall req | recall crit | recall dec | surfaces | stems | behaviours | \
             conflicts | divergences | unknown | covered | then [unknown] | slices | waves | \
             design blocks (types) |{build_columns}"
        )?;
        writeln!(f, "|{}", " --- |".repeat(if self.built { 30 } else { 25 }))?;
        for attempts in runs {
            if let Some(first) = attempts.first() {
                row(f, first, case.expected.sources.len() > 1)?;
                if self.built {
                    build_cells(f, first)?;
                }
                writeln!(f)?;
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

// A cell over every source: each source's reading, led by its name where
// the case has several.
fn per_source(
    grades: &[SourceGrade], named: bool, cell: impl Fn(&SourceGrade) -> String,
) -> String {
    grades
        .iter()
        .map(|grade| if named { format!("{} {}", grade.name, cell(grade)) } else { cell(grade) })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn row(f: &mut Formatter<'_>, attempt: &Attempt, named: bool) -> fmt::Result {
    let Attempt { run, grade } = attempt;
    let tokens = run.tokens();
    let (blocks, types) = run.design_blocks();
    let stems = per_source(&grade.sources, named, |source| {
        if source.missing_stems.is_empty() && source.extra_stems.is_empty() {
            "exact".to_owned()
        } else {
            format!("-{} +{}", source.missing_stems.len(), source.extra_stems.len())
        }
    });
    let surfaces = per_source(&grade.sources, named, |source| source.surfaces.to_string());

    write!(
        f,
        "| {} | {} | {}s | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | \
         {} | {} | {} | {} | {} | {} | {} ({}) |",
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
        run.evidence.iter().map(|(_, evidence)| evidence.claims.len()).sum::<usize>(),
        run.claims(ClaimKind::Requirement).count(),
        run.claims(ClaimKind::Criterion).count(),
        grade.requirements,
        grade.criteria,
        grade.decisions,
        surfaces,
        stems,
        grade.behaviours,
        run.status("conflict"),
        run.status("divergence"),
        run.status("unknown"),
        run.covered(),
        run.unknown_outcomes(),
        run.slices(),
        run.waves(),
        blocks,
        types,
    )
}

// The five build cells after a row's own: what the build merged, verified,
// took, saved over a serial walk, and covered; `—` where no plan was built.
fn build_cells(f: &mut Formatter<'_>, attempt: &Attempt) -> fmt::Result {
    let run = &attempt.run;
    let Some(build) = &run.build else {
        return write!(f, " — | — | — | — | — |");
    };
    let integrated = build.integrated();
    let conflicted = integrated.iter().filter(|slice| !slice.conflicts.is_empty()).count();
    let mut merged = format!("{}/{}", integrated.len(), run.slices());
    if conflicted > 0 {
        let _ = write!(merged, " -{conflicted}");
    }
    if build.code() == Some("slice-conflict") {
        merged.push_str(" !1");
    }
    if let Some(code) = build.code() {
        let _ = write!(merged, " `{code}`");
    }
    let verified = format!("{}/{}", build.verified(), build.waves());
    let wall = match build.exit {
        Some(0) => format!("{}s", build.wall.as_secs()),
        Some(code) => format!("{}s exit {code}", build.wall.as_secs()),
        None => format!("{}s killed", build.wall.as_secs()),
    };
    // a landed build reads against the plan's factor; one that did not says
    // how far it got, since a partial path compares with nothing
    let against = match (build.landed(), run.wave_count()) {
        (_, 0) => String::new(),
        (true, waves) => format!(" (plan {})", factor(run.slices() * 100 / waves)),
        (false, waves) => format!(" ({} of {waves} waves)", build.waves()),
    };
    let speedup = build
        .speedup()
        .map_or_else(|| "—".to_owned(), |speedup| format!("{}{against}", factor(speedup)));
    let (covered, all) = build.covered();
    write!(f, " {merged} | {verified} | {wall} | {speedup} | {covered}/{all} |")
}

// A ratio in hundredths as the card spells it, `1.40×`.
fn factor(hundredths: usize) -> String {
    format!("{}.{:02}×", hundredths / 100, hundredths % 100)
}

fn notes(f: &mut Formatter<'_>, attempt: &Attempt, climbed: bool) -> fmt::Result {
    let Attempt { run, grade } = attempt;
    let at = if climbed { format!(" @ {}", run.rung) } else { String::new() };
    let graded = !run.evidence.is_empty();
    let named = grade.sources.len() > 1;
    let led = |source: &str| if named { format!("{source}: ") } else { String::new() };
    if run.exit == Some(0) {
        let stems: Vec<String> = grade
            .sources
            .iter()
            .map(|source| format!("{}{:?}", led(&source.name), run.stems_of(&source.name)))
            .collect();
        writeln!(f, "\nRun {}{at}: stems {}", run.n, stems.join(" · "))?;
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

    for source in &grade.sources {
        survey_notes(f, run, source, &led(&source.name), graded)?;
    }
    // the claims
    if graded {
        if let Some(landing) = run.landing() {
            writeln!(f, "- requirement anchors: {landing}")?;
        }
        for (kind, recall) in [
            ("requirement", &grade.requirements),
            ("criterion", &grade.criteria),
            ("decision", &grade.decisions),
        ] {
            for missed in &recall.missed {
                writeln!(f, "- {kind} missed: {missed}")?;
            }
            for misplaced in &recall.misplaced {
                writeln!(f, "- {kind} misplaced: {misplaced}")?;
            }
        }
    }
    if grade.behaviours.expected > 0 && run.spec.is_some() {
        behaviour_notes(f, &grade.behaviours)?;
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
    if let Some(build) = &run.build {
        build_notes(f, run, build)?;
    }

    Ok(())
}

// The build beneath its run: how it ended, each wave's slices as they
// merged, what conflicted, what the verify refused, what was left
// uncovered, the chain the repository holds, and the turns it spent.
fn build_notes(f: &mut Formatter<'_>, run: &Run, build: &Build) -> fmt::Result {
    let integrated = build.integrated();
    let tokens = build.tokens();
    build_ending(f, run, build, integrated.len())?;

    // each wave's slices, as they merged
    let waves: BTreeSet<usize> = integrated.iter().map(|slice| slice.wave).collect();
    let turns = build.wave_turns();
    for wave in waves {
        let slices: Vec<String> = integrated
            .iter()
            .filter(|slice| slice.wave == wave)
            .map(|slice| {
                let merged = slice.commit.as_deref().map_or_else(
                    || "nothing to merge".to_owned(),
                    |c| format!("merged {}", short(c)),
                );
                format!(
                    "{} {} (covered {}/{}, {} files, {merged})",
                    slice.id,
                    slice.name,
                    slice.covered.len(),
                    slice.requirements.len(),
                    slice.files.len()
                )
            })
            .collect();
        let spent =
            wave.checked_sub(1).and_then(|k| turns.get(k)).map_or_else(String::new, |turns| {
                let builds: Vec<String> =
                    turns.builds.iter().map(|b| format!("{}s", b.duration.as_secs())).collect();
                let verify: Duration = turns.verifies.iter().map(|v| v.duration).sum();
                format!("; builds {} then verify {}s", builds.join(" / "), verify.as_secs())
            });
        writeln!(f, "- wave {wave}: {}{spent}", slices.join(", "))?;
    }
    for slice in integrated.iter().filter(|slice| !slice.conflicts.is_empty()) {
        writeln!(
            f,
            "- conflicted: {} at {}, merged on its second build",
            slice.id,
            slice.conflicts.join(", ")
        )?;
    }
    for failure in build.verify_failures() {
        writeln!(f, "- verify refused: {failure}")?;
    }
    for slice in integrated.iter().filter(|slice| !slice.uncovered().is_empty()) {
        writeln!(f, "- uncovered: {} {}", slice.id, slice.uncovered().join(", "))?;
    }

    // the chain, newest first
    if !build.history.is_empty() {
        let lines: Vec<String> = build
            .history
            .iter()
            .map(|landed| {
                let files = match landed.files.len() {
                    0 => String::new(),
                    n => format!(" ({n} files)"),
                };
                format!("{} {}{files}", short(&landed.commit), landed.subject)
            })
            .collect();
        writeln!(f, "- history: {}", lines.join(" · "))?;
    }

    // the turns
    writeln!(
        f,
        "- build turns: {} completions, {} input, {} cached, {} output ({} reasoning); serial {}s, \
         critical path {}s",
        build.completions.len(),
        tokens.input,
        tokens.cached,
        tokens.output,
        tokens.reasoning,
        build.serial().as_secs(),
        build.critical().as_secs(),
    )?;
    for completion in &build.completions {
        writeln!(
            f,
            "- build completion `{}` {} in {}s: {} input, {} cached, {} output ({} reasoning)",
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

// How the build ended: where the label stands and whether it was pushed,
// or the failure's code, exit, and first line.
fn build_ending(f: &mut Formatter<'_>, run: &Run, build: &Build, merged: usize) -> fmt::Result {
    let exit = build.exit.map_or_else(|| "killed".to_owned(), |code| format!("exit {code}"));
    match (&build.failure, build.exit) {
        (_, Some(0)) => {
            let envelope = build.envelope.as_ref();
            let label = envelope.and_then(|out| out["label"].as_str()).unwrap_or("");
            let head = envelope.and_then(|out| out["head"].as_str()).unwrap_or("");
            let pushed = envelope
                .and_then(|out| out["pushed"].as_str())
                .map_or_else(String::new, |remote| format!(", pushed to `{remote}`"));
            let resumed = envelope
                .and_then(|out| out["resumed"].as_array())
                .filter(|resumed| !resumed.is_empty())
                .map_or_else(String::new, |resumed| format!(", {} resumed", resumed.len()));
            writeln!(
                f,
                "\nBuild: {merged}/{} slices merged in {} waves in {}s{resumed}; `{label}` at \
                 `{}`{pushed}",
                run.slices(),
                build.waves(),
                build.wall.as_secs(),
                short(head),
            )
        }
        (Some(failure), _) => {
            let first = failure.message.lines().next().unwrap_or("");
            writeln!(
                f,
                "\nBuild failed `{}` ({exit}) after {}s, {merged}/{} slices merged: {first}",
                failure.code,
                build.wall.as_secs(),
                run.slices(),
            )
        }
        (None, _) => writeln!(
            f,
            "\nBuild failed ({exit}) after {}s with no envelope:\n\n```\n{}\n```",
            build.wall.as_secs(),
            build.tail
        ),
    }
}

// A commit as the card spells it: its first eight characters.
fn short(commit: &str) -> &str {
    commit.get(..8).unwrap_or(commit)
}

// One source's survey: the surfaces its adapter logged, what it placed
// nowhere, and how the (entry, stem) pairs and the stems read against the
// case; `led` names the source where the case has several.
fn survey_notes(
    f: &mut Formatter<'_>, run: &Run, source: &SourceGrade, led: &str, graded: bool,
) -> fmt::Result {
    if let Some(surveyed) = run.surveyed.get(&source.name) {
        let listed: Vec<String> = surveyed
            .iter()
            .map(|surface| {
                let ids: Vec<String> = surface.ids.iter().map(|id| format!("`{id}`")).collect();
                let ids = if ids.is_empty() {
                    String::new()
                } else {
                    format!(" (id {})", ids.join(", "))
                };
                format!("`{}` @ `{}` as `{}`{ids}", surface.name, surface.entry, surface.stem)
            })
            .collect();
        writeln!(f, "- {led}surfaces: {}", listed.join(", "))?;
    }
    if let Some(unreached) = run.unreached.get(&source.name) {
        writeln!(f, "- {led}unreached: {unreached} modules the survey placed under no surface")?;
    }
    if source.surfaces.surveyed {
        for restemmed in &source.surfaces.restemmed {
            writeln!(f, "- {led}surface restemmed: {restemmed}")?;
        }
        for missed in &source.surfaces.missed {
            writeln!(f, "- {led}surface missed: {missed}")?;
        }
        if source.surfaces.expected > 0 {
            for extra in &source.surfaces.extra {
                writeln!(f, "- {led}surface extra: {extra}")?;
            }
        }
    } else if source.surfaces.expected > 0 {
        writeln!(f, "- {led}no survey: the adapter cut its seams without deciding a surface")?;
    }
    if graded {
        if !source.missing_stems.is_empty() {
            writeln!(f, "- {led}stems missing: {:?}", source.missing_stems)?;
        }
        if !source.extra_stems.is_empty() {
            writeln!(f, "- {led}stems extra: {:?}", source.extra_stems)?;
        }
    }

    Ok(())
}

// The reconciled requirements against the behaviours expected.
fn behaviour_notes(f: &mut Formatter<'_>, behaviours: &Behaviours) -> fmt::Result {
    if behaviours.status_expected > 0 {
        writeln!(
            f,
            "- behaviour statuses: {}/{} as expected",
            behaviours.status_met, behaviours.status_expected
        )?;
    }
    for (what, listed) in [
        ("missed", &behaviours.missed),
        ("split", &behaviours.split),
        ("partial", &behaviours.partial),
        ("met by criterion", &behaviours.by_criterion),
        ("across kinds", &behaviours.across),
        ("status", &behaviours.status_wrong),
        ("wrong merge", &behaviours.wrong),
    ] {
        for item in listed {
            writeln!(f, "- behaviour {what}: {item}")?;
        }
    }
    if behaviours.premerged.is_empty() {
        writeln!(f, "- cross-source pre-merges: none")
    } else {
        writeln!(
            f,
            "- cross-source pre-merges: {} — {}",
            behaviours.premerged.len(),
            behaviours.premerged.join("; ")
        )
    }
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

    // the stems each run's requirements lead with, as sets of (source, stem)
    let sets: BTreeSet<BTreeSet<(&str, &str)>> = runs.iter().map(|run| run.stems()).collect();
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
        if !self.surveyed {
            return f.write_str("no survey");
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

// `met/expected`, then `~k` behaviours split across requirements and `!k`
// requirements merging two, each where any.
impl Display for Behaviours {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.expected == 0 {
            return f.write_str("—");
        }
        write!(f, "{}/{}", self.met, self.expected)?;
        if !self.split.is_empty() {
            write!(f, " ~{}", self.split.len())?;
        }
        if !self.wrong.is_empty() {
            write!(f, " !{}", self.wrong.len())?;
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
    // SDK log them, with the specification `show` would answer.
    fn run_from(exit: Option<i32>, stderr: &str, spec: Option<Value>) -> Run {
        Run {
            n: 1,
            rung: RUNG,
            wall: Duration::ZERO,
            exit,
            tail: String::new(),
            diff: None,
            waves: None,
            evidence: accepted(stderr),
            completions: completions(stderr),
            surveyed: surveyed(stderr),
            unreached: unreached(stderr),
            spec,
            design: None,
            plan: None,
            dead: None,
            build: None,
        }
    }

    fn completion(label: &str, outcome: &str) -> String {
        timed(label, outcome, 10)
    }

    fn timed(label: &str, outcome: &str, duration_ms: u64) -> String {
        format!(
            "INFO complete{{n=1 model=\"m\" format=schema label=\"{label}\"}}: \
             omnia_cursor::model::observe: completion outcome=\"{outcome}\" attempts=1 \
             duration_ms={duration_ms} input_tokens=5 cache_read_tokens=0 output_tokens=2 \
             reasoning_tokens=0\n"
        )
    }

    // A build read from its log and envelope, as `build` reads them, over
    // the chain `history` would answer.
    fn build_from(exit: i32, stdout: &str, stderr: &str, history: Vec<Landed>) -> Build {
        Build {
            wall: Duration::ZERO,
            exit: Some(exit),
            envelope: serde_json::from_str(stdout).ok(),
            failure: failure(stderr),
            completions: completions(stderr),
            history,
            tail: String::new(),
        }
    }

    const LANDED: &str = r#"{
  "revision": "feb4bd36",
  "waves": [["SLICE-001"], ["SLICE-002", "SLICE-003"]],
  "base": "1111111111111111111111111111111111111111",
  "slices": [
    {"id": "SLICE-001", "name": "start", "wave": 1, "covered": ["REQ-001"], "uncovered": [],
     "written": ["src/index.ts", "src/start/index.ts"], "commit": "aaaaaaaa"},
    {"id": "SLICE-002", "name": "orders", "wave": 2, "covered": ["REQ-002"], "uncovered": ["REQ-003"],
     "written": ["src/orders/index.ts"], "commit": "bbbbbbbb"},
    {"id": "SLICE-003", "name": "payments", "wave": 3, "covered": ["REQ-004", "REQ-005"], "uncovered": [],
     "written": ["src/payments/index.ts"], "commit": "cccccccc", "conflicts": ["src/index.ts"]}
  ],
  "verified": ["aaaaaaaa", "bbbbbbbb", "cccccccc"],
  "head": "cccccccc",
  "label": "emery/feb4bd36",
  "pushed": "origin"
}"#;

    fn accepted_line(source: &str, claims: &str) -> String {
        format!(
            "TRACE emery_sdk: accepted source={source} seam=0 evidence={{\"claims\":[{claims}]}}\n"
        )
    }

    fn requirement(id: &str, path: &str) -> String {
        format!(
            "{{\"kind\":\"requirement\",\"id\":\"{id}\",\"path\":\"{path}\",\"statement\":\"s\"}}"
        )
    }

    fn criterion(id: &str, path: &str) -> String {
        format!(
            "{{\"kind\":\"criterion\",\"id\":\"{id}\",\"path\":\"{path}\",\"criterion\":\"c\"}}"
        )
    }

    fn case(toml: &str) -> Result<Expected, String> {
        let expected: Expected = toml::from_str(toml).map_err(|error| error.to_string())?;
        expected.check()?;
        Ok(expected)
    }

    // The error a case's text is refused with.
    fn refused(toml: &str) -> String {
        match case(toml) {
            Ok(_) => panic!("the case parsed: {toml}"),
            Err(error) => error,
        }
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
            None,
        );
        assert!(starved.starved());
        assert!(!starved.passed());
        let landed = run_from(Some(0), &completion("evidence-python-0", "ok"), None);
        assert!(!landed.starved());
        assert!(landed.passed());
    }

    #[test]
    fn stillborn_run() {
        let refused = run_from(Some(4), &completion("survey-python", "error"), None);
        assert!(refused.stillborn());
        assert_eq!(refused.died_in(), Some("survey-python"));

        let after_claims = run_from(
            Some(4),
            &(completion("evidence-python-0", "ok")
                + &accepted_line("python", &requirement("orders.create", "app.py#L3-L9"))
                + &completion("grouping", "error")),
            None,
        );
        assert!(!after_claims.stillborn(), "a claim accepted is a run to grade");
        assert_eq!(after_claims.died_in(), Some("grouping"));

        let engine = run_from(Some(1), &completion("survey-python", "ok"), None);
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
        let run = run_from(Some(1), &stderr, None);
        assert_eq!(run.died_in(), Some("spec-draft"), "the aborted siblings are not where it died");
    }

    #[test]
    fn build_landed() {
        let stderr = timed("build-SLICE-001", "ok", 100_000)
            + &timed("verify", "ok", 20_000)
            + &timed("build-SLICE-002", "ok", 60_000)
            + &timed("build-SLICE-003", "ok", 90_000)
            + &timed("verify", "ok", 30_000)
            + &timed("build-SLICE-003", "corrected", 50_000)
            + &timed("verify", "ok", 10_000);
        let build = build_from(0, LANDED, &stderr, Vec::new());
        assert!(build.landed());
        assert_eq!(build.code(), None);
        let integrated = build.integrated();
        assert_eq!(integrated.len(), 3);
        assert_eq!(integrated[1].uncovered(), ["REQ-003"]);
        assert_eq!(integrated[2].conflicts, ["src/index.ts"]);
        assert_eq!(build.waves(), 3);
        assert_eq!(build.verified(), 3);
        assert_eq!(build.covered(), (4, 5));
        assert_eq!(build.verify_failures(), [] as [&str; 0]);
    }

    // The serial walk is every turn in a row; the critical path takes each
    // wave at its slowest slice, then its verify.
    #[test]
    fn build_critical_path() {
        let stderr = timed("build-SLICE-001", "ok", 100_000)
            + &timed("verify", "ok", 20_000)
            + &timed("build-SLICE-002", "ok", 60_000)
            + &timed("build-SLICE-003", "ok", 90_000)
            + &timed("verify", "ok", 30_000)
            + &timed("verify", "corrected", 5_000);
        let build = build_from(0, "{}", &stderr, Vec::new());
        let waves = build.wave_turns();
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[1].builds.len(), 2);
        assert_eq!(waves[1].verifies.len(), 2, "a verify's correction round closes the same wave");
        assert_eq!(build.serial(), Duration::from_secs(305));
        assert_eq!(build.critical(), Duration::from_secs(100 + 20 + 90 + 35));
        assert_eq!(build.speedup(), Some(124));
        assert_eq!(factor(124), "1.24×");
        assert_eq!(factor(300), "3.00×");
    }

    // Two rounds of one slice's build add up on its wave's path; a wave
    // the failure ended has builds and no verify.
    #[test]
    fn build_rounds_and_open_wave() {
        let stderr = timed("build-SLICE-001", "corrected", 40_000)
            + &timed("build-SLICE-001", "ok", 30_000)
            + &timed("verify", "ok", 10_000)
            + &timed("build-SLICE-002", "ok", 25_000);
        let build = build_from(1, "", &stderr, Vec::new());
        let waves = build.wave_turns();
        assert_eq!(waves.len(), 2);
        assert_eq!(waves[0].critical(), Duration::from_secs(80));
        assert_eq!(waves[1].critical(), Duration::from_secs(25));
        assert_eq!(build.waves(), 2);
    }

    #[test]
    fn build_failure_envelope() {
        let stderr = timed("build-SLICE-001", "ok", 1_000)
            + &timed("verify", "ok", 1_000)
            + "2026-10-09T03:53:16Z  INFO emery_sdk::target: verified passed=false failures=2\n\
               {\n  \"error\": \"verify-failed\",\n  \"message\": \"wave 2 failed; SLICE-002 merged \
               in it stays committed in `.emery/vcs/integration`; `emery/feb4` stays at `aaaa`: \
               verification failed:\\n- check 2 (tsc): src/orders/index.ts(3,1): error TS2304\\n- \
               check 3 (npm test): not run\",\n  \"exit-code\": 1\n}\n\
               2026-10-09T03:53:17Z  INFO omnia_core::runtime::command: wasi:cli/run exited code=1\n";
        let history = vec![
            Landed {
                commit: "bbbbbbbbbbbb".to_owned(),
                subject: "SLICE-002 orders".to_owned(),
                slice: Some("SLICE-002".to_owned()),
                wave: Some(2),
                requirements: vec!["REQ-002".to_owned(), "REQ-003".to_owned()],
                covered: vec!["REQ-002".to_owned()],
                files: vec!["src/orders/index.ts".to_owned()],
            },
            Landed {
                commit: "aaaaaaaaaaaa".to_owned(),
                subject: "SLICE-001 start".to_owned(),
                slice: Some("SLICE-001".to_owned()),
                wave: Some(1),
                requirements: vec!["REQ-001".to_owned()],
                covered: vec!["REQ-001".to_owned()],
                files: vec!["src/index.ts".to_owned()],
            },
            Landed {
                commit: "000000000000".to_owned(),
                subject: "Greenfield".to_owned(),
                slice: None,
                wave: None,
                requirements: Vec::new(),
                covered: Vec::new(),
                files: vec![".gitignore".to_owned(), "README.md".to_owned()],
            },
        ];
        let build = build_from(1, "", &stderr, history);
        assert!(!build.landed());
        assert_eq!(build.code(), Some("verify-failed"));
        assert_eq!(build.wave_failed(), Some(2));
        assert_eq!(build.verified(), 1);
        assert_eq!(
            build.verify_failures(),
            [
                "check 2 (tsc): src/orders/index.ts(3,1): error TS2304",
                "check 3 (npm test): not run"
            ]
        );
        let integrated = build.integrated();
        assert_eq!(integrated.len(), 2, "the greenfield root is no slice");
        assert_eq!(integrated[0].id, "SLICE-001");
        assert_eq!(integrated[1].name, "orders");
        assert_eq!(integrated[1].uncovered(), ["REQ-003"]);
        assert_eq!(build.covered(), (2, 3));
    }

    #[test]
    fn build_slice_conflict_wave() {
        let stderr = "{\n  \"error\": \"slice-conflict\",\n  \"message\": \"slice `SLICE-003` \
                      (payments) failed in wave 3; SLICE-002 merged before it stays committed in \
                      `.emery/vcs/integration`: its merge conflicts at package.json, src/index.ts \
                      on its build 2\",\n  \"exit-code\": 1\n}\n";
        let build = build_from(1, "", stderr, Vec::new());
        assert_eq!(build.code(), Some("slice-conflict"));
        assert_eq!(build.wave_failed(), Some(3));
        assert_eq!(build.verified(), 2);
        assert_eq!(build.verify_failures(), [] as [&str; 0]);
        assert_eq!(build.speedup(), None, "no turn read, no path to measure");
    }

    #[test]
    fn build_chain() {
        let text = "\x1ecccc\x1fWave 2 verified\x1f\x1f\x1f\x1f\x1f\n\npackage-lock.json\n\
                    \x1ebbbb\x1fSLICE-002 orders\x1fSLICE-002\n\x1f2\n\x1fREQ-002, REQ-003\n\x1fREQ-002\n\
                    \x1f\n\nsrc/index.ts\nsrc/orders/index.ts\n\
                    \x1eaaaa\x1fGreenfield\x1f\x1f\x1f\x1f\x1f\n\n.gitignore\nREADME.md\n";
        let chain = chain(text);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].slice, None);
        assert_eq!(chain[0].files, ["package-lock.json"]);
        assert_eq!(chain[1].slice.as_deref(), Some("SLICE-002"));
        assert_eq!(chain[1].wave, Some(2));
        assert_eq!(chain[1].requirements, ["REQ-002", "REQ-003"]);
        assert_eq!(chain[1].covered, ["REQ-002"]);
        assert_eq!(chain[1].files, ["src/index.ts", "src/orders/index.ts"]);
        assert_eq!(chain[2].subject, "Greenfield");
        assert_eq!(chain[2].files.len(), 2);
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
        let run =
            run_from(Some(0), &(surveyed.to_owned() + &accepted_line("python", &claims)), None);
        assert_eq!(
            run.landing(),
            Some(Landing {
                head: 2,
                inside: 2,
                outside: 3
            }),
            "the bootstrap's span is set aside, so `start.boot` at its line is outside"
        );

        let unlined = run_from(Some(0), &accepted_line("python", &claims), None);
        assert_eq!(unlined.landing(), None, "no surface span logged, nothing to land against");
    }

    // Two sources' lines are read apart: each source's claims, surfaces, and
    // unplaced count under its own name, and a span of one source never lands
    // a claim of the other.
    #[test]
    fn two_sources_read_apart() {
        let stderr = "TRACE typescript: surveyed source=code surfaces=[\
            {\"name\":\"OrderService\",\"entry\":\"orders.ts\",\"stem\":\"order-service\",\"ids\":[],\"lines\":\"L35-L76\"}]\n\
            INFO emery_sdk: placed by model source=code surfaces=1 unplaced=0\n\
            INFO emery_sdk: surveyed by model source=docs surfaces=3 unreached=1\n"
            .to_owned()
            + &accepted_line("code", &requirement("order-service.place", "orders.ts#L40-L42"))
            + &accepted_line("docs", &requirement("orders.place", "orders.md#L10-L16"))
            + &accepted_line("docs", &requirement("orders.cancel", "orders.ts#L40-L42"));
        let run = run_from(Some(0), &stderr, None);
        assert_eq!(run.claims_of("code", ClaimKind::Requirement).count(), 1);
        assert_eq!(run.claims_of("docs", ClaimKind::Requirement).count(), 2);
        assert_eq!(run.stems_of("docs"), BTreeSet::from(["orders"]));
        assert_eq!(run.pairs_of("code"), BTreeSet::from([("orders.ts", "order-service")]));
        assert!(run.pairs_of("docs").is_empty());
        assert_eq!(run.unreached.get("code"), Some(&0));
        assert_eq!(
            run.unreached.get("docs"),
            Some(&1),
            "the SDK's count stands where the adapter logs none"
        );
        assert_eq!(
            run.landing(),
            Some(Landing {
                head: 0,
                inside: 1,
                outside: 2
            }),
            "the docs claim at `orders.ts` lands against no span of its own source"
        );
    }

    #[test]
    fn source_shapes() {
        let one = case("[[source]]\nadapter = \"python\"\nfixture = \"f\"\nstems = [\"a\"]\n\n[[requirement]]\nanchor = \"a.py#L1\"\ngloss = \"g\"\n")
            .expect("one source, the item's by default");
        assert_eq!(one.sources[0].name(), "python", "a source unnamed is named for its adapter");
        assert_eq!(one.source(None).unwrap().name(), "python");
        assert_eq!(one.sources[0].rank, None, "a source unranked leaves the rank to its kind");

        let ranked = case(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"faq\"\nadapter = \"documentation\"\nfixture = \"f\"\nrank = 4\n",
        )
        .expect("a ranked source");
        assert_eq!(ranked.sources[0].rank, None);
        assert_eq!(ranked.sources[1].rank, NonZeroU32::new(4));

        let zero = refused("[[source]]\nadapter = \"python\"\nfixture = \"f\"\nrank = 0\n");
        assert!(zero.contains("nonzero"), "zero is no rank, as the engine has it: {zero}");

        let several = refused(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"brief\"\nadapter = \"intent\"\ndescription = \"Ship it.\"\n\n\
             [[requirement]]\nanchor = \"a.md#L1\"\ngloss = \"g\"\n",
        );
        assert!(several.contains("names no source"), "an item of several sources names its own");

        let both =
            refused("[[source]]\nadapter = \"intent\"\nfixture = \"f\"\ndescription = \"d\"\n");
        assert!(both.contains("not both or neither"));

        let unknown = refused(
            "[[source]]\nadapter = \"python\"\nfixture = \"f\"\n\n[[behaviour]]\ngloss = \"g\"\nstatus = \"agreed\"\nclaims = [{ source = \"docs\", anchor = \"a.md#L1\" }]\n",
        );
        assert!(unknown.contains("names no source of the case: `docs`"));

        let status = refused(
            "[[source]]\nadapter = \"python\"\nfixture = \"f\"\n\n[[behaviour]]\ngloss = \"g\"\nstatus = \"settled\"\nclaims = [{ source = \"python\", anchor = \"a.py#L1\" }]\n",
        );
        assert!(status.contains("`settled`"));

        let two = "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
                   [[source]]\nname = \"faq\"\nadapter = \"documentation\"\nfixture = \"f\"\n\n";
        let unlosing = refused(&format!(
            "{two}[[behaviour]]\ngloss = \"g\"\nstatus = \"agreed\"\nloser = \"faq\"\n\
             claims = [{{ source = \"docs\", anchor = \"a.md#L1\" }}, {{ source = \"faq\", anchor = \"b.md#L1\" }}]\n"
        ));
        assert!(unlosing.contains("under no disagreement"), "{unlosing}");
        let unstated = refused(&format!(
            "{two}[[behaviour]]\ngloss = \"g\"\nloser = \"faq\"\n\
             claims = [{{ source = \"docs\", anchor = \"a.md#L1\" }}, {{ source = \"faq\", anchor = \"b.md#L1\" }}]\n"
        ));
        assert!(unstated.contains("under no disagreement"), "{unstated}");
        let stranger = refused(&format!(
            "{two}[[behaviour]]\ngloss = \"g\"\nstatus = \"divergence\"\nloser = \"faq\"\n\
             claims = [{{ source = \"docs\", anchor = \"a.md#L1\" }}]\n"
        ));
        assert!(stranger.contains("none of its claims is from"), "{stranger}");
        case(&format!(
            "{two}[[behaviour]]\ngloss = \"g\"\nstatus = \"divergence\"\nloser = \"faq\"\n\
             claims = [{{ source = \"docs\", anchor = \"a.md#L1\" }}, {{ source = \"faq\", anchor = \"b.md#L1\" }}]\n"
        ))
        .expect("a loser among the claims' sources under a disagreement");
    }

    // Every committed case parses under the shape, with every item at a
    // source of its case.
    #[test]
    fn committed_cases_parse() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut seen = 0;
        for entry in fs::read_dir(root.join(CASES)).unwrap() {
            let dir = entry.unwrap().path();
            let Ok(text) = fs::read_to_string(dir.join("expected.toml")) else { continue };
            case(&text).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
            seen += 1;
        }
        assert!(seen > 0, "the cases are beside the runner");
    }

    fn spec(requirements: &Value) -> Value {
        serde_json::json!({ "requirements": requirements })
    }

    fn cited(source: &str, claim: &str, path: &str) -> Value {
        serde_json::json!({ "source": source, "claim": claim, "path": path })
    }

    #[test]
    fn behaviour_grading() {
        let expected = case(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"code\"\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[behaviour]]\ngloss = \"placing an empty order is refused\"\nstatus = \"agreed\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L12\" }, { source = \"code\", anchor = \"orders.ts#L40-L42\" }]\n\n\
             [[behaviour]]\ngloss = \"cancelling a shipped order is refused\"\nstatus = \"divergence\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L30-L32\" }, { source = \"code\", anchor = \"orders.ts#L67-L71\" }]\n\n\
             [[behaviour]]\ngloss = \"an unknown order is not found\"\n\
             claims = [{ source = \"code\", anchor = \"orders.ts#L59-L65\" }]\n\n\
             [[behaviour]]\ngloss = \"the id is opaque\"\nstatus = \"unknown\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L14\" }]\n",
        )
        .unwrap();
        let spec = spec(&serde_json::json!([
            // the first behaviour, one requirement citing both sources, agreed
            { "subject": "orders.place", "status": "agreed",
              "sources": [cited("docs", "orders.place", "orders.md#L12"), cited("code", "order-service.place.empty", "orders.ts#L40-L42")] },
            // the second behaviour split across two requirements, the status read off either
            { "subject": "orders.cancel", "status": "divergence",
              "sources": [cited("docs", "orders.cancel", "orders.md#L31"), cited("code", "order-service.cancel.state", "orders.ts#L72-L74")] },
            { "subject": "order-service.cancel", "status": "agreed",
              "sources": [cited("code", "order-service.cancel.shipped", "orders.ts#L67-L71")] },
            // the third and fourth merged into one, a pre-merge on a shared id
            { "subject": "orders.read", "status": "agreed",
              "sources": [cited("docs", "orders.read", "orders.md#L14"), cited("code", "orders.read", "orders.ts#L59-L65")] },
        ]));
        let run = run_from(Some(0), "", Some(spec));

        let graded = behaviours(&expected.behaviours, &run);
        assert_eq!(graded.met, 4);
        assert_eq!(graded.split.len(), 1, "{:?}", graded.split);
        assert!(graded.split[0].contains("cancelling a shipped order"));
        assert_eq!(graded.wrong.len(), 1, "{:?}", graded.wrong);
        assert!(graded.wrong[0].starts_with("`orders.read` cites"));
        assert!(graded.partial.is_empty(), "{:?}", graded.partial);
        assert_eq!((graded.status_met, graded.status_expected), (2, 3));
        assert_eq!(graded.status_wrong.len(), 1);
        assert!(graded.status_wrong[0].contains("expected `unknown`, got {\"agreed\"}"));
        assert_eq!(
            graded.premerged,
            vec!["`orders.read` on `orders.read` from {\"code\", \"docs\"}"]
        );
        assert_eq!(graded.to_string(), "4/4 ~1 !1");
    }

    #[test]
    fn behaviour_missed_and_partial() {
        let expected = case(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"code\"\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[behaviour]]\ngloss = \"both state it\"\n\
             claims = [{ source = \"docs\", anchor = \"a.md#L1\" }, { source = \"code\", anchor = \"a.ts#L1\" }]\n\n\
             [[behaviour]]\ngloss = \"neither states it\"\n\
             claims = [{ source = \"docs\", anchor = \"a.md#L9\" }]\n",
        )
        .unwrap();
        let spec = spec(&serde_json::json!([
            { "subject": "a", "status": "unknown", "sources": [cited("docs", "a.one", "a.md#L1")] },
        ]));
        let run = run_from(Some(0), "", Some(spec));

        let graded = behaviours(&expected.behaviours, &run);
        assert_eq!(graded.met, 1);
        assert_eq!(graded.missed.len(), 1);
        assert_eq!(graded.partial.len(), 1, "{:?}", graded.partial);
        assert!(graded.partial[0].ends_with("(uncited code:`a.ts#L1`)"));
        assert_eq!(graded.premerged, Vec::<String>::new());
        assert_eq!(graded.to_string(), "1/2");
    }

    // The planted disagreement is between the document and the FAQ, so a
    // divergence the code lost, in a requirement the FAQ's claim never
    // reached, is not the status expected.
    #[test]
    fn behaviour_status_loser() {
        let expected = case(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"faq\"\nadapter = \"documentation\"\nfixture = \"f\"\nrank = 4\n\n\
             [[source]]\nname = \"code\"\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[behaviour]]\ngloss = \"which orders can be cancelled\"\nstatus = \"divergence\"\nloser = \"faq\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L31-L33\" }, { source = \"faq\", anchor = \"faq.md#L9-L12\" }, { source = \"code\", anchor = \"orders.ts#L67-L74\" }]\n",
        )
        .unwrap();
        let graded = |requirements: Value| {
            behaviours(&expected.behaviours, &run_from(Some(0), "", Some(spec(&requirements))))
        };
        let losing = |sources: &[&str]| -> Value {
            sources.iter().map(|source| serde_json::json!({ "sources": [source] })).collect()
        };
        let docs = cited("docs", "orders.cancel", "orders.md#L31");
        let faq = cited("faq", "orders.cancellation", "faq.md#L9-L10");
        let code = cited("code", "order-service.cancel", "orders.ts#L72-L74");

        // the code the loser, the FAQ's claim apart: the key never decided it
        let apart = graded(serde_json::json!([
            { "subject": "orders.cancel", "status": "divergence", "losers": losing(&["code"]),
              "sources": [docs, code] },
            { "subject": "orders.cancellation", "status": "unknown", "losers": [], "sources": [faq] },
        ]));
        assert_eq!((apart.status_met, apart.status_expected), (0, 1));
        assert_eq!(apart.status_wrong.len(), 1);
        assert!(
            apart.status_wrong[0].ends_with(
                "(expected `divergence losing faq`, got {\"divergence losing code\", \"unknown\"})"
            ),
            "{}",
            apart.status_wrong[0]
        );

        // the FAQ the loser beside another: the key decided it
        let ranked = graded(serde_json::json!([
            { "subject": "orders.cancel", "status": "divergence", "losers": losing(&["code", "faq"]),
              "sources": [docs, faq, code] },
        ]));
        assert_eq!((ranked.status_met, ranked.status_expected), (1, 1));
        assert!(ranked.status_wrong.is_empty(), "{:?}", ranked.status_wrong);

        // a source losing twice is named once
        let twice = graded(serde_json::json!([
            { "subject": "orders.cancel", "status": "conflict", "losers": losing(&["code", "docs", "code"]),
              "sources": [docs, faq, code] },
        ]));
        assert!(
            twice.status_wrong[0].ends_with("got {\"conflict losing code, docs\"})"),
            "{}",
            twice.status_wrong[0]
        );

        // the FAQ's claim grouped in and the code still the one loser
        let unturned = graded(serde_json::json!([
            { "subject": "orders.cancel", "status": "divergence", "losers": losing(&["code"]),
              "sources": [docs, faq] },
        ]));
        assert_eq!((unturned.status_met, unturned.status_expected), (0, 1));
    }

    // A citation over a whole method spans the branches inside it without
    // stating any; one a line wider than the sentence it cites states it.
    #[test]
    fn behaviour_wide_citations() {
        let expected = case(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"code\"\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[behaviour]]\ngloss = \"an order with no line items is refused\"\n\
             claims = [{ source = \"code\", anchor = \"orders.ts#L40-L42\" }]\n\n\
             [[behaviour]]\ngloss = \"a line quantity below one is refused\"\n\
             claims = [{ source = \"code\", anchor = \"orders.ts#L43-L45\" }]\n\n\
             [[behaviour]]\ngloss = \"a valid order is stored pending and answered\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L10-L13\" }, { source = \"code\", anchor = \"orders.ts#L46-L56\" }]\n\n\
             [[behaviour]]\ngloss = \"an order is in exactly one of four states\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L21\" }]\n\n\
             [[behaviour]]\ngloss = \"the id is opaque\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L17\" }]\n",
        )
        .unwrap();
        let spec = spec(&serde_json::json!([
            // the happy path cited over the whole method, validation and all
            { "subject": "orders.place", "status": "agreed",
              "sources": [cited("docs", "orders.place", "orders.md#L10-L13"), cited("code", "order-service.place.create-pending-order", "orders.ts#L39-L57")] },
            { "subject": "order-service.place.empty", "status": "unknown",
              "sources": [cited("code", "order-service.place.empty", "orders.ts#L40-L42")] },
            { "subject": "order-service.place.quantity", "status": "unknown",
              "sources": [cited("code", "order-service.place.quantity", "orders.ts#L43-L45")] },
            // the sentence runs onto a second line the expected anchor leaves out
            { "subject": "orders.state", "status": "unknown",
              "sources": [cited("docs", "orders.state", "orders.md#L21-L22")] },
            // three asides cited as one, the behaviour one line of the three
            { "subject": "orders.place.asides", "status": "unknown",
              "sources": [cited("docs", "orders.place.asides", "orders.md#L15-L17")] },
        ]));
        let run = run_from(Some(0), "", Some(spec));

        let graded = behaviours(&expected.behaviours, &run);
        assert_eq!(graded.met, 4);
        assert_eq!(graded.missed.len(), 1, "{:?}", graded.missed);
        assert!(graded.missed[0].contains("the id is opaque"));
        assert!(graded.split.is_empty(), "{:?}", graded.split);
        assert!(graded.wrong.is_empty(), "{:?}", graded.wrong);
        assert!(graded.partial.is_empty(), "{:?}", graded.partial);
        assert_eq!(graded.to_string(), "4/5");
    }

    // The model may state a behaviour as a criterion of the rule it qualifies
    // rather than as a rule of its own; the grader reads it under the
    // requirement citing the criterion's parent id, and under no other.
    #[test]
    fn behaviour_met_by_criterion() {
        let expected = case(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"code\"\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[behaviour]]\ngloss = \"the id is opaque\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L14\" }]\n\n\
             [[behaviour]]\ngloss = \"a quantity below one is refused\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L13\" }]\n",
        )
        .unwrap();
        let stderr = accepted_line(
            "docs",
            &[
                requirement("orders.place", "orders.md#L10-L12"),
                criterion("orders.place.opaque-id", "orders.md#L14"),
                criterion("orders.state.quantity", "orders.md#L13"),
            ]
            .join(","),
        );
        let spec = spec(&serde_json::json!([
            { "subject": "orders.place", "status": "agreed",
              "sources": [cited("docs", "orders.place", "orders.md#L10-L12")] },
        ]));
        let run = run_from(Some(0), &stderr, Some(spec));

        let graded = behaviours(&expected.behaviours, &run);
        assert_eq!(graded.met, 1);
        assert_eq!(graded.missed.len(), 1, "{:?}", graded.missed);
        assert!(graded.missed[0].contains("a quantity below one"));
        assert_eq!(graded.by_criterion.len(), 1, "{:?}", graded.by_criterion);
        assert!(
            graded.by_criterion[0].ends_with("(`orders.place.opaque-id` under `orders.place`)"),
            "{:?}",
            graded.by_criterion
        );
        assert!(graded.partial.is_empty(), "{:?}", graded.partial);
        assert!(graded.split.is_empty(), "{:?}", graded.split);
        assert_eq!(graded.to_string(), "1/2");
    }

    // A behaviour the document states as a criterion and the code as a
    // requirement is held twice, in two kinds the engine cannot join: it is
    // met and noted, and it is neither a split nor a wrong merge of the
    // requirement holding the criterion.
    #[test]
    fn behaviour_across_kinds() {
        let expected = case(
            "[[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[source]]\nname = \"code\"\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[behaviour]]\ngloss = \"a valid order is placed\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L10-L12\" }]\n\n\
             [[behaviour]]\ngloss = \"an empty order is refused\"\nstatus = \"unknown\"\n\
             claims = [{ source = \"docs\", anchor = \"orders.md#L13\" }, { source = \"code\", anchor = \"orders.ts#L40-L42\" }]\n",
        )
        .unwrap();
        let stderr = accepted_line(
            "docs",
            &[
                requirement("orders.place", "orders.md#L10-L12"),
                criterion("orders.place.empty", "orders.md#L13"),
            ]
            .join(","),
        ) + &accepted_line(
            "code",
            &requirement("order-service.place.empty", "orders.ts#L40-L42"),
        );
        let spec = spec(&serde_json::json!([
            { "subject": "orders.place", "status": "agreed",
              "sources": [cited("docs", "orders.place", "orders.md#L10-L12")] },
            { "subject": "order-service.place.empty", "status": "unknown",
              "sources": [cited("code", "order-service.place.empty", "orders.ts#L40-L42")] },
        ]));
        let run = run_from(Some(0), &stderr, Some(spec));

        let graded = behaviours(&expected.behaviours, &run);
        assert_eq!(graded.met, 2);
        assert!(graded.split.is_empty(), "{:?}", graded.split);
        assert!(graded.wrong.is_empty(), "{:?}", graded.wrong);
        assert!(graded.by_criterion.is_empty(), "{:?}", graded.by_criterion);
        assert!(graded.partial.is_empty(), "{:?}", graded.partial);
        assert_eq!(graded.across.len(), 1, "{:?}", graded.across);
        assert!(
            graded.across[0].ends_with(
                "(`orders.place.empty` under `orders.place`; cited by [\"order-service.place.empty\"])"
            ),
            "{:?}",
            graded.across
        );
        assert_eq!((graded.status_met, graded.status_expected), (1, 1));
        assert_eq!(graded.to_string(), "2/2");
    }
}
