//! Live evaluation runner for shipped source and target adapters.
//!
//! Stages cases from `evals/cases/` under `target/eval/<card>/`, runs
//! `emery specify` and optional `emery build` passes, and writes the graded
//! scorecard to `target/eval/<card>.md`. See `evals/README.md` for flags.

#![expect(
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

use emery_sdk::{Anchor, Claim, ClaimKind, Evidence, Lexicon};
use serde::Deserialize;
use serde_json::Value;

mod reference;

use reference::Reference;

const CASES: &str = "evals/cases";
const LADDER: &str = "600/120,1200/240,2400/480";
// A build's turn lays a slice and repairs its findings, so it runs at the
// ladder's second rung from the start rather than losing a slice to the
// first.
const BUILD_RUNG: &str = "1200/240";
const BOOTSTRAP: &str = "start";

// The run's log filter unless the caller sets one: every adapter the case
// runs under at trace beside the SDK's, and the backend's tool-call
// arguments whole, which it logs under a target of their own. The crate is
// `emery-<kind>`, so the tracing target is `emery_<kind>`.
fn rust_log<'a>(adapters: impl IntoIterator<Item = &'a str>) -> String {
    let mut filter = "emery_sdk=trace".to_owned();
    for adapter in adapters {
        let _ = write!(filter, ",emery_{adapter}=trace");
    }
    filter + ",omnia_cursor=info,omnia_cursor::tools=trace,omnia_core=off"
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

// What the command line asks for: the cases or adapters named, and the
// mode (`--facts`, `--build`, `--rebuild <card>`, `--oracle <card>`) with
// the target a build goes through.
struct Args {
    filter: BTreeSet<String>,
    facts_only: bool,
    rebuild: Option<String>,
    // the card whose `omnia` build the reference's fixtures are driven through
    oracle: Option<String>,
    // the target a build runs through; none where no build runs
    target: Option<Target>,
}

impl Args {
    fn parse(argv: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut argv = argv;
        let mut filter: BTreeSet<String> = BTreeSet::new();
        let mut rebuild: Option<String> = None;
        let mut oracle: Option<String> = None;
        let mut target: Option<Target> = None;
        while let Some(arg) = argv.next() {
            if arg == "--rebuild" {
                rebuild = Some(argv.next().ok_or("`--rebuild` names a card")?);
            } else if arg == "--oracle" {
                oracle = Some(argv.next().ok_or("`--oracle` names a card")?);
            } else if arg == "--target" {
                let name = argv.next().ok_or("`--target` names a target adapter")?;
                target = Some(Target::parse(&name)?);
            } else {
                filter.insert(arg);
            }
        }
        let facts_only = filter.remove("--facts");
        let build = filter.remove("--build") || rebuild.is_some();
        if facts_only && filter.is_empty() {
            return Err("`--facts` names a case or an adapter".into());
        }
        if facts_only && build {
            return Err("`--facts` spends no turn, so it cannot `--build`".into());
        }
        if rebuild.is_some() && filter.is_empty() {
            return Err("`--rebuild` names a case or an adapter".into());
        }
        if oracle.is_some() {
            if filter.is_empty() {
                return Err("`--oracle` names a case or an adapter".into());
            }
            if facts_only || build {
                return Err(
                    "`--oracle` spends no turn and builds nothing, so it cannot `--facts`, \
                            `--build`, or `--rebuild`"
                        .into(),
                );
            }
            if target.is_some_and(|target| target != Target::Omnia) {
                return Err("`--oracle` reads an `omnia` build; `--target` names another".into());
            }
        } else if target.is_some() && !build {
            return Err("`--target` chooses what `--build` or `--rebuild` builds through".into());
        }
        Ok(Self {
            filter,
            facts_only,
            rebuild,
            oracle,
            target: build.then(|| target.unwrap_or_default()),
        })
    }
}

#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
fn eval() -> Result<(), Box<dyn std::error::Error>> {
    // the crate sits one level beneath the repository root, which every
    // fixture, binary, and scorecard path is relative to
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("CARGO_MANIFEST_DIR: no parent directory")?;
    let Args {
        filter,
        facts_only,
        rebuild,
        oracle,
        target,
    } = Args::parse(env::args().skip(1))?;
    let cases = cases(root, &filter)?;
    if cases.is_empty() {
        return Err(format!("no case under `{CASES}` matches").into());
    }
    let eval_dir = root.join("target/eval");

    // the oracle alone needs no binary, no component, and no model
    if let Some(card) = oracle {
        for case in &cases {
            let Some(reference) = Reference::locate(root, case.expected.reference.as_deref())
            else {
                eprintln!("eval: case `{}` names no reference; nothing driven", case.name);
                continue;
            };
            let project = eval_dir.join(&card).join(&case.name);
            let (file, text) = self::oracle(&project, &reference)?;
            print!("{text}");
            eprintln!("eval: `{}` oracle at {}", case.name, file.display());
        }
        return Ok(());
    }

    let adapters: BTreeSet<&str> = cases.iter().flat_map(|case| case.expected.adapters()).collect();
    let mut settings = Settings::from_env(root, &adapters, target)?;
    settings.rebuild = rebuild;

    if let Some(card) = settings.rebuild.clone() {
        let started = now();
        let card_name = started.replace(':', "-");
        let mut reports = Vec::with_capacity(cases.len());
        for case in &cases {
            eprintln!("eval: case `{}` rebuilt from `{card}`", case.name);
            let run = rebuilt(&eval_dir, &card, &card_name, case, &settings)?;
            eprintln!("eval: `{}` {}", case.name, run.summary());
            let attempt = Attempt {
                grade: grade(&case.expected, &run),
                run,
            };
            reports.push(Report {
                case,
                runs: vec![vec![attempt]],
            });
        }
        let card = scorecard(&settings, &reports, &started, &card_name);
        let out = eval_dir.join(format!("{card_name}.md"));
        fs::write(&out, &card)?;
        print!("{card}");
        eprintln!("eval: scorecard at {}", out.display());
        return Ok(());
    }
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
    target: Option<Target>,
    // the width cap a `--build` run passes as `--jobs`; none leaves the
    // engine's default
    jobs: Option<NonZeroUsize>,
    runs: usize,
    ladder: Vec<Rung>,
    // the one rung every `emery build` runs at, since a slice's turn writes
    // a tree and repairs its findings where a specify turn answers claims
    build_rung: Rung,
    model: String,
    // the caller's `RUST_LOG`, when set, over the per-case default
    rust_log: Option<String>,
    // the caller's `CURSOR_MAX_TOOL_CALLS`, when set; the runner inherits it,
    // the card names it
    max_tool_calls: Option<String>,
    // the card whose committed plans this invocation builds again
    rebuild: Option<String>,
}

impl Settings {
    fn from_env(
        root: &Path, adapters: &BTreeSet<&str>, target: Option<Target>,
    ) -> Result<Self, String> {
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
        let mut wasm = BTreeMap::new();
        for component in adapters.iter().copied().chain(target.map(Target::name)) {
            // the crate is `emery-<kind>`; cargo spells the hyphen as an underscore
            let artifact = format!("emery_{component}");
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
        let ladder: Vec<Rung> = ladder
            .split(',')
            .map(|rung| Rung::parse(rung, "EVAL_LADDER"))
            .collect::<Result<_, _>>()?;
        let build_rung = Rung::parse(
            &env::var("EVAL_BUILD_RUNG").unwrap_or_else(|_| BUILD_RUNG.to_owned()),
            "EVAL_BUILD_RUNG",
        )?;

        Ok(Self {
            root: root.to_path_buf(),
            emery,
            store,
            wasm,
            target,
            jobs,
            runs,
            ladder,
            build_rung,
            model: env::var("CURSOR_MODEL").unwrap_or_else(|_| "auto".to_owned()),
            rust_log: env::var("RUST_LOG").ok(),
            max_tool_calls: env::var("CURSOR_MAX_TOOL_CALLS").ok(),
            rebuild: None,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rung {
    timeout: u64,
    inactivity: u64,
}

impl Rung {
    // `text` read as a rung, `variable` the setting it came from.
    fn parse(text: &str, variable: &str) -> Result<Self, String> {
        let parsed = text.trim().split_once('/').and_then(|(timeout, inactivity)| {
            Some(Self {
                timeout: timeout.parse().ok()?,
                inactivity: inactivity.parse().ok()?,
            })
        });
        parsed.ok_or_else(|| {
            format!("{variable}: `{text}`: a rung is `<timeout>/<inactivity>` in seconds")
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
    // the spans no accepted requirement claim should anchor within: the
    // readiness and lifecycle code whose behaviour is the design's, so a
    // run that still states it is read apart from one that merely finds less
    #[serde(default, rename = "absent")]
    absent: Vec<Item>,
    // the requirements the engine reconciles across the sources, exactly
    #[serde(default, rename = "behaviour")]
    behaviours: Vec<Behaviour>,
    // a hand-written omnia guest of the same behaviour, relative to the
    // repository root, which a build through `omnia` is compared with and
    // whose recorded fixtures the built guest is driven through
    reference: Option<String>,
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

    fn trees(&self) -> impl Iterator<Item = &str> {
        self.sources
            .iter()
            .filter_map(|source| source.fixture.as_deref())
            .chain(self.reference.as_deref())
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
        if let Some(reference) = &self.reference
            && (reference.is_empty() || Path::new(reference).is_absolute())
        {
            return Err(format!(
                "names `reference = \"{reference}\"`, which is not a path relative to the \
                 repository root"
            ));
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
            ("absent", &self.absent),
        ] {
            for item in items {
                self.source(item.source.as_deref())
                    .map_err(|why| format!("{kind} `{}` {why}", item.anchor))?;
            }
        }
        for item in &self.absent {
            if item.stem.is_some() {
                return Err(format!(
                    "absent `{}` names a stem; a span no requirement anchors in has none",
                    item.anchor
                ));
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
        for tree in expected.trees() {
            let path = root.join(tree);
            if path.is_dir() {
                check_tree(&path, tree).map_err(|error| format!("{name}: {error}"))?;
            }
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
    if let Some(target) = settings.target {
        config.push_str(&target_table(settings, target)?);
    }
    fs::write(project.join("emery.toml"), config)?;

    Ok(project)
}

// The `[target] name` every staged project builds under, so the repository
// the engine lays sits at `TARGET_REPO` whichever target the run names.
const TARGET_NAME: &str = "target";
const TARGET_REPO: &str = ".emery/vcs/repos/target";

// The `[target]` table of a staged project: the target's component staged
// in the store, under the one name. Both targets lay a whole tree, so the
// engine lays the repository beneath the project itself, one root commit
// holding its ignore rule, and no URL is named.
fn target_table(settings: &Settings, target: Target) -> io::Result<String> {
    let reference = settings.stage_component(target.name())?;
    Ok(format!("[target]\nname = \"{TARGET_NAME}\"\nadapter = \"{reference}\"\n"))
}

fn toml_escaped(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
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
    "target",
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
        if skipped_entry(&name) {
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

fn skipped_entry(name: &std::ffi::OsStr) -> bool {
    UNCOPIED.iter().any(|skip| name == *skip)
        || name.to_str().is_some_and(|name| name.ends_with(".egg-info"))
}

// Holds every staged file of a fixture or reference tree at `dir` to the
// credential gate before any case is accepted or run.
fn check_tree(dir: &Path, label: &str) -> Result<(), String> {
    let mut findings = Vec::new();
    check_tree_files(dir, dir, label, &mut findings)
        .map_err(|error| format!("{label}: {error}"))?;
    if findings.is_empty() {
        Ok(())
    } else {
        findings.sort();
        Err(findings.join("; "))
    }
}

fn check_tree_files(
    root: &Path, dir: &Path, label: &str, findings: &mut Vec<String>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if skipped_entry(&entry.file_name()) {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            check_tree_files(root, &path, label, findings)?;
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else { continue };
        let parts: Vec<String> = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();
        let rel = parts.join("/");
        let what = match named_secret(&rel) {
            Some(what) => Some(what.to_owned()),
            None => carried_secret(&rel, &fs::read(&path)?),
        };
        if let Some(what) = what {
            findings.push(format!("{label}/{rel}: {what}"));
        }
    }
    Ok(())
}

// Files that carry a credential by name, matched on the final path segment;
// `.npmrc` is read for `_authToken=` instead since a fixture may set
// `legacy-peer-deps` there, and `.env.example` / `.sample` / `.template`
// document variable names alone.
const SECRET_CARRIERS: [&str; 9] = [
    "id_rsa",
    "id_dsa",
    "id_ecdsa",
    "id_ed25519",
    ".netrc",
    ".git-credentials",
    ".pgpass",
    ".pypirc",
    ".env",
];

fn named_secret(path: &str) -> Option<&'static str> {
    let mut segments = path.rsplit('/');
    let name = segments.next()?;
    let env_file = name.starts_with(".env.")
        && !matches!(name, ".env.example" | ".env.sample" | ".env.template");
    if SECRET_CARRIERS.contains(&name)
        || env_file
        || (name == "credentials" && segments.next() == Some(".aws"))
    {
        return Some("a file that carries a credential by name");
    }
    None
}

#[derive(Clone, Copy)]
enum SecretRun {
    Upper,
    Alnum,
    Token,
    Base64,
}

impl SecretRun {
    const fn admits(self, byte: u8) -> bool {
        match self {
            Self::Upper => byte.is_ascii_uppercase() || byte.is_ascii_digit(),
            Self::Alnum => byte.is_ascii_alphanumeric(),
            Self::Token => byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-',
            Self::Base64 => {
                byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/' || byte == b'='
            }
        }
    }
}

struct SecretShape {
    prefix: &'static [u8],
    run: SecretRun,
    least: usize,
    what: &'static str,
}

const SECRET_SHAPES: [SecretShape; 23] = [
    SecretShape {
        prefix: b"AKIA",
        run: SecretRun::Upper,
        least: 16,
        what: "an AWS access key id",
    },
    SecretShape {
        prefix: b"ASIA",
        run: SecretRun::Upper,
        least: 16,
        what: "an AWS access key id",
    },
    SecretShape {
        prefix: b"ghp_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"gho_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"ghu_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"ghs_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"ghr_",
        run: SecretRun::Alnum,
        least: 36,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"github_pat_",
        run: SecretRun::Token,
        least: 22,
        what: "a GitHub token",
    },
    SecretShape {
        prefix: b"npm_",
        run: SecretRun::Alnum,
        least: 36,
        what: "an npm token",
    },
    SecretShape {
        prefix: b"_authToken=",
        run: SecretRun::Token,
        least: 20,
        what: "an npm `_authToken` line",
    },
    SecretShape {
        prefix: b"xoxa-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxb-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxp-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxr-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"xoxs-",
        run: SecretRun::Token,
        least: 10,
        what: "a Slack token",
    },
    SecretShape {
        prefix: b"sk_live_",
        run: SecretRun::Alnum,
        least: 24,
        what: "a Stripe live key",
    },
    SecretShape {
        prefix: b"rk_live_",
        run: SecretRun::Alnum,
        least: 24,
        what: "a Stripe live key",
    },
    SecretShape {
        prefix: b"AIza",
        run: SecretRun::Token,
        least: 35,
        what: "a Google API key",
    },
    SecretShape {
        prefix: b"glpat-",
        run: SecretRun::Token,
        least: 20,
        what: "a GitLab token",
    },
    SecretShape {
        prefix: b"NRAK-",
        run: SecretRun::Upper,
        least: 27,
        what: "a New Relic key",
    },
    SecretShape {
        prefix: b"NRII-",
        run: SecretRun::Token,
        least: 27,
        what: "a New Relic key",
    },
    SecretShape {
        prefix: b"AccountKey=",
        run: SecretRun::Base64,
        least: 20,
        what: "an Azure Storage account key",
    },
    SecretShape {
        prefix: b"SharedAccessKey=",
        run: SecretRun::Base64,
        least: 20,
        what: "an Azure shared access key",
    },
];

fn carried_secret(path: &str, bytes: &[u8]) -> Option<String> {
    if pem_private_key(bytes) {
        return Some("a PEM private key block".to_owned());
    }
    if let Some(shape) = SECRET_SHAPES.iter().find(|shape| shape.matches(bytes)) {
        return Some(shape.what.to_owned());
    }
    if azure_storage_key(bytes) {
        return Some("an Azure Storage account key".to_owned());
    }
    parameter_secret(path, bytes)
}

fn pem_private_key(bytes: &[u8]) -> bool {
    const HEAD: &[u8] = b"-----BEGIN ";
    const DASHES: &[u8] = b"-----";
    const PRIVATE: &[u8] = b"PRIVATE KEY";

    byte_occurrences(bytes, HEAD).any(|start| {
        let rest = &bytes[start + HEAD.len()..];
        byte_occurrences(rest, DASHES).next().is_some_and(|end| {
            let label = &rest[..end];
            label.ends_with(PRIVATE) && !label.contains(&b'\n')
        })
    })
}

impl SecretShape {
    fn matches(&self, bytes: &[u8]) -> bool {
        byte_occurrences(bytes, self.prefix).any(|start| {
            let rest = &bytes[start + self.prefix.len()..];
            let run = rest.iter().take_while(|byte| self.run.admits(**byte)).count();
            run >= self.least && rest.get(run) != Some(&b'/')
        })
    }
}

// A 512-bit Azure Storage account key literal: 86 base64 characters and
// `==` bounded by a quote, `=`, or whitespace, never the `sha512-` prefix
// of a lockfile's integrity hash.
fn azure_storage_key(bytes: &[u8]) -> bool {
    byte_occurrences(bytes, b"==").any(|end| {
        if end < 86 {
            return false;
        }
        let start = end - 86;
        let bounded_before = start == 0
            || matches!(bytes[start - 1], b'"' | b'\'' | b'=' | b' ' | b'\t' | b'\r' | b'\n');
        let bounded_after = end + 2 == bytes.len()
            || matches!(bytes[end + 2], b'"' | b'\'' | b',' | b' ' | b'\t' | b'\r' | b'\n');
        let body = &bytes[start..end];
        bounded_before
            && bounded_after
            && body
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'+' || *byte == b'/')
            && body.iter().any(|byte| *byte == b'+' || *byte == b'/')
    })
}

// A cloud parameter file (`parameters.json` or `*.parameters.json`) whose
// key, secret, token, password, or connection-string entry holds a
// non-empty literal rather than an empty placeholder or expression.
fn parameter_secret(path: &str, bytes: &[u8]) -> Option<String> {
    let name = path.rsplit('/').next()?;
    if name != "parameters.json" && !name.ends_with(".parameters.json") {
        return None;
    }
    let root: Value = serde_json::from_slice(bytes).ok()?;
    let object = root.get("parameters").and_then(Value::as_object).or_else(|| root.as_object())?;
    for (key, entry) in object {
        if secret_parameter_key(key) && has_credential_literal(entry) {
            return Some(format!("parameter `{key}` carries a credential literal"));
        }
    }
    None
}

fn secret_parameter_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    if lower.contains("secret_name") || lower.contains("key_name") {
        return false;
    }
    lower.ends_with("connection_string")
        || lower.ends_with("connectionstring")
        || lower
            .rsplit(['_', '-'])
            .next()
            .is_some_and(|last| matches!(last, "key" | "secret" | "token" | "password" | "passwd"))
}

fn has_credential_literal(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with('[')
                && !trimmed.starts_with("${")
                && !trimmed.starts_with("@Microsoft.KeyVault(")
        }
        Value::Object(map) => map.values().any(has_credential_literal),
        _ => false,
    }
}

fn byte_occurrences<'a>(bytes: &'a [u8], needle: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    (0..bytes.len()).filter(move |start| bytes[*start..].starts_with(needle))
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

    // The design's data type names: each `data` `type` block's key, the name
    // alone where the key carries an anchor beside it. A `mechanism` type —
    // a client, a retriever, a bootstrap, a settings carrier — is left out,
    // so a requirement naming one counts as stated through the mechanism.
    fn design_types(&self) -> BTreeSet<&str> {
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

    fn mechanism(&self) -> Vec<Stated> {
        mechanism(self.requirements(), &self.design_types())
    }

    // The lexicon the run's specification is held to, as the engine holds
    // its spec draft: the fixed names, and the packages the accepted `call`
    // claims spell before the `:` of their `callee`.
    fn lexicon(&self) -> Lexicon {
        Lexicon::from_packages(
            self.claims(ClaimKind::Call)
                .filter_map(|claim| claim.extras.get("callee")?.as_str())
                .filter_map(|callee| callee.split_once(':').map(|(package, _)| package)),
        )
    }

    // Every place the specification's own words name a technology, in the
    // order the gates read them: each accepted `requirement` and
    // `criterion` claim's id and body, then the committed specification's
    // preamble paragraphs and scenario lines. Each is one the SDK's or the
    // engine's gate should have returned for correction, so the list is
    // expected empty.
    fn leaks(&self) -> Vec<String> {
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
    fn citing(&self, source: &str) -> usize {
        self.requirements()
            .iter()
            .filter(|requirement| citations(requirement).any(|(of, _)| of == source))
            .count()
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
    shell: ShellUse,
    writes: Writes,
}

// The shell calls one completion made: how many, their summed wall time
// from the call to its completion, and the bytes their results carried back.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct ShellUse {
    calls: usize,
    time: Duration,
    bytes: u64,
}

// What one turn's `write_files` and `edit_files` calls did, read from the
// SDK's `responded` lines: the `write_files` calls answered, and among the
// files they wrote those the turn had written before, those written empty,
// and those rewritten at under half their last size; the `edit_files` calls
// answered; and the calls of either tool refused.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Writes {
    calls: usize,
    rewrites: usize,
    empty: usize,
    shrinking: usize,
    edits: usize,
    refused: usize,
}

impl Writes {
    const fn any(&self) -> bool {
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
    fn answered(&self) -> bool {
        matches!(self.outcome.as_str(), "ok" | "corrected")
    }
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

    // the plan is built once the specify landed, as the ladder reads it: a
    // starved run climbs a rung instead, and a failed one has no plan
    let revision = envelope.as_ref().and_then(|out| out["revision"].as_str());
    if let Some(revision) = revision
        && settings.target.is_some()
        && run.passed()
    {
        eprintln!("eval: `{}` run {n} building at {}", case.name, settings.build_rung);
        let built = build(project, settings, case, tag, revision, run.spec.as_ref())?;
        run.build = Some(built);
    }

    Ok(run)
}

const SPECIFY: &[&str] = &["specify", "--config", "emery.toml"];

// One `emery build` of the revision the run committed, at the build rung,
// into the repository the engine lays for the case, then the host's own
// pass over the tree it left; the build's log and envelope are kept as
// `<tag>.build.stderr` / `<tag>.build.stdout`, the pass as
// `<tag>.fidelity.md`.
#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
fn build(
    project: &Path, settings: &Settings, case: &Case, tag: &str, revision: &str,
    spec: Option<&Value>,
) -> io::Result<Build> {
    let rust_log = settings.rust_log(case);
    let reference = Reference::locate(&settings.root, case.expected.reference.as_deref());
    let jobs = settings.jobs.map(|jobs| jobs.to_string());
    let mut args = vec!["build", "--config", "emery.toml"];
    if let Some(jobs) = &jobs {
        args.extend(["--jobs", jobs]);
    }
    let started = Instant::now();
    let output = emery(project, settings, &rust_log, settings.build_rung, &args, Turns::Live)?;
    let wall = started.elapsed();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    fs::write(project.join(format!("{tag}.build.stderr")), stderr.as_bytes())?;
    fs::write(project.join(format!("{tag}.build.stdout")), stdout.as_bytes())?;
    let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    tail.reverse();
    let target =
        settings.target.ok_or_else(|| io::Error::other("a build runs through a target"))?;

    let mut build = Build {
        target,
        wall,
        exit: output.status.code(),
        envelope: serde_json::from_str(&stdout).ok(),
        failure: failure(&stderr),
        completions: completions(&stderr),
        history: history(project, revision),
        tail: tail.join("\n"),
        fidelity: None,
    };
    // the host's pass is read beside the build, never in its place: a
    // toolchain the host lacks leaves the cells empty and the card whole
    if let Some(tree) = Tree::left(project, revision, &build) {
        eprintln!("eval: `{tag}` host checks over {tree}");
        match Fidelity::read(project, tag, &tree, &build, spec, target, reference.as_ref()) {
            Ok(fidelity) => {
                fs::write(project.join(&fidelity.file), fidelity.to_string())?;
                build.fidelity = Some(fidelity);
            }
            Err(error) => eprintln!("eval: `{tag}` host checks not read: {error}"),
        }
    }

    Ok(build)
}

// Builds the plan `run-1` of `case` committed under `card` again. The staged
// project is copied whole under the new card, less the engine's clones and
// working copies and the repository the earlier build laid, and its
// `emery.toml` gains a `[target]` table for this invocation's target, so the
// engine lays a repository with no label afresh and builds every slice over
// its root; the run's specify is read back from the
// files the earlier run kept, exactly as `run` read them, so the card's
// specify columns repeat the source card's.
fn rebuilt(
    eval_dir: &Path, card: &str, card_name: &str, case: &Case, settings: &Settings,
) -> io::Result<Run> {
    const TAG: &str = "run-1";
    let source = eval_dir.join(card).join(&case.name);
    let project = eval_dir.join(card_name).join(&case.name);
    if !source.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no staged project at `{}`", source.display()),
        ));
    }
    let stdout = fs::read_to_string(source.join(format!("{TAG}.stdout")))?;
    let stderr = fs::read_to_string(source.join(format!("{TAG}.stderr")))?;
    let envelope: Option<Value> = serde_json::from_str(&stdout).ok();
    let revision = envelope
        .as_ref()
        .and_then(|out| out["revision"].as_str())
        .ok_or_else(|| {
            io::Error::other(format!("`{TAG}` of `{}` committed no revision", case.name))
        })?
        .to_owned();

    // the project copied, the engine's repositories and the earlier build's files left behind
    if project.exists() {
        fs::remove_dir_all(&project)?;
    }
    fs::create_dir_all(&project)?;
    for entry in fs::read_dir(&source)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.contains(".build") || name_str.contains(".fidelity") {
            continue;
        }
        let target = project.join(&name);
        if entry.file_type()?.is_dir() {
            if name == ".emery" {
                copy_tree_whole(&entry.path().join("storage"), &target.join("storage"))?;
            } else {
                copy_tree_whole(&entry.path(), &target)?;
            }
        } else {
            fs::copy(entry.path(), target)?;
        }
    }

    // the config's `[target]` table rewritten for this invocation's target,
    // its component staged
    let config = fs::read_to_string(source.join("emery.toml"))?;
    let (sources, _) = config.split_once("[target]").ok_or_else(|| {
        io::Error::other(format!("`{TAG}` of `{}` was staged with no `[target]`", case.name))
    })?;
    let target =
        settings.target.ok_or_else(|| io::Error::other("a rebuild builds through a target"))?;
    let table = target_table(settings, target)?;
    fs::write(project.join("emery.toml"), format!("{sources}{table}"))?;

    // the run as the earlier specify left it, then this invocation's build
    let rung =
        settings.ladder.first().copied().ok_or_else(|| io::Error::other("EVAL_LADDER: no rung"))?;
    let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
    tail.reverse();
    let mut run = Run {
        n: 1,
        rung,
        wall: Duration::ZERO,
        exit: Some(0),
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
    for (artifact, slot) in
        [("spec", &mut run.spec), ("design", &mut run.design), ("plan", &mut run.plan)]
    {
        let text = fs::read_to_string(source.join(format!("{TAG}.{artifact}.json")))?;
        *slot = Some(serde_json::from_str(&text)?);
    }
    eprintln!("eval: `{}` rebuilding revision `{revision}` at {}", case.name, settings.build_rung);
    run.build = Some(build(&project, settings, case, TAG, &revision, run.spec.as_ref())?);

    Ok(run)
}

// The reference comparison alone over the tree `run-1` of the case staged at
// `project` left: the tip exported once to `run-1.oracle/` beside the run's
// files and kept, so the next invocation reuses its build directory and an
// operator may patch the export by hand and read the oracle again, and the
// comparison written to `run-1.oracle.md` with the harness's own output in
// `run-1.oracle.log`.
fn oracle(project: &Path, reference: &Reference) -> io::Result<(PathBuf, String)> {
    const TAG: &str = "run-1";
    if !project.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no staged project at `{}`", project.display()),
        ));
    }
    let stdout = fs::read_to_string(project.join(format!("{TAG}.stdout")))?;
    let envelope: Option<Value> = serde_json::from_str(&stdout).ok();
    let revision = envelope
        .as_ref()
        .and_then(|out| out["revision"].as_str())
        .ok_or_else(|| io::Error::other(format!("`{TAG}` committed no revision")))?;
    let build = Build::recorded(project, TAG, Target::Omnia, revision)?;
    let tree = Tree::left(project, revision, &build).ok_or_else(|| {
        io::Error::other(format!(
            "`{TAG}` left no tree: its build set no label and no integration copy of its own stands"
        ))
    })?;

    // the export kept, so its build directory is
    let export = project.join(format!("{TAG}.oracle"));
    if export.is_dir() {
        eprintln!(
            "eval: `{TAG}` oracle over the export kept at {}; remove it to export {tree} again",
            export.display()
        );
    } else {
        eprintln!("eval: `{TAG}` exporting {tree} to {}", export.display());
        export_tip(&tree, &export)?;
    }

    let comparison = reference.compare(&export)?;
    let text = comparison.to_string();
    fs::write(project.join(format!("{TAG}.oracle.log")), comparison.log())?;
    let file = project.join(format!("{TAG}.oracle.md"));
    fs::write(&file, &text)?;
    Ok((file, text))
}

// The tip of `tree` exported whole to `export`, laid fresh, and every path
// the tip tracks.
#[expect(clippy::disallowed_types, reason = "the eval unpacks the export with the host's `tar`")]
fn export_tip(tree: &Tree, export: &Path) -> io::Result<Vec<String>> {
    let (repo, tip) = tree.at();
    if export.exists() {
        fs::remove_dir_all(export)?;
    }
    fs::create_dir_all(export)?;
    let archive = PathBuf::from(format!("{}.tar", export.display()));
    git(repo, &["archive", "--format=tar", "-o", &archive.display().to_string(), tip])?;
    let untar = std::process::Command::new("tar")
        .arg("-xf")
        .arg(&archive)
        .arg("-C")
        .arg(export)
        .output()?;
    fs::remove_file(&archive)?;
    if !untar.status.success() {
        return Err(io::Error::other(format!(
            "tar over the exported tree: {}",
            String::from_utf8_lossy(&untar.stderr).trim()
        )));
    }
    Ok(git(repo, &["ls-tree", "-r", "--name-only", tip])?.lines().map(str::to_owned).collect())
}

// A copy of a directory tree skipping nothing, for a staged project's
// sources and `.emery/storage`.
fn copy_tree_whole(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree_whole(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

// What one `emery build` did, read from its envelope, its log, and the
// repository it built into.
struct Build {
    target: Target,
    wall: Duration,
    exit: Option<i32>,
    // the `build` envelope on success
    envelope: Option<Value>,
    // the failure envelope the binary wrote among its log lines
    failure: Option<Failure>,
    completions: Vec<Completion>,
    // the first-parent chain the build left, newest first: the label the
    // run set, or the integration copy a failed run left behind
    history: Vec<Landed>,
    tail: String,
    // the host's own pass over the tree the chain stands at; none where the
    // build left no tree
    fidelity: Option<Fidelity>,
}

// The failure envelope: its code and message.
struct Failure {
    code: String,
    message: String,
}

// One commit on the build's first-parent chain: a slice's merge, read
// from its trailers, a `Wave <k> verified` seal, or the root the engine laid.
#[derive(Debug, PartialEq, Eq)]
struct Landed {
    commit: String,
    subject: String,
    // the `Revision:` trailer a merge and a seal carry; the root none
    revision: Option<String>,
    slice: Option<String>,
    wave: Option<usize>,
    requirements: Vec<String>,
    covered: Vec<String>,
    files: Vec<String>,
}

impl Landed {
    // The wave a `Wave <k> verified` seal closed; a merge and the root none.
    fn sealed(&self) -> Option<usize> {
        self.subject.strip_prefix("Wave ")?.strip_suffix(" verified")?.parse().ok()
    }
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
    resolved: Vec<String>,
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
    // The wave's line tail: its build durations, its verify's duration with
    // the shell part, and what the verify sealed — `(lock only)` for the
    // lock rewrite alone, else `repaired` with the files.
    fn spent(&self, sealed: Option<&Vec<String>>) -> String {
        let builds: Vec<String> =
            self.builds.iter().map(|b| format!("{}s", b.duration.as_secs())).collect();
        let verify: Duration = self.verifies.iter().map(|v| v.duration).sum();
        let shell = self.verifies.iter().fold(ShellUse::default(), |mut sum, v| {
            sum.calls += v.shell.calls;
            sum.time += v.shell.time;
            sum.bytes += v.shell.bytes;
            sum
        });
        let shelled = if shell.calls == 0 {
            String::new()
        } else {
            format!(
                " (shell {}s over {} calls, {} KB back)",
                shell.time.as_secs(),
                shell.calls,
                shell.bytes / 1024
            )
        };
        let sealed = match sealed {
            Some(files) if files.is_empty() => " (lock only)".to_owned(),
            Some(files) => format!(", repaired {}", files.join(", ")),
            None => String::new(),
        };
        format!(
            "; builds {} then verify {}s{shelled}{sealed}",
            builds.join(" / "),
            verify.as_secs()
        )
    }

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
    // A build read back from the files `build` kept beside its run, with the
    // chain it left where it stands: the exit is the envelope's success or
    // the failure's class, and the wall time, unkept, is zero.
    fn recorded(project: &Path, tag: &str, target: Target, revision: &str) -> io::Result<Self> {
        let stdout = fs::read_to_string(project.join(format!("{tag}.build.stdout")))?;
        let stderr = fs::read_to_string(project.join(format!("{tag}.build.stderr")))?;
        let envelope: Option<Value> = serde_json::from_str(&stdout).ok();
        let failure = failure(&stderr);
        let exit = match (&envelope, &failure) {
            (Some(_), _) => Some(0),
            (None, Some(failure)) => Some(match failure.code.as_str() {
                "not_found" => 2,
                "server_error" => 3,
                "bad_gateway" => 4,
                _ => 1,
            }),
            (None, None) => None,
        };
        let mut tail: Vec<&str> = stderr.lines().rev().take(5).collect();
        tail.reverse();
        Ok(Self {
            target,
            wall: Duration::ZERO,
            exit,
            envelope,
            failure,
            completions: completions(&stderr),
            history: history(project, revision),
            tail: tail.join("\n"),
            fidelity: None,
        })
    }

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
                        resolved: strings(&slice["resolved"]),
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
                    resolved: Vec::new(),
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
    // every wave whose verify answered, less the one whose verdict failed
    // the run — a `verify-failed` is an answered verify, and the last.
    fn verified(&self) -> usize {
        if let Some(verified) = self.envelope.as_ref().and_then(|out| out["verified"].as_array()) {
            return verified.len();
        }
        let answered = self
            .wave_turns()
            .iter()
            .filter(|wave| wave.verifies.last().is_some_and(|verify| verify.answered()))
            .count();
        answered.saturating_sub(usize::from(self.code() == Some("verify-failed")))
    }

    // The waves whose verify changed the tree before it passed, as the
    // envelope lists them.
    fn repaired(&self) -> Vec<usize> {
        self.envelope
            .as_ref()
            .and_then(|out| out["repaired"].as_array())
            .map(|waves| {
                waves.iter().filter_map(Value::as_u64).filter_map(|k| k.try_into().ok()).collect()
            })
            .unwrap_or_default()
    }

    // What each wave's verify sealed besides the lockfile, by wave: the
    // envelope's `sealed` where the engine reports it, else the `Wave <k>
    // verified` commits on the chain. An empty list is a seal of the lock
    // rewrite alone, and a wave with no seal changed nothing.
    fn repairs(&self) -> BTreeMap<usize, Vec<String>> {
        let besides = |paths: &[String]| -> Vec<String> {
            paths.iter().filter(|path| *path != self.target.lockfile()).cloned().collect()
        };
        if let Some(sealed) = self.envelope.as_ref().and_then(|out| out["sealed"].as_array()) {
            return sealed
                .iter()
                .filter_map(|seal| {
                    let wave = seal["wave"].as_u64()?.try_into().ok()?;
                    Some((wave, besides(&strings(&seal["paths"]))))
                })
                .collect();
        }
        self.history
            .iter()
            .filter_map(|landed| Some((landed.sealed()?, besides(&landed.files))))
            .collect()
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
// failed run leaves for inspection, else the label a landed run set in the
// repository the engine laid. The project is staged once for every run of a
// case, so the copy standing may be an earlier run's, left by a build that
// failed after cutting it, when this run's failed before it cut one: the
// copy is this run's only where its chain holds a commit under this run's
// revision. A chain nowhere, a run that ended before any was cut, is empty.
fn history(project: &Path, revision: &str) -> Vec<Landed> {
    let integration = project.join(".emery/vcs/integration");
    if integration.is_dir() {
        let left = log(&integration, "HEAD");
        if left.iter().any(|landed| landed.revision.as_deref() == Some(revision)) {
            return left;
        }
    }
    let repository = project.join(TARGET_REPO);
    if !repository.is_dir() {
        return Vec::new();
    }
    log(&repository, &format!("refs/heads/emery/{revision}"))
}

// The first-parent chain from `tip`, newest first; empty where `tip` is
// nothing the repository holds.
fn log(at: &Path, tip: &str) -> Vec<Landed> {
    let format = "--format=%x1e%H%x1f%s%x1f%(trailers:key=Revision,valueonly)%x1f\
                  %(trailers:key=Slice,valueonly)%x1f%(trailers:key=Wave,valueonly)%x1f\
                  %(trailers:key=Requirements,valueonly)%x1f%(trailers:key=Covered,valueonly)%x1f";
    let log = git(at, &["log", "--first-parent", "--name-only", format, tip]);
    log.map(|text| chain(&text)).unwrap_or_default()
}

// The chain as `log` asks git to spell it: one record per commit led by
// `\x1e`, its fields split by `\x1f`, the files it changed against its
// first parent after the last.
fn chain(text: &str) -> Vec<Landed> {
    text.split('\x1e')
        .filter(|record| !record.trim().is_empty())
        .filter_map(|record| {
            let fields: Vec<&str> = record.split('\x1f').collect();
            let [commit, subject, revision, slice, wave, requirements, covered, files] = fields[..]
            else {
                return None;
            };
            let ids = |text: &str| {
                text.trim()
                    .split(", ")
                    .filter(|id| !id.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            };
            let revision = revision.trim();
            let slice = slice.trim();
            Some(Landed {
                commit: commit.trim().to_owned(),
                subject: subject.trim().to_owned(),
                revision: (!revision.is_empty()).then(|| revision.to_owned()),
                slice: (!slice.is_empty()).then(|| slice.to_owned()),
                wave: wave.trim().parse().ok(),
                requirements: ids(requirements),
                covered: ids(covered),
                files: files.lines().filter(|line| !line.is_empty()).map(str::to_owned).collect(),
            })
        })
        .collect()
}

// --- the host's pass over the tree a build left ---

// Where a build left its tree: the label in the repository the engine laid
// once the run landed, else the integration copy's head a failed run
// leaves, read only where its chain is this run's.
enum Tree {
    Label { repository: PathBuf, label: String },
    Integration(PathBuf),
}

impl Tree {
    fn left(project: &Path, revision: &str, build: &Build) -> Option<Self> {
        if build.landed() {
            return Some(Self::Label {
                repository: project.join(TARGET_REPO),
                label: format!("emery/{revision}"),
            });
        }
        let integration = project.join(".emery/vcs/integration");
        (!build.history.is_empty() && integration.is_dir())
            .then_some(Self::Integration(integration))
    }

    // The repository to read and the tip within it.
    fn at(&self) -> (&Path, &str) {
        match self {
            Self::Label { repository, label } => (repository, label),
            Self::Integration(copy) => (copy, "HEAD"),
        }
    }
}

impl Display for Tree {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label { label, .. } => write!(f, "the label `{label}`"),
            Self::Integration(_) => f.write_str("the integration copy's head"),
        }
    }
}

// A target adapter a `--build` run builds through, and what the host reads
// of the tree it leaves: the files its layout admits, the checks its
// toolchain runs, and how its tests are counted and named.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Target {
    #[default]
    Node,
    Omnia,
}

impl Target {
    const ALL: [Self; 2] = [Self::Node, Self::Omnia];

    fn parse(name: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|target| target.name() == name).ok_or_else(|| {
            let names: Vec<&str> = Self::ALL.iter().map(|target| target.name()).collect();
            format!("--target: `{name}` is no target adapter; one of {}", names.join(", "))
        })
    }

    // The crate's name: the component's, and the `eval:` package's.
    const fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Omnia => "omnia",
        }
    }

    // The one file the checks may write into the tree without it counting.
    const fn lockfile(self) -> &'static str {
        match self {
            Self::Node => "package-lock.json",
            Self::Omnia => "Cargo.lock",
        }
    }

    // The directory the checks leave beside the tree, passed over when the
    // export is listed.
    const fn scratch(self) -> &'static str {
        match self {
            Self::Node => "node_modules",
            Self::Omnia => "target",
        }
    }

    // A configuration file the checks find by walking up from the tree,
    // laid empty at the export's root for the checks when the tree holds
    // none, so the export, which sits beneath this repository, is read
    // under the tree's own configuration as the worker under the fence
    // reads it, and never under this repository's.
    const fn config_shim(self) -> Option<&'static str> {
        match self {
            Self::Node => None,
            Self::Omnia => Some("clippy.toml"),
        }
    }

    // The check whose exit stands for the tests, and whose output counts them.
    const fn runner(self) -> &'static str {
        match self {
            Self::Node => "node --test",
            Self::Omnia => "cargo test",
        }
    }

    // The root files the layout admits: for `omnia`, the template the adapter
    // lays and the lockfile.
    const fn root_files(self) -> &'static [&'static str] {
        match self {
            Self::Node => &["package.json", "package-lock.json", "tsconfig.json", ".gitignore"],
            Self::Omnia => &[
                "Cargo.toml",
                "Cargo.lock",
                "src/lib.rs",
                "src/routes.rs",
                "src/messaging.rs",
                "tests/routes.rs",
                "tests/messaging.rs",
                "examples/runtime.rs",
                "crates/common/Cargo.toml",
                "crates/common/src/lib.rs",
                "crates/common/src/topic.rs",
                "crates/common/src/config.rs",
                "Makefile",
                "mise.toml",
                "rust-toolchain.toml",
                "rustfmt.toml",
                "taplo.toml",
                "clippy.toml",
                "deny.toml",
                ".gitignore",
                ".vscode/settings.json",
                "supply-chain/README.md",
                "supply-chain/config.toml",
                "supply-chain/audits.toml",
                ".github/dependabot.yml",
                ".github/workflows/audit.yaml",
                ".github/workflows/ci.yaml",
                ".github/workflows/patch.yaml",
                ".github/workflows/release.yaml",
                ".github/workflows/publish.yaml",
            ],
        }
    }

    // Whether a tracked path is one the layout admits: a root file, the entry
    // or the runner, a file under a stem's directory, or a stem's test file
    // or directory.
    fn within_layout(self, path: &str, stems: &BTreeSet<&str>) -> bool {
        if self.root_files().contains(&path) {
            return true;
        }
        match self {
            Self::Node => {
                if let Some(rest) = path.strip_prefix("src/") {
                    if rest == "index.ts" || rest == "main.ts" {
                        return true;
                    }
                    return rest.split_once('/').is_some_and(|(dir, _)| stems.contains(dir));
                }
                if let Some(rest) = path.strip_prefix("test/") {
                    return rest.strip_suffix(".test.ts").is_some_and(|name| stems.contains(name))
                        || rest.split_once('/').is_some_and(|(dir, _)| stems.contains(dir));
                }
                false
            }
            Self::Omnia => path
                .strip_prefix("crates/")
                .and_then(|rest| rest.split_once('/'))
                .is_some_and(|(dir, _)| stems.contains(dir)),
        }
    }

    // The layout's checks run in `export`, each unrun once an earlier one
    // fails to leave what it needs.
    fn checks(self, export: &Path) -> Vec<HostCheck> {
        match self {
            Self::Node => {
                let install = host_check(export, "npm install", &["--no-audit", "--no-fund"]);
                let installed = install.passed();
                let compile = if installed {
                    host_check(export, "npx tsc", &["--noEmit"])
                } else {
                    unrun("npx tsc", "the install failed")
                };
                let tests = if installed {
                    host_check(
                        export,
                        "node --test",
                        &[
                            "--test-force-exit",
                            "--test-timeout=5000",
                            "--test-reporter=tap",
                            "test/**/*.test.ts",
                        ],
                    )
                } else {
                    unrun("node --test", "the install failed")
                };
                vec![install, compile, tests]
            }
            Self::Omnia => {
                let lint = host_check(
                    export,
                    "cargo clippy",
                    &["--workspace", "--all-targets", "--", "-D", "warnings"],
                );
                let linted = lint.passed();
                let tests = if linted {
                    host_check(export, "cargo test", &["--workspace"])
                } else {
                    unrun("cargo test", "the lint failed")
                };
                let guest = if tests.passed() {
                    host_check(export, "cargo build", &["--target", "wasm32-wasip2"])
                } else if linted {
                    unrun("cargo build", "the tests failed")
                } else {
                    unrun("cargo build", "the lint failed")
                };
                vec![lint, tests, guest]
            }
        }
    }

    // The tests the runner's output counts as passed and failed.
    fn test_counts(self, runner: &HostCheck) -> (usize, usize) {
        match self {
            Self::Node => tap_counts(&runner.note),
            Self::Omnia => cargo_test_counts(&runner.output),
        }
    }

    // Every test name the tree's test files declare, spelled as a scenario
    // is: a `test(` or `it(` string, or a `#[test]` fn's name with its
    // underscores as spaces.
    fn test_names(self, export: &Path) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        match self {
            Self::Node => test_names(&export.join("test"), &mut names)?,
            Self::Omnia => {
                rust_test_names(&export.join("tests"), &mut names)?;
                let crates = export.join("crates");
                if crates.is_dir() {
                    for entry in fs::read_dir(crates)? {
                        rust_test_names(&entry?.path().join("tests"), &mut names)?;
                    }
                }
            }
        }
        Ok(names)
    }

    // Every module the layout's lists do not reach, sorted: a handler module
    // `crates/<stem>/src/<part>.rs` whose stem's entry names neither its
    // `routes()` nor its `topics`, a stem crate `crates/<stem>/` neither root
    // list merges and no Rust file outside it names as `<stem>::`, a
    // constant of the topic table no stem's module names, or a stem's or a
    // part's `index.ts` the list above it does not re-export. A module's own
    // tests pass without the list, so the checks cannot see it.
    fn unreached(self, export: &Path) -> io::Result<Vec<String>> {
        let mut unreached = Vec::new();
        match self {
            Self::Node => {
                let src = export.join("src");
                let entry = fs::read_to_string(src.join("index.ts")).unwrap_or_default();
                for stem in directories(&src)? {
                    let stem_dir = src.join(&stem);
                    if !stem_dir.join("index.ts").is_file() {
                        continue;
                    }
                    if !reexported(&entry, &stem) {
                        unreached.push(format!("src/{stem}/index.ts"));
                    }
                    let list = fs::read_to_string(stem_dir.join("index.ts"))?;
                    for part in directories(&stem_dir)? {
                        if stem_dir.join(&part).join("index.ts").is_file()
                            && !reexported(&list, &part)
                        {
                            unreached.push(format!("src/{stem}/{part}/index.ts"));
                        }
                    }
                }
            }
            Self::Omnia => {
                let routes =
                    code_lines(&fs::read_to_string(export.join(ROUTE_LIST)).unwrap_or_default());
                let topics =
                    code_lines(&fs::read_to_string(export.join(TOPIC_LIST)).unwrap_or_default());
                let mut paths = Vec::new();
                files_under(export, export, "target", &mut paths)?;
                let mut sources = Vec::new();
                for path in paths
                    .iter()
                    .filter(|path| Path::new(path).extension().is_some_and(|e| e == "rs"))
                {
                    sources
                        .push((path.as_str(), code_lines(&fs::read_to_string(export.join(path))?)));
                }
                let mut modules = Vec::new();
                let crates = export.join("crates");
                for stem in directories(&crates)? {
                    if stem == "common" {
                        continue;
                    }
                    let src = crates.join(&stem).join("src");
                    if !src.is_dir() {
                        continue;
                    }
                    let name = stem.replace('-', "_");
                    let entry =
                        code_lines(&fs::read_to_string(src.join("lib.rs")).unwrap_or_default());

                    // a stem neither list merges and nothing outside it names
                    let own = format!("crates/{stem}/");
                    let merged = spells_word(&routes, &format!("{name}::router"))
                        || spells_word(&topics, &format!("{name}::topics"));
                    if !merged
                        && crates.join(&stem).join("Cargo.toml").is_file()
                        && !sources
                            .iter()
                            .any(|(path, text)| !path.starts_with(&own) && names_crate(text, &name))
                    {
                        unreached.push(own);
                    }

                    // a handler module the entry's arrays do not name
                    for entry_path in fs::read_dir(&src)? {
                        let path = entry_path?.path();
                        let Some(part) = path.file_stem().and_then(|part| part.to_str()) else {
                            continue;
                        };
                        if path.extension().is_none_or(|ext| ext != "rs") {
                            continue;
                        }
                        let text = code_lines(&fs::read_to_string(&path)?);
                        if part != "lib"
                            && text.contains("Context<")
                            && !spells_word(&entry, &format!("{part}::routes"))
                            && !spells_word(&entry, &format!("{part}::topics"))
                        {
                            unreached.push(format!("crates/{stem}/src/{part}.rs"));
                        }
                        modules.push(text);
                    }
                }

                let topic = fs::read_to_string(export.join(TOPIC_TABLE)).unwrap_or_default();
                for (name, _) in constants(&topic) {
                    if !modules.iter().any(|text| spells_word(text, &name)) {
                        unreached.push(format!("{TOPIC_TABLE}#{name}"));
                    }
                }
            }
        }
        unreached.sort();
        Ok(unreached)
    }

    // Every route the tree registers, in tree order: for `omnia`, each
    // `.route(<path>, <method>(<handler>))` of a part's `routes()`,
    // attributed to the part's module `crates/<stem>/src/<part>.rs`, the
    // path read from the constant the module declares or a literal, and the
    // method each route fn, or `MethodFilter`, in its second argument
    // spells; a `.route(..)` over a topic is none. The TypeScript layout
    // registers routes on an app a slice is handed, which no one file
    // spells, so it lists none.
    fn routes(self, export: &Path) -> Vec<(String, String, String)> {
        let mut routes = Vec::new();
        if self == Self::Node {
            return routes;
        }
        let crates = export.join("crates");
        let Ok(mut stems) = directories(&crates) else {
            return routes;
        };
        stems.sort();
        for stem in stems {
            if stem == "common" {
                continue;
            }
            let src = crates.join(&stem).join("src");
            let Ok(entries) = fs::read_dir(&src) else {
                continue;
            };
            let mut parts: Vec<PathBuf> =
                entries.filter_map(Result::ok).map(|e| e.path()).collect();
            parts.sort();
            for path in parts {
                let Some(part) = path.file_stem().and_then(|part| part.to_str()) else {
                    continue;
                };
                if part == "lib" || path.extension().is_none_or(|ext| ext != "rs") {
                    continue;
                }
                let text = fs::read_to_string(&path).unwrap_or_default();
                let module = format!("crates/{stem}/src/{part}.rs");
                for statement in registered(&text, &text) {
                    if statement.topic {
                        continue;
                    }
                    for method in statement.methods {
                        routes.push((module.clone(), method, statement.path.clone()));
                    }
                }
            }
        }
        routes
    }
}

// The omnia layout's two root lists and the topic table of `common`.
const ROUTE_LIST: &str = "src/routes.rs";
const TOPIC_LIST: &str = "src/messaging.rs";
const TOPIC_TABLE: &str = "crates/common/src/topic.rs";

// Each `pub const <NAME>: &str = "<text>";` or `pub(crate) const ..` of a
// file, in order, with the text it spells.
fn constants(table: &str) -> Vec<(String, String)> {
    table
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let declaration = line
                .strip_prefix("pub const ")
                .or_else(|| line.strip_prefix("pub(crate) const "))?;
            let (name, rest) = declaration.split_once(':')?;
            let text = literal(rest.split_once('=')?.1.trim().strip_suffix(';')?.trim())?;
            Some((name.trim().to_owned(), text.to_owned()))
        })
        .collect()
}

fn code_lines(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| if line.trim_start().starts_with("//") { "\n" } else { line })
        .collect()
}

// Whether `text` reaches into the crate `name`: `name::` spelled at the
// head of a path, `use name::..` or `name::module::item`.
fn names_crate(text: &str, name: &str) -> bool {
    let path = format!("{name}::");
    let ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut from = 0;
    while let Some(at) = text[from..].find(&path) {
        let start = from + at;
        if text[..start].chars().next_back().is_none_or(|ch| !ident(ch) && ch != ':') {
            return true;
        }
        from = start + path.len();
    }
    false
}

fn spells_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut from = 0;
    while let Some(at) = text[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = text[..start].chars().next_back().is_none_or(|ch| !ident(ch));
        let after = text[end..].chars().next().is_none_or(|ch| !ident(ch));
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

// One `.route(<path>, <method>(<handler>))` statement of a list: whether it
// registers a topic, its first argument qualified through `topic_for` or
// `topic`; the path, a literal or the table's constant the first argument
// names by its last segment, `?` where it is neither; and the methods the
// second argument spells, one per route fn (`get(..)`) or
// `MethodFilter::<M>`, `?` where it spells none.
struct Registered {
    topic: bool,
    path: String,
    methods: Vec<String>,
}

// Each statement of a list, in order.
fn registered(list: &str, table: &str) -> Vec<Registered> {
    let constants = constants(table);
    let mut routes = Vec::new();
    let mut from = 0;
    while let Some(found) = list[from..].find(".route(") {
        let open = from + found + ".route(".len();
        let Some(close) = closing(&list[open..]).map(|at| open + at) else {
            break;
        };
        let call = &list[open..close];
        let (first, second) =
            top_level_comma(call).map_or((call, ""), |at| (&call[..at], &call[at + 1..]));
        let first = first.trim();
        let path = literal(first)
            .or_else(|| {
                let name = first.rsplit("::").next()?;
                constants
                    .iter()
                    .find(|(declared, _)| declared == name)
                    .map(|(_, text)| text.as_str())
            })
            .unwrap_or("?")
            .to_owned();
        let topic = first.contains("topic_for(") || first.contains("topic(");
        let mut methods = methods_in(second);
        if methods.is_empty() {
            methods.push("?".to_owned());
        }
        routes.push(Registered { topic, path, methods });
        from = close;
    }
    routes
}

// The offset of the `)` closing a call whose `(` is just before `text`,
// string literals skipped; none where the call is unclosed.
fn closing(text: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            _ if escaped => escaped = false,
            b'\\' if quoted => escaped = true,
            b'"' => quoted = !quoted,
            _ if quoted => {}
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

// The offset of the first `,` of `text` outside every bracket and string.
fn top_level_comma(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quoted = false;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            _ if quoted => {}
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

// The text of a plain string literal, `"…"`, with nothing around it.
fn literal(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('"')).then_some(inner)
}

// The methods a route expression spells, in order: each route fn called
// bare (`get(..)`, never `map.get(..)`) and each `MethodFilter::<M>`.
fn methods_in(expression: &str) -> Vec<String> {
    const VERBS: [&str; 7] = ["get", "post", "put", "patch", "delete", "head", "options"];
    let mut methods = Vec::new();
    let mut at = 0;
    while let Some(found) = expression[at..].find(|c: char| c.is_ascii_alphabetic() || c == '_') {
        let start = at + found;
        let end = expression[start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(expression.len(), |len| start + len);
        let word = &expression[start..end];
        let before = &expression[..start];
        let called = expression[end..].starts_with('(');
        if before.ends_with("MethodFilter::") && word.bytes().all(|b| b.is_ascii_uppercase()) {
            methods.push(word.to_owned());
        } else if VERBS.contains(&word) && called && !before.ends_with('.') {
            methods.push(word.to_ascii_uppercase());
        }
        at = end;
    }
    methods
}

// The names of the directories under `dir`; none where `dir` is no directory.
fn directories(dir: &Path) -> io::Result<Vec<String>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

// Whether a list re-exports the directory `name` beside it: an
// `export * as … from "./<name>/index.ts"` line, the specifier read
// whatever it names within the directory.
fn reexported(list: &str, name: &str) -> bool {
    list.lines().map(str::trim).filter(|line| line.starts_with("export * as")).any(|line| {
        let Some((_, from)) = line.split_once(" from ") else {
            return false;
        };
        let specifier = from.trim().trim_end_matches(';').trim_matches(|c| c == '"' || c == '\'');
        specifier == format!("./{name}") || specifier.starts_with(&format!("./{name}/"))
    })
}

// What the host's own checks say of the tree a build left: the layout's
// three checks run in a fresh export of the tip, the tests they ran, the
// scenarios of the merged slices a test is named by, the modules the
// layout's lists do not reach, the tracked files outside the layout, what
// the checks wrote besides the lockfile, and, for an `omnia` build of a
// case naming a reference guest, the tree measured against it and its
// fixtures driven through the guest.
struct Fidelity {
    // the file beside the run's log the pass is written to
    file: String,
    tree: String,
    target: Target,
    checks: Vec<HostCheck>,
    // the tests the runner reported passed and failed
    passed: usize,
    failed: usize,
    // the test names the tree's test files declare
    tests: usize,
    scenarios: Vec<Scenario>,
    unreached: Vec<String>,
    routes: Vec<Route>,
    outside: Vec<String>,
    wrote: Vec<String>,
    reference: Option<reference::Comparison>,
}

// One route a module of the tree mounts, as its source spells it; a probe
// is a route whose module's requirements state no method and path, so the
// surface is the layout's shape and not the specification's.
#[derive(Debug, PartialEq, Eq)]
struct Route {
    module: String,
    method: String,
    path: String,
    probe: bool,
}

// One command the host ran in the exported tree: its exit, or none where an
// earlier check's failure left it unrun, what it printed, and the tail of it.
struct HostCheck {
    name: &'static str,
    exit: Option<i32>,
    output: String,
    note: String,
}

impl HostCheck {
    fn passed(&self) -> bool {
        self.exit == Some(0)
    }

    // The one line of the note the table shows: the TAP counts for Node's
    // runner, the summed `test result` counts for cargo's, the last line
    // printed for the rest.
    fn summary(&self) -> String {
        let lines: Vec<&str> =
            self.note.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
        if self.name == "node --test" {
            let counts: Vec<&str> = lines
                .iter()
                .copied()
                .filter(|line| {
                    ["# tests", "# pass", "# fail"].iter().any(|key| line.starts_with(key))
                })
                .collect();
            if !counts.is_empty() {
                return counts.join(" · ");
            }
        }
        if self.name == "cargo test" && self.output.contains("test result:") {
            let (passed, failed) = cargo_test_counts(&self.output);
            return format!("{passed} passed · {failed} failed");
        }
        lines.last().copied().unwrap_or("").to_owned()
    }
}

// One acceptance scenario of a merged slice's requirement, and the test
// named by it, if any.
struct Scenario {
    stem: String,
    unknown: bool,
    named: Option<Named>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Named {
    Exact,
    Fuzzy,
}

impl Fidelity {
    fn read(
        project: &Path, tag: &str, tree: &Tree, build: &Build, spec: Option<&Value>,
        target: Target, reference: Option<&Reference>,
    ) -> io::Result<Self> {
        let export = project.join(format!("{tag}.fidelity"));
        let tracked = export_tip(tree, &export)?;

        // the layout's checks under the tree's own configuration, then the
        // tests the runner counted
        let shim = target.config_shim().map(|name| export.join(name)).filter(|path| !path.exists());
        if let Some(shim) = &shim {
            fs::write(shim, "")?;
        }
        let checks = target.checks(&export);
        if let Some(shim) = &shim {
            fs::remove_file(shim)?;
        }
        let (passed, failed) = checks
            .iter()
            .find(|check| check.name == target.runner())
            .map_or((0, 0), |runner| target.test_counts(runner));

        // the tests the tree declares, against the merged slices' scenarios
        let names = target.test_names(&export)?;
        let merged: BTreeSet<String> =
            build.integrated().into_iter().flat_map(|slice| slice.requirements).collect();
        let requirements = spec
            .and_then(|spec| spec["requirements"].as_array())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut scenarios = Vec::new();
        for requirement in requirements {
            let Some(id) = requirement["id"].as_str() else {
                continue;
            };
            if !merged.is_empty() && !merged.contains(id) {
                continue;
            }
            let stem = requirement["subject"].as_str().and_then(stem).unwrap_or("").to_owned();
            let listed = requirement["scenarios"].as_array().map(Vec::as_slice).unwrap_or_default();
            for scenario in listed {
                scenarios.push(Scenario {
                    stem: stem.clone(),
                    unknown: scenario["then"].as_str() == Some("[unknown]"),
                    named: named_by(scenario["name"].as_str().unwrap_or(""), &names),
                });
            }
        }

        // the modules the lists do not reach, the routes they mount, the
        // files outside the layout, and what the checks wrote
        let unreached = target.unreached(&export)?;
        let routes = routes(target.routes(&export), &build.integrated(), requirements);
        let stems: BTreeSet<&str> =
            requirements.iter().filter_map(|r| r["subject"].as_str().and_then(stem)).collect();
        let outside =
            tracked.iter().filter(|path| !target.within_layout(path, &stems)).cloned().collect();
        let mut present = Vec::new();
        files_under(&export, &export, target.scratch(), &mut present)?;
        let wrote = present
            .into_iter()
            .filter(|path| path != target.lockfile() && !tracked.contains(path))
            .collect();

        // the reference guest the case names, measured against the tree and
        // its fixtures driven through the guest, for an omnia build alone
        let reference =
            reference.filter(|_| matches!(target, Target::Omnia)).and_then(|reference| {
                match reference.compare(&export) {
                    Ok(comparison) => Some(comparison),
                    Err(error) => {
                        eprintln!("eval: `{tag}` reference not compared: {error}");
                        None
                    }
                }
            });
        fs::remove_dir_all(&export)?;

        Ok(Self {
            file: format!("{tag}.fidelity.md"),
            tree: tree.to_string(),
            target,
            checks,
            passed,
            failed,
            tests: names.len(),
            scenarios,
            unreached,
            routes,
            outside,
            wrote,
            reference,
        })
    }

    fn probes(&self) -> usize {
        self.routes.iter().filter(|route| route.probe).count()
    }

    // Whether the host's three checks all passed, with every test.
    fn clean(&self) -> bool {
        self.checks.iter().all(HostCheck::passed) && self.failed == 0
    }

    fn named(&self) -> usize {
        self.scenarios.iter().filter(|scenario| scenario.named.is_some()).count()
    }

    // The `host checks` cell: `ok` with the tests' count, else each check
    // that failed or was left unrun.
    fn host_cell(&self) -> String {
        let tests = format!("{}/{}", self.passed, self.passed + self.failed);
        if self.clean() {
            return format!("ok {tests}");
        }
        let mut parts = Vec::new();
        for check in &self.checks {
            let runner = check.name == self.target.runner();
            match check.exit {
                Some(0) if !runner || self.failed == 0 => {}
                Some(_) if runner => parts.push(format!("tests {tests}")),
                Some(_) => parts.push(format!("{} ✗", check.name)),
                None => parts.push(format!("{} unrun", check.name)),
            }
        }
        parts.join(" · ")
    }

    // The `scenarios named` cell: the scenarios a test is named by, the
    // tracked files outside the layout, the modules the lists do not
    // reach, and the probe routes.
    fn scenarios_cell(&self) -> String {
        let total = self.scenarios.len();
        let percent = (self.named() * 100).checked_div(total).unwrap_or(0);
        format!(
            "{}/{total} ({percent}%) · {} outside · {} unreached · {} probes",
            self.named(),
            self.outside.len(),
            self.unreached.len(),
            self.probes()
        )
    }

    // The routes the tree mounts, per module, each probe marked.
    fn routes_section(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.routes.is_empty() {
            return writeln!(f, "\n## Routes\n\n- none: 0 routes mounted.");
        }
        let modules: BTreeSet<&str> =
            self.routes.iter().map(|route| route.module.as_str()).collect();
        writeln!(
            f,
            "\n## Routes\n\n- {} routes under {} modules; {} probes, a route whose module's \
             requirements state no method and path.",
            self.routes.len(),
            modules.len(),
            self.probes()
        )?;
        for module in modules {
            let of: Vec<String> = self
                .routes
                .iter()
                .filter(|route| route.module == module)
                .map(|route| {
                    let probe = if route.probe { " (probe)" } else { "" };
                    format!("`{} {}`{probe}", route.method, route.path)
                })
                .collect();
            writeln!(f, "  - `{module}`: {}", of.join(", "))?;
        }
        Ok(())
    }
}

// The routes a tree mounts, each marked a probe where the merged slice
// owning its module has no requirement stating a method and path.
fn routes(
    mounted: Vec<(String, String, String)>, slices: &[Integrated], requirements: &[Value],
) -> Vec<Route> {
    mounted
        .into_iter()
        .map(|(module, method, path)| {
            let probe = slice_of(slices, &module)
                .is_some_and(|slice| !states_route(&slice.requirements, requirements));
            Route {
                module,
                method,
                path,
                probe,
            }
        })
        .collect()
}

// The merged slice whose module `crates/<stem>/src/<part>.rs` is: the one
// named `<stem>-<part>`, or `<stem>` where the part is the stem itself, or
// `<part>` where the slice's name went in whole under another crate, as
// the bootstrap's under `start`; the module spells the name's hyphens as
// underscores.
fn slice_of<'a>(slices: &'a [Integrated], module: &str) -> Option<&'a Integrated> {
    let rest = module.strip_prefix("crates/")?;
    let (stem, rest) = rest.split_once("/src/")?;
    let part = rest.strip_suffix(".rs")?;
    let stem = stem.replace('-', "_");
    let name = if part == stem { stem } else { format!("{stem}_{part}") };
    let named = |wanted: &str| slices.iter().find(|slice| slice.name.replace('-', "_") == wanted);
    named(&name).or_else(|| named(part))
}

// Whether any of the requirements `ids` name, among `requirements`, states
// an HTTP surface: a method in capitals followed by a path, `GET
// /orders/{id}`, in backticks or bare, in its body or in a scenario's
// given, when, then, or and.
fn states_route(ids: &[String], requirements: &[Value]) -> bool {
    const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
    let texts = |value: &Value| -> Vec<String> {
        match value {
            Value::String(text) => vec![text.clone()],
            Value::Array(items) => {
                items.iter().filter_map(Value::as_str).map(str::to_owned).collect()
            }
            _ => Vec::new(),
        }
    };
    requirements
        .iter()
        .filter(|requirement| {
            requirement["id"].as_str().is_some_and(|id| ids.iter().any(|x| x == id))
        })
        .flat_map(|requirement| {
            let mut lines = texts(&requirement["body"]);
            for scenario in requirement["scenarios"].as_array().into_iter().flatten() {
                for clause in ["given", "when", "then", "and"] {
                    lines.extend(texts(&scenario[clause]));
                }
            }
            lines
        })
        .any(|line| {
            let words: Vec<&str> = line
                .split(|c: char| c.is_whitespace() || c == '`')
                .filter(|w| !w.is_empty())
                .collect();
            words.windows(2).any(|pair| METHODS.contains(&pair[0]) && pair[1].starts_with('/'))
        })
}

impl Display for Fidelity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "# Fidelity\n\nTree: {}.\n\n## The host's checks\n", self.tree)?;
        writeln!(f, "| check | exit | note |\n| --- | --- | --- |")?;
        for check in &self.checks {
            let exit = check.exit.map_or_else(|| "unrun".to_owned(), |code| code.to_string());
            writeln!(f, "| `{}` | {exit} | {} |", check.name, check.summary())?;
        }
        writeln!(
            f,
            "\n## Tests\n\n- {} test names in the tree; `{}` reports {} passed, {} failed.",
            self.tests,
            self.target.runner(),
            self.passed,
            self.failed
        )?;

        // the scenarios, by stem
        let total = self.scenarios.len();
        let exact = self.scenarios.iter().filter(|s| s.named == Some(Named::Exact)).count();
        let unknown = self.scenarios.iter().filter(|s| s.unknown).count();
        writeln!(
            f,
            "\n## Scenarios named by a test\n\n- {}/{total} scenarios of the merged slices have a \
             test named by them: {exact} by the scenario's name exactly, {} by a test name \
             containing it or contained in it.\n- {unknown} scenarios have `then [unknown]`.\n",
            self.named(),
            self.named() - exact,
        )?;
        writeln!(
            f,
            "| stem | scenarios | exact | fuzzy | then [unknown] |\n| --- | --- | --- | --- | --- |"
        )?;
        let stems: BTreeSet<&str> = self.scenarios.iter().map(|s| s.stem.as_str()).collect();
        for stem in stems {
            let of: Vec<&Scenario> = self.scenarios.iter().filter(|s| s.stem == stem).collect();
            writeln!(
                f,
                "| `{stem}` | {} | {} | {} | {} |",
                of.len(),
                of.iter().filter(|s| s.named == Some(Named::Exact)).count(),
                of.iter().filter(|s| s.named == Some(Named::Fuzzy)).count(),
                of.iter().filter(|s| s.unknown).count(),
            )?;
        }

        // the files
        listed(f, "## Modules the lists do not reach", "modules no list names", &self.unreached)?;
        self.routes_section(f)?;
        listed(f, "## Files outside the layout", "tracked files outside it", &self.outside)?;
        listed(f, "## What the checks wrote", "paths besides the lockfile", &self.wrote)?;

        // the reference
        if let Some(comparison) = &self.reference {
            write!(f, "{comparison}")?;
        }
        Ok(())
    }
}

// A section counting `paths` and listing them, `none` where there are none.
fn listed(f: &mut fmt::Formatter<'_>, heading: &str, what: &str, paths: &[String]) -> fmt::Result {
    if paths.is_empty() {
        return writeln!(f, "\n{heading}\n\n- none: 0 {what}.");
    }
    writeln!(f, "\n{heading}\n\n- {} {what}:", paths.len())?;
    for path in paths {
        writeln!(f, "  - `{path}`")?;
    }
    Ok(())
}

// One command of the host's toolchain run in `at`, its stdout and stderr
// read together for the note.
#[expect(clippy::disallowed_types, reason = "the eval runs the host's toolchain over the tree")]
fn host_check(at: &Path, command: &'static str, args: &[&str]) -> HostCheck {
    let mut words = command.split(' ');
    let program = words.next().unwrap_or(command);
    let output = std::process::Command::new(program)
        .args(words)
        .args(args)
        .current_dir(at)
        .env_remove("CURSOR_API_KEY")
        .output();
    match output {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            let mut tail: Vec<&str> =
                text.lines().filter(|line| !line.trim().is_empty()).rev().take(12).collect();
            tail.reverse();
            HostCheck {
                name: command,
                exit: output.status.code(),
                note: tail.join("\n"),
                output: text,
            }
        }
        Err(error) => HostCheck {
            name: command,
            exit: None,
            output: String::new(),
            note: format!("not run: {error}"),
        },
    }
}

fn unrun(command: &'static str, why: &str) -> HostCheck {
    HostCheck {
        name: command,
        exit: None,
        output: String::new(),
        note: format!("not run: {why}"),
    }
}

// The `# pass N` and `# fail N` lines of a TAP summary.
fn tap_counts(note: &str) -> (usize, usize) {
    let count = |key: &str| {
        note.lines()
            .rev()
            .filter_map(|line| line.trim().strip_prefix(key))
            .find_map(|rest| rest.trim().parse::<usize>().ok())
            .unwrap_or(0)
    };
    (count("# pass"), count("# fail"))
}

// The `N passed` and `M failed` of every `test result:` line cargo's runner
// prints, one per test binary, summed.
fn cargo_test_counts(output: &str) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    for line in output.lines().filter_map(|line| line.trim().strip_prefix("test result:")) {
        for field in line.split(';') {
            let words: Vec<&str> = field.split_whitespace().collect();
            if let [.., count, kind] = words.as_slice()
                && let Ok(count) = count.parse::<usize>()
            {
                match *kind {
                    "passed" => passed += count,
                    "failed" => failed += count,
                    _ => {}
                }
            }
        }
    }
    (passed, failed)
}

// Every `#[test]` or `#[tokio::test]` fn under `dir`, read from the files'
// text and spelled as a scenario, its underscores as spaces.
fn rust_test_names(dir: &Path, names: &mut Vec<String>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            rust_test_names(&path, names)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let text = fs::read_to_string(&path)?;
            names.extend(declared_rust_tests(&text));
        }
    }
    Ok(())
}

fn declared_rust_tests(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line != "#[test]" && !line.starts_with("#[tokio::test") {
            continue;
        }
        // the fn head follows, past any further attribute
        let head = lines.by_ref().find(|line| !line.starts_with("#["));
        let Some(rest) = head.and_then(|head| head.split_once("fn ")) else {
            continue;
        };
        let name: String =
            rest.1.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !name.is_empty() {
            names.push(name.replace('_', " "));
        }
    }
    names
}

// Every `test("…")` and `it("…")` name under `dir`, read from the files'
// text; a name over a template or an expression is passed over.
fn test_names(dir: &Path, names: &mut Vec<String>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            test_names(&path, names)?;
        } else if path.extension().is_some_and(|ext| ext == "ts") {
            let text = fs::read_to_string(&path)?;
            names.extend(declared_tests(&text));
        }
    }
    Ok(())
}

fn declared_tests(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for opener in ["test(", "it("] {
        for (at, _) in text.match_indices(opener) {
            let led = text[..at].chars().next_back();
            if led.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                continue;
            }
            let rest = text[at + opener.len()..].trim_start();
            let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                continue;
            };
            if let Some(end) = rest[1..].find(quote) {
                names.push(rest[1..=end].to_owned());
            }
        }
    }
    names
}

// The test named by a scenario: one spelled exactly as it is, else one that
// contains it or that it contains, both read without case; a short name is
// never contained, since a word is not a scenario.
fn named_by(scenario: &str, tests: &[String]) -> Option<Named> {
    let scenario = scenario.trim().to_lowercase();
    if scenario.is_empty() {
        return None;
    }
    let lowered: Vec<String> = tests.iter().map(|test| test.trim().to_lowercase()).collect();
    if lowered.contains(&scenario) {
        return Some(Named::Exact);
    }
    let fuzzy = lowered.iter().any(|test| {
        (test.len() >= 12 && scenario.contains(test.as_str()))
            || (scenario.len() >= 12 && test.contains(scenario.as_str()))
    });
    fuzzy.then_some(Named::Fuzzy)
}

// Every file beneath `dir` as a `/`-separated path relative to `root`, the
// checks' `scratch` directory and `.git` passed over.
fn files_under(root: &Path, dir: &Path, scratch: &str, found: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == scratch || name == ".git" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            files_under(root, &path, scratch, found)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            let parts: Vec<String> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            found.push(parts.join("/"));
        }
    }
    Ok(())
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
// `key=value` pairs after the message. The lines before it fill the rest:
// the completion's shell calls, under its `complete{n=…}` span, paired
// start to completion by subject; and the SDK's `responded` lines of the
// turn it ran, `turn=<slice id>` for a build and `turn=verify` for a
// verify, which the outcome line closes, since a wave's verifies never
// overlap.
fn completions(stderr: &str) -> Vec<Completion> {
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
    absent: Absent,
    behaviours: Behaviours,
    // what each source's survey and stems read against its expectations
    sources: Vec<SourceGrade>,
    // where the specification's own words name a technology; expected empty
    leaks: Vec<String>,
}

// The accepted requirement claims anchored within the spans expected to
// state none: one hit per claim, naming the span and the claim.
#[derive(Debug, Default, PartialEq, Eq)]
struct Absent {
    expected: usize,
    hits: Vec<String>,
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
        absent: absent(&expected.absent, expected, run),
        behaviours: behaviours(&expected.behaviours, run),
        sources,
        leaks: run.leaks(),
    }
}

// An absent span is hit by every accepted requirement claim of its source
// whose anchor is held to it (`held`): a claim half or more within the
// span states what the span does, while one a line wider than it does not.
// A span naming no lines is hit by any claim in its file. Each hit is
// labelled by the span and the claim, so the card's reading names the
// passage of the prompt the claim passed through.
fn absent(items: &[Item], expected: &Expected, run: &Run) -> Absent {
    let mut absent = Absent {
        expected: items.len(),
        ..Absent::default()
    };
    for item in items {
        let source = match (&item.source, expected.sources.len()) {
            (Some(source), _) => format!("{source}:"),
            (None, 1) => String::new(),
            (None, _) => "?:".to_owned(),
        };
        let (Ok(of), Ok(span)) =
            (expected.source(item.source.as_deref()), Anchor::parse(&item.anchor))
        else {
            continue;
        };
        for claim in run.claims_of(of.name(), ClaimKind::Requirement) {
            let Some(Ok(anchor)) = claim.anchor() else { continue };
            if held(anchor, span) {
                absent.hits.push(format!(
                    "{source}`{}` — {} (`{}` at `{}`)",
                    item.anchor,
                    item.gloss,
                    claim.id.as_deref().unwrap_or("?"),
                    claim.path.as_deref().unwrap_or("?"),
                ));
            }
        }
    }

    absent
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

// A requirement stated through a platform's mechanism: the code
// identifiers its body names that the design owns no type for, with the
// sources it cites.
#[derive(Debug, PartialEq, Eq)]
struct Stated {
    id: String,
    sources: Vec<String>,
    identifiers: Vec<String>,
}

// The requirements naming a code identifier the design owns no type for,
// in specification order. A body names one in backticks or bare, since one
// adapter's model sets code apart and another's does not. The figure is a
// proxy for a requirement stated through a platform's mechanism rather than
// its behaviour, and the list beside the count is its reading: a product
// name (`TypeORM`) has the shape, and a platform-neutral requirement names
// neither.
fn mechanism(requirements: &[Value], types: &BTreeSet<&str>) -> Vec<Stated> {
    requirements
        .iter()
        .filter_map(|requirement| {
            let body: Vec<&str> = requirement["body"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            let mut identifiers: Vec<String> = Vec::new();
            for span in body.iter().flat_map(|line| spans(line)) {
                if identifier(span).is_some_and(|name| !types.contains(name))
                    && !identifiers.iter().any(|seen| seen == span)
                {
                    identifiers.push(span.to_owned());
                }
            }
            if identifiers.is_empty() {
                return None;
            }
            let mut sources: Vec<String> = Vec::new();
            for source in citations(requirement).map(|(source, _)| source) {
                if !sources.iter().any(|seen| seen == source) {
                    sources.push(source.to_owned());
                }
            }
            Some(Stated {
                id: requirement["id"].as_str().unwrap_or_default().to_owned(),
                sources,
                identifiers,
            })
        })
        .collect()
}

// The spans of a line an identifier may stand in: each span set in backticks,
// whole, and each bare word between them, its surrounding punctuation
// stripped and a call's parentheses kept.
fn spans(line: &str) -> impl Iterator<Item = &str> {
    line.split('`').enumerate().flat_map(|(index, part)| {
        let quoted = index % 2 == 1;
        let words: Box<dyn Iterator<Item = &str>> = if quoted {
            Box::new(std::iter::once(part))
        } else {
            Box::new(part.split_whitespace().map(bare))
        };
        words.filter(|span| !span.is_empty())
    })
}

fn bare(word: &str) -> &str {
    let mut word = word.trim_start_matches(['(', '[', '"', '\'', '*']);
    loop {
        let trimmed = word.trim_end_matches(['.', ',', ';', ':', '!', '?', ']', '"', '\'', '*']);
        let unbalanced =
            trimmed.ends_with(')') && trimmed.matches(')').count() > trimmed.matches('(').count();
        let trimmed = if unbalanced { &trimmed[..trimmed.len() - 1] } else { trimmed };
        if trimmed.len() == word.len() {
            return word;
        }
        word = trimmed;
    }
}

// The name a span spells a platform's code identifier by, when it does: a
// decorator (`@Exclude({ toPlainOnly: true })`), a call (`AuthGuard('jwt')`,
// `validate()`, `I18nContext.current()`; an English plural, `token(s)`, is
// none), or a type's name in CamelCase (`FileInterceptor`, `TypeORM`). The
// name is the span's first segment, the one a design type is known by. A
// field, an error key, a configuration key, and a function of the program's
// own (`refreshToken`, `errors.email`, `auth.secret`, `setTier`) are its
// vocabulary on any platform and none of these, nor is a single capitalised
// word, an acronym, a `SCREAMING_SNAKE` setting, a path, a route, or a
// value.
fn identifier(span: &str) -> Option<&str> {
    let (decorated, span) = span.strip_prefix('@').map_or((false, span), |rest| (true, rest));
    let (arguments, head) = match (span.find('('), span.ends_with(')')) {
        (Some(open), true) => (Some(&span[open + 1..span.len() - 1]), &span[..open]),
        (None, false) => (None, span),
        _ => return None,
    };
    let segment = |text: &str| {
        let mut chars = text.chars();
        chars.next().is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    let segments: Vec<&str> = head.split('.').collect();
    if !segments.iter().all(|part| segment(part)) {
        return None;
    }
    let name = segments.first().copied()?;
    let typed = segments.len() == 1
        && name.starts_with(|c: char| c.is_ascii_uppercase())
        && camel_case(name);
    let called = arguments.is_some_and(|arguments| {
        arguments.is_empty()
            || arguments.starts_with(['\'', '"', '{', '[', '`'])
            || arguments.starts_with(|c: char| c.is_ascii_digit())
            || segments.len() > 1
            || camel_case(name)
    });
    (decorated || called || typed).then_some(name)
}

// A name in CamelCase: a lowercase letter directly followed by an uppercase
// one, which `Authorization`, `JWTs`, and `S3` lack.
fn camel_case(name: &str) -> bool {
    name.as_bytes()
        .windows(2)
        .any(|pair| pair[0].is_ascii_lowercase() && pair[1].is_ascii_uppercase())
}

// --- the scorecard ---

fn scorecard(settings: &Settings, reports: &[Report<'_>], started: &str, name: &str) -> String {
    let ladder: Vec<String> = settings.ladder.iter().map(Rung::to_string).collect();
    let adapters: Vec<String> = settings
        .wasm
        .iter()
        .map(|(adapter, path)| format!("adapter `{adapter}` `{}`", path.display()))
        .collect();
    let built = settings.target.map_or_else(String::new, |target| {
        let jobs = settings
            .jobs
            .map_or_else(|| "the engine's default".to_owned(), |jobs| format!("`--jobs {jobs}`"));
        format!(
            " · each landed plan built through `{}` into a repository the engine laid under \
             {jobs} at {} (`EVAL_BUILD_RUNG`)",
            target.name(),
            settings.build_rung
        )
    });
    let budget = settings
        .max_tool_calls
        .as_deref()
        .map_or_else(String::new, |calls| format!(" · `CURSOR_MAX_TOOL_CALLS={calls}`"));
    let rebuilt = settings.rebuild.as_deref().map_or_else(String::new, |card| {
        format!(
            " · REBUILD: each plan is `run-1`'s of `target/eval/{card}/`, built again fresh; the \
             specify columns and completions repeat that card's"
        )
    });
    let mut card = format!(
        "# Eval {started}\n\nmodel `{}` · emery `{}` · {} · {} runs per case · ladder {} \
         (`CURSOR_TIMEOUT_SECS`/`CURSOR_INACTIVITY_SECS`, climbed on a `timeout` or `inactive` \
         completion; the columns are the first rung's){budget}{built}{rebuilt} · runs \
         under `target/eval/{name}/`\n",
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
        let build_columns = if self.built {
            " built | verified | build wall | speedup | covered | host checks | scenarios named |"
        } else {
            ""
        };
        writeln!(
            f,
            "| run | exit | wall | completions | input | cached | output | reasoning | claims | \
             req | crit | recall req | recall crit | recall dec | surfaces | stems | behaviours | \
             conflicts | divergences | unknown | covered | then [unknown] | slices | waves | \
             design blocks (types) | mechanism | leaks |{build_columns}"
        )?;
        writeln!(f, "|{}", " --- |".repeat(if self.built { 34 } else { 27 }))?;
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
        let landed: Vec<&Attempt> = runs
            .iter()
            .filter_map(|attempts| attempts.first())
            .filter(|attempt| attempt.run.exit == Some(0))
            .collect();
        if landed.len() >= 2 {
            let label = format!("across {} runs at {}", landed.len(), self.ladder[0]);
            stability(f, &label, &landed)?;
        } else {
            let landed: Vec<&Attempt> = runs
                .iter()
                .filter_map(|attempts| attempts.last())
                .filter(|attempt| attempt.run.exit == Some(0))
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
    let stated = run.mechanism();
    let mechanism = per_source(&grade.sources, named, |source| {
        let named = stated.iter().filter(|stated| stated.sources.contains(&source.name)).count();
        format!("{named}/{}", run.citing(&source.name))
    });

    write!(
        f,
        "| {} | {} | {}s | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | \
         {} | {} | {} | {} | {} | {} | {} ({}) | {mechanism} | {} |",
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
        grade.leaks.len(),
    )
}

// The seven build cells after a row's own: what the build merged, verified,
// took, saved over a serial walk, and covered, then what the host's checks
// and the tests say of the tree it left; `—` where no plan was built, and
// in the last two where the build left no tree.
fn build_cells(f: &mut Formatter<'_>, attempt: &Attempt) -> fmt::Result {
    let run = &attempt.run;
    let Some(build) = &run.build else {
        return write!(f, " — | — | — | — | — | — | — |");
    };
    let integrated = build.integrated();
    let resolved = integrated.iter().filter(|slice| !slice.resolved.is_empty()).count();
    let mut merged = format!("{}/{}", integrated.len(), run.slices());
    if resolved > 0 {
        let _ = write!(merged, " ~{resolved}");
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
    let (host, scenarios) = build.fidelity.as_ref().map_or_else(
        || ("—".to_owned(), "—".to_owned()),
        |fidelity| (fidelity.host_cell(), fidelity.scenarios_cell()),
    );
    write!(
        f,
        " {merged} | {verified} | {wall} | {speedup} | {covered}/{all} | {host} | {scenarios} |"
    )
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
        if grade.absent.expected > 0 {
            writeln!(f, "- absent hits: {}", grade.absent.hits.len())?;
            for hit in &grade.absent.hits {
                writeln!(f, "- absent hit: {hit}")?;
            }
        }
    }
    if run.spec.is_some() {
        mechanism_notes(f, run, &grade.sources, &led)?;
    }
    for leak in &grade.leaks {
        writeln!(f, "- leak: {leak}")?;
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

// The requirements each source stated through a mechanism, with the
// identifiers each names, one line per source naming any.
fn mechanism_notes(
    f: &mut Formatter<'_>, run: &Run, sources: &[SourceGrade], led: &dyn Fn(&str) -> String,
) -> fmt::Result {
    let stated = run.mechanism();
    for source in sources {
        let listed: Vec<String> = stated
            .iter()
            .filter(|stated| stated.sources.contains(&source.name))
            .map(|stated| {
                let identifiers: Vec<String> =
                    stated.identifiers.iter().map(|name| format!("`{name}`")).collect();
                format!("{} ({})", stated.id, identifiers.join(", "))
            })
            .collect();
        if !listed.is_empty() {
            writeln!(
                f,
                "- {}mechanism {}/{}: {}",
                led(&source.name),
                listed.len(),
                run.citing(&source.name),
                listed.join(", ")
            )?;
        }
    }
    Ok(())
}

// The build beneath its run: how it ended, each wave's slices as they
// merged, what the model resolved, what the verify refused, what was left
// uncovered, the chain the repository holds, and the turns it spent.
fn build_notes(f: &mut Formatter<'_>, run: &Run, build: &Build) -> fmt::Result {
    let integrated = build.integrated();
    let tokens = build.tokens();
    build_ending(f, run, build, integrated.len())?;

    // each wave's slices, as they merged, and what its verify sealed
    let waves: BTreeSet<usize> = integrated.iter().map(|slice| slice.wave).collect();
    let turns = build.wave_turns();
    let repairs = build.repairs();
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
        let spent = wave
            .checked_sub(1)
            .and_then(|k| turns.get(k))
            .map_or_else(String::new, |turns| turns.spent(repairs.get(&wave)));
        writeln!(f, "- wave {wave}: {}{spent}", slices.join(", "))?;
    }
    build_findings(f, build, &integrated, &repairs)?;

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
        let writes = if completion.writes.any() {
            format!("; {}", completion.writes)
        } else {
            String::new()
        };
        writeln!(
            f,
            "- build completion `{}` {} in {}s: {} input, {} cached, {} output ({} \
             reasoning){writes}",
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

// What the build's waves left to read: the slices whose merge the model
// resolved and the paths, the waves the verify repaired and the files each
// sealed, the verify's refusals, the uncovered ids per slice, and what the
// host's checks say of the tree.
fn build_findings(
    f: &mut Formatter<'_>, build: &Build, integrated: &[Integrated],
    repairs: &BTreeMap<usize, Vec<String>>,
) -> fmt::Result {
    for slice in integrated.iter().filter(|slice| !slice.resolved.is_empty()) {
        writeln!(
            f,
            "- resolved: {} at {}, combined by the model and merged again",
            slice.id,
            slice.resolved.join(", ")
        )?;
    }
    let repaired = build.repaired();
    if !repaired.is_empty() {
        let waves: Vec<String> = repaired
            .iter()
            .map(|wave| match repairs.get(wave) {
                Some(files) if files.is_empty() => format!("wave {wave} (lock only)"),
                Some(files) => format!("wave {wave} ({})", files.join(", ")),
                None => format!("wave {wave}"),
            })
            .collect();
        writeln!(f, "- repaired by the verify: {}", waves.join(", "))?;
    }
    for failure in build.verify_failures() {
        writeln!(f, "- verify refused: {failure}")?;
    }
    for slice in integrated.iter().filter(|slice| !slice.uncovered().is_empty()) {
        writeln!(f, "- uncovered: {} {}", slice.id, slice.uncovered().join(", "))?;
    }
    if let Some(fidelity) = &build.fidelity {
        let agree = match (build.landed(), fidelity.clean()) {
            (true, true) => "the host's checks agree with the verdicts",
            (true, false) => "the host's checks disagree with the passed verdicts",
            (false, true) => "the host's checks pass the tree the run left",
            (false, false) => "the host's checks fail the tree the run left",
        };
        writeln!(
            f,
            "- host checks over {}: {}; scenarios named {}; {agree} (`{}`)",
            fidelity.tree,
            fidelity.host_cell(),
            fidelity.scenarios_cell(),
            fidelity.file,
        )?;
        if let Some(comparison) = &fidelity.reference {
            writeln!(f, "- {} (`{}`)", comparison.finding(), fidelity.file)?;
        }
    }
    Ok(())
}

// How the build ended: where the label stands, or the failure's code, exit,
// and first line.
fn build_ending(f: &mut Formatter<'_>, run: &Run, build: &Build, merged: usize) -> fmt::Result {
    let exit = build.exit.map_or_else(|| "killed".to_owned(), |code| format!("exit {code}"));
    match (&build.failure, build.exit) {
        (_, Some(0)) => {
            let envelope = build.envelope.as_ref();
            let label = envelope.and_then(|out| out["label"].as_str()).unwrap_or("");
            let head = envelope.and_then(|out| out["head"].as_str()).unwrap_or("");
            let resumed = envelope
                .and_then(|out| out["resumed"].as_array())
                .filter(|resumed| !resumed.is_empty())
                .map_or_else(String::new, |resumed| format!(", {} resumed", resumed.len()));
            writeln!(
                f,
                "\nBuild: {merged}/{} slices merged in {} waves in {}s{resumed}; `{label}` at `{}`",
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

fn stability(f: &mut Formatter<'_>, label: &str, attempts: &[&Attempt]) -> fmt::Result {
    writeln!(f, "\nStability {label}:")?;
    let runs: Vec<&Run> = attempts.iter().map(|attempt| &attempt.run).collect();
    let runs = runs.as_slice();

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

    // the band the absent hits fell in, where the case expects any
    if attempts.iter().any(|attempt| attempt.grade.absent.expected > 0) {
        let hits: Vec<usize> =
            attempts.iter().map(|attempt| attempt.grade.absent.hits.len()).collect();
        let (low, high) = (hits.iter().min().copied(), hits.iter().max().copied());
        let per_run: Vec<String> = attempts
            .iter()
            .zip(&hits)
            .map(|(attempt, hits)| format!("run {} {hits}", attempt.run.n))
            .collect();
        if let (Some(low), Some(high)) = (low, high) {
            writeln!(f, "- absent hits: {low}–{high} ({})", per_run.join(" · "))?;
        }
    }

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
            target: Target::Node,
            wall: Duration::ZERO,
            exit: Some(exit),
            envelope: serde_json::from_str(stdout).ok(),
            failure: failure(stderr),
            completions: completions(stderr),
            history,
            tail: String::new(),
            fidelity: None,
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
     "written": ["src/payments/index.ts"], "commit": "cccccccc", "resolved": ["src/index.ts"]}
  ],
  "verified": ["aaaaaaaa", "bbbbbbbb", "cccccccc"],
  "repaired": [2],
  "head": "cccccccc",
  "label": "emery/feb4bd36",
  "repository": "./.emery/vcs/repos/target"
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
        let ladder: Vec<Rung> = LADDER
            .split(',')
            .map(|rung| Rung::parse(rung, "EVAL_LADDER"))
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(ladder.len(), 3);
        assert_eq!(ladder[1].to_string(), "1200/240");
        assert_eq!(Rung::parse(BUILD_RUNG, "EVAL_BUILD_RUNG").unwrap(), ladder[1]);
        assert!(Rung::parse("1200", "EVAL_LADDER").unwrap_err().starts_with("EVAL_LADDER:"));
        assert!(Rung::parse("a/b", "EVAL_BUILD_RUNG").unwrap_err().starts_with("EVAL_BUILD_RUNG:"));
    }

    #[test]
    fn oracle_args() {
        let args =
            Args::parse(["--oracle", "card", "train"].map(String::from).into_iter()).unwrap();
        assert_eq!(args.oracle.as_deref(), Some("card"));
        assert!(args.filter.contains("train"));
        assert!(args.target.is_none(), "the oracle builds nothing");
        for argv in [
            &["--oracle", "card"][..],
            &["--oracle", "card", "--build", "train"][..],
            &["--oracle", "card", "--facts", "train"][..],
            &["--oracle", "card", "--target", "node", "train"][..],
            &["--oracle"][..],
        ] {
            assert!(Args::parse(argv.iter().map(|arg| (*arg).to_owned())).is_err(), "{argv:?}");
        }
        let omnia = Args::parse(
            ["--oracle", "card", "--target", "omnia", "train"].map(String::from).into_iter(),
        );
        assert!(omnia.is_ok(), "the oracle reads an omnia build");
    }

    #[test]
    fn rust_log_traces_adapter_crates() {
        let filter = rust_log(["documentation", "typescript", "node"]);
        let targets: Vec<_> =
            filter.split(',').map(|part| part.split('=').next().unwrap()).collect();
        assert!(targets.contains(&"emery_documentation"));
        assert!(targets.contains(&"emery_typescript"));
        assert!(targets.contains(&"emery_node"));
        assert!(!targets.contains(&"documentation"));
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
        assert_eq!(integrated[2].resolved, ["src/index.ts"]);
        assert_eq!(build.waves(), 3);
        assert_eq!(build.verified(), 3);
        assert_eq!(build.covered(), (4, 5));
        assert_eq!(build.verify_failures(), [] as [&str; 0]);
        assert_eq!(build.repaired(), [2]);
    }

    fn check(name: &'static str, exit: Option<i32>) -> HostCheck {
        HostCheck {
            name,
            exit,
            output: String::new(),
            note: String::new(),
        }
    }

    #[test]
    fn host_checks_cells() {
        let mut fidelity = Fidelity {
            file: "run-1.fidelity.md".to_owned(),
            tree: "the label `emery/feb4bd36`".to_owned(),
            target: Target::Node,
            checks: vec![
                check("npm install", Some(0)),
                check("npx tsc", Some(0)),
                check("node --test", Some(0)),
            ],
            passed: 12,
            failed: 0,
            tests: 12,
            scenarios: vec![
                Scenario {
                    stem: "orders".to_owned(),
                    unknown: false,
                    named: Some(Named::Exact),
                },
                Scenario {
                    stem: "orders".to_owned(),
                    unknown: true,
                    named: None,
                },
                Scenario {
                    stem: "start".to_owned(),
                    unknown: false,
                    named: Some(Named::Fuzzy),
                },
            ],
            unreached: vec!["src/orders/index.ts".to_owned()],
            routes: vec![
                Route {
                    module: "crates/orders/src/reading.rs".to_owned(),
                    method: "GET".to_owned(),
                    path: "/orders/{id}".to_owned(),
                    probe: false,
                },
                Route {
                    module: "crates/architecture/src/layering.rs".to_owned(),
                    method: "GET".to_owned(),
                    path: "/architecture/layering".to_owned(),
                    probe: true,
                },
            ],
            outside: vec!["scripts/check.mjs".to_owned()],
            wrote: Vec::new(),
            reference: None,
        };
        assert!(fidelity.clean());
        assert_eq!(fidelity.host_cell(), "ok 12/12");
        assert_eq!(fidelity.scenarios_cell(), "2/3 (66%) · 1 outside · 1 unreached · 1 probes");
        assert!(
            fidelity.to_string().contains("- 1 modules no list names:\n  - `src/orders/index.ts`")
        );

        fidelity.checks[1].exit = Some(2);
        fidelity.checks[2].exit = Some(1);
        fidelity.failed = 2;
        assert!(!fidelity.clean());
        assert_eq!(fidelity.host_cell(), "npx tsc ✗ · tests 12/14");

        fidelity.checks[0].exit = Some(1);
        fidelity.checks[1] = unrun("npx tsc", "the install failed");
        fidelity.checks[2] = unrun("node --test", "the install failed");
        assert_eq!(fidelity.host_cell(), "npm install ✗ · npx tsc unrun · node --test unrun");
    }

    // The omnia profile's runner is `cargo test`, so its failures read as
    // the tests' and the guest build's name stands beside them.
    #[test]
    fn omnia_host_checks_cells() {
        let mut fidelity = Fidelity {
            file: "run-1.fidelity.md".to_owned(),
            tree: "the label `emery/feb4bd36`".to_owned(),
            target: Target::Omnia,
            checks: vec![
                check("cargo clippy", Some(0)),
                check("cargo test", Some(0)),
                check("cargo build", Some(0)),
            ],
            passed: 7,
            failed: 0,
            tests: 7,
            scenarios: Vec::new(),
            unreached: Vec::new(),
            routes: Vec::new(),
            outside: Vec::new(),
            wrote: Vec::new(),
            reference: None,
        };
        assert!(fidelity.clean());
        assert_eq!(fidelity.host_cell(), "ok 7/7");
        assert_eq!(fidelity.scenarios_cell(), "0/0 (0%) · 0 outside · 0 unreached · 0 probes");

        fidelity.checks[1].exit = Some(101);
        fidelity.checks[2] = unrun("cargo build", "the tests failed");
        fidelity.failed = 1;
        assert!(!fidelity.clean());
        assert_eq!(fidelity.host_cell(), "tests 7/8 · cargo build unrun");
    }

    #[test]
    fn target_names() {
        assert_eq!(Target::default(), Target::Node);
        assert_eq!(Target::parse("node"), Ok(Target::Node));
        Target::parse("typescript-target").unwrap_err();
        assert_eq!(Target::parse("omnia"), Ok(Target::Omnia));
        Target::parse("omnia-target").unwrap_err();
        assert_eq!(Target::Omnia.lockfile(), "Cargo.lock");
        assert_eq!(Target::Node.runner(), "node --test");
        assert_eq!(Target::Omnia.config_shim(), Some("clippy.toml"));
        assert_eq!(Target::Node.config_shim(), None);
    }

    #[test]
    fn tap_summary() {
        let note = "TAP version 13\nok 1 - a\nnot ok 2 - b\n# tests 2\n# pass 1\n# fail 1\n";
        assert_eq!(tap_counts(note), (1, 1));
        assert_eq!(tap_counts("not run: the install failed"), (0, 0));
    }

    // Cargo prints one `test result` line per test binary; the counts sum
    // over them, and a check that did not run counts nothing.
    #[test]
    fn cargo_test_summary() {
        let output = "     Running tests/reading.rs\n\
                      test an_order_is_returned_by_its_id ... ok\n\
                      test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; \
                      finished in 0.00s\n\
                      \n     Running tests/placing.rs\n\
                      test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered \
                      out; finished in 0.01s\n";
        assert_eq!(cargo_test_counts(output), (3, 1));
        assert_eq!(cargo_test_counts("not run: the lint failed"), (0, 0));
        let runner = HostCheck {
            name: "cargo test",
            exit: Some(101),
            output: output.to_owned(),
            note: String::new(),
        };
        assert_eq!(runner.summary(), "3 passed · 1 failed");
    }

    // A `#[test]` or `#[tokio::test]` fn is a scenario with its underscores
    // as spaces; a helper fn, and an attribute between, are passed over.
    #[test]
    fn rust_tests_named_by_scenarios() {
        let text = "fn app(provider: Provider) -> Router {}\n\
                    \n#[tokio::test]\n\
                    async fn an_order_with_no_items_is_refused() {}\n\
                    \n#[test]\n#[should_panic]\nfn a_paid_order_ships() {}\n\
                    \n#[tokio::test(flavor = \"current_thread\")]\n\
                    async fn the_service_reports_it_is_up<P>() {}\n";
        let names = declared_rust_tests(text);
        assert_eq!(
            names,
            [
                "an order with no items is refused",
                "a paid order ships",
                "the service reports it is up"
            ]
        );
        assert_eq!(named_by("An order with no items is refused", &names), Some(Named::Exact));
        assert_eq!(named_by("a paid order ships to the address", &names), Some(Named::Fuzzy));
        assert_eq!(named_by("an order is cancelled", &names), None);
    }

    // A scratch tree of the test's own under the host's temp dir, laid from
    // `(path, text)` pairs and removed first where an earlier run left it.
    #[expect(
        clippy::disallowed_methods,
        reason = "a unit test lays its tree under the host's temp dir"
    )]
    fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("emery-evals-{name}-{}", std::process::id()));
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        for (path, text) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        root
    }

    // A handler module a statement of either root list names is reached
    // whatever the stem's spelling; one neither names is listed, as is a
    // table constant its list does not name; `lib.rs`, a module with no
    // handler, and `common` itself are passed over.
    // A handler module is reached through its stem's entry, which names its
    // `routes()` or its `topics`, and its stem through the root lists, which
    // merge its `router()` and register its `topics(..)`; a crate nothing
    // merges and its own tests alone name is unreached, a topic constant no
    // module names is unreached, and a module with no handler is passed over.
    #[test]
    fn omnia_modules_unreached_by_the_lists() {
        let root = scratch(
            "omnia-unreached",
            &[
                (
                    "src/routes.rs",
                    "    let router = axum::Router::new();\n    \
                     let router = router.merge(order_items::router());\n    \
                     let router = router.merge(health::router());\n    \
                     router.with_state(Client::new(OWNER, provider))\n",
                ),
                (
                    "src/messaging.rs",
                    "    let router = messaging::Router::new(Client::new(OWNER, provider));\n    \
                     let router = order_items::topics(router, &env);\n    \
                     let router = health::topics(router, &env);\n",
                ),
                (
                    "crates/common/src/topic.rs",
                    "//! Topics.\n\n/// Placed.\npub const ITEM_PLACED: &str = \"items-placed.v1\";\n\
                     /// Shipped.\npub const ITEM_SHIPPED: &str = \"items-shipped.v1\";\n\
                     /// Unwired.\npub const ITEM_LOST: &str = \"items-lost.v1\";\n",
                ),
                ("crates/common/src/config.rs", "pub async fn env(provider: &impl Config) {}\n"),
                ("crates/order-items/Cargo.toml", "[package]\nname = \"order-items\"\n"),
                (
                    "crates/order-items/src/lib.rs",
                    "pub mod fulfilment;\npub mod placing;\npub mod reading;\n\n\
                     pub fn router<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     common::routers([\n        // one entry per part with routes\n        \
                     reading::routes(),\n    ])\n}\n\n\
                     pub fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {\n    \
                     common::topics(router, env).parts([\n        // one entry per part with topics\n        \
                     fulfilment::topics,\n    ])\n}\n",
                ),
                ("crates/key-vault/Cargo.toml", "[package]\nname = \"key-vault\"\n"),
                ("crates/key-vault/src/lib.rs", "pub mod secrets;\n"),
                (
                    "crates/key-vault/src/secrets.rs",
                    "pub async fn fetch<P: Config>(provider: &P) -> Result<String> {}\n",
                ),
                (
                    "crates/key-vault/tests/secrets.rs",
                    "use key_vault::secrets::fetch;\n#[tokio::test]\nasync fn fetches() {}\n",
                ),
                (
                    "crates/order-items/src/reading.rs",
                    "pub(crate) async fn list<P>(input: Q, context: Context<P>) -> Result<Vec<Item>> {}\n",
                ),
                (
                    "crates/order-items/src/fulfilment.rs",
                    "pub(crate) async fn on_placed<P>(input: Item, context: Context<P>) -> Result<()> {\n    \
                     let _ = topic::ITEM_SHIPPED;\n    let _ = topic::ITEM_PLACED;\n}\n",
                ),
                (
                    "crates/order-items/src/placing.rs",
                    "pub(crate) async fn place<P>(input: Place, context: Context<P>) -> Result<Item> {}\n",
                ),
                ("crates/order-items/src/types.rs", "pub struct Item;\n"),
                ("crates/health/Cargo.toml", "[package]\nname = \"health\"\n"),
                ("crates/health/src/lib.rs", "pub mod health;\n"),
                (
                    "crates/health/src/health.rs",
                    "pub(crate) async fn live<P>(input: Q, context: Context<P>) -> Result<()> {}\n",
                ),
            ],
        );
        let unreached = Target::Omnia.unreached(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            unreached,
            [
                "crates/common/src/topic.rs#ITEM_LOST",
                "crates/health/src/health.rs",
                "crates/key-vault/",
                "crates/order-items/src/placing.rs",
            ],
            "a crate its own tests alone name is unreached"
        );
    }

    // Each `.route(` of a part's `routes()` is one entry per method its
    // second argument spells, attributed to the part's module, the path read
    // from the constant the module declares or a literal, a route fn of the
    // part's own spelling no method, a `.route(` over a topic none; a route is
    // a probe when the slice owning the module, found by the module's
    // spelling of its name, has no requirement stating a method and path in
    // its body or scenarios, and a module no merged slice owns is no probe.
    #[test]
    fn omnia_routes_per_module() {
        let root = scratch(
            "omnia-routes",
            &[
                (
                    "crates/orders/src/reading.rs",
                    "pub(crate) const ORDER: &str = \"/orders/{id}\";\n\
                     pub(crate) const ORDERS: &str = \"/orders\";\n\n\
                     pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     axum::Router::new()\n        .route(ORDER, get(get_order))\n        \
                     .route(ORDER, delete(cancel_order))\n        \
                     .route(ORDERS, get(list_orders))\n}\n",
                ),
                (
                    "crates/orders/src/placing.rs",
                    "pub(crate) const ORDERS: &str = \"/orders\";\n\n\
                     pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     axum::Router::new().route(ORDERS, place_order_route())\n}\n\n\
                     pub(crate) fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {\n    \
                     router.route(config::topic_for(env, topic::ORDER_PLACED), messaging::consume(on_placed))\n}\n",
                ),
                (
                    "crates/architecture/src/architecture.rs",
                    "pub(crate) const LAYERING: &str = \"/architecture/layering\";\n\n\
                     pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     axum::Router::new().route(LAYERING, get(describe))\n}\n",
                ),
                (
                    "crates/health/src/health.rs",
                    "pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     axum::Router::new().route(\"/health/ready\", get(ready))\n}\n",
                ),
                (
                    "crates/order-items/src/order_items.rs",
                    "pub(crate) const ORDER_ITEMS: &str = \"/order-items\";\n\n\
                     pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     axum::Router::new().route(ORDER_ITEMS, get(list))\n}\n",
                ),
                (
                    "crates/start/src/application_bootstrap.rs",
                    "pub(crate) const LIFECYCLE: &str = \"/bootstrap/lifecycle\";\n\n\
                     pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     axum::Router::new().route(LIFECYCLE, get(lifecycle))\n}\n",
                ),
            ],
        );
        let routes_read = Target::Omnia.routes(&root);
        fs::remove_dir_all(&root).unwrap();
        let owned = |name: &str, requirements: &[&str]| Integrated {
            id: format!("SLICE-{name}"),
            name: name.to_owned(),
            wave: 1,
            requirements: requirements.iter().map(|id| (*id).to_owned()).collect(),
            covered: Vec::new(),
            files: Vec::new(),
            commit: None,
            resolved: Vec::new(),
        };
        let slices = [
            owned("orders-reading", &["REQ-001"]),
            owned("orders-placing", &["REQ-002"]),
            owned("architecture", &["REQ-003"]),
            owned("order-items", &["REQ-004"]),
            owned("application-bootstrap", &["REQ-005"]),
        ];
        let spec = serde_json::json!([
            {"id": "REQ-001", "body": ["`GET /orders/{id}` answers the order."]},
            {"id": "REQ-002", "body": ["A client places an order."],
             "scenarios": [{"when": "POST /orders with valid fields", "then": "201"}]},
            {"id": "REQ-003", "body": ["Controllers never import repositories directly."]},
            {"id": "REQ-004", "body": ["Items are listed."], "scenarios": [{"given": ["GET /order-items is called"]}]},
            {"id": "REQ-005", "body": ["The application enables shutdown hooks."]},
        ]);
        let requirements = spec.as_array().unwrap();
        let read: Vec<(String, String, String, bool)> = routes(routes_read, &slices, requirements)
            .into_iter()
            .map(|route| (route.module, route.method, route.path, route.probe))
            .collect();
        let expected = [
            ("crates/architecture/src/architecture.rs", "GET", "/architecture/layering", true),
            ("crates/health/src/health.rs", "GET", "/health/ready", false),
            ("crates/order-items/src/order_items.rs", "GET", "/order-items", false),
            ("crates/orders/src/placing.rs", "?", "/orders", false),
            ("crates/orders/src/reading.rs", "GET", "/orders/{id}", false),
            ("crates/orders/src/reading.rs", "DELETE", "/orders/{id}", false),
            ("crates/orders/src/reading.rs", "GET", "/orders", false),
            ("crates/start/src/application_bootstrap.rs", "GET", "/bootstrap/lifecycle", true),
        ];
        let expected: Vec<(String, String, String, bool)> = expected
            .iter()
            .map(|(m, v, p, probe)| ((*m).to_owned(), (*v).to_owned(), (*p).to_owned(), *probe))
            .collect();
        assert_eq!(read, expected);
    }

    // A stem's `index.ts` the entry re-exports is reached, and a part's its
    // stem's list re-exports; a stem the entry lacks and a part the stem's
    // list lacks are listed, a directory with no `index.ts` aside.
    #[test]
    fn typescript_modules_unreached_by_the_lists() {
        let root = scratch(
            "typescript-unreached",
            &[
                (
                    "src/index.ts",
                    "export * as orders from \"./orders/index.ts\";\n\
                     export * as start from './start/index.ts';\n",
                ),
                ("src/orders/index.ts", "export * as reading from \"./reading/index.ts\";\n"),
                ("src/orders/reading/index.ts", "export const read = 1;\n"),
                ("src/orders/placing/index.ts", "export const place = 1;\n"),
                ("src/orders/shared/types.ts", "export type Order = {};\n"),
                ("src/start/index.ts", "export const main = () => {};\n"),
                ("src/health/index.ts", "export const register = () => {};\n"),
                ("src/main.ts", "import { main } from \"./start/index.ts\";\nawait main();\n"),
            ],
        );
        let unreached = Target::Node.unreached(&root).unwrap();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(unreached, ["src/health/index.ts", "src/orders/placing/index.ts"]);
        assert!(reexported("export * as a from \"./a\";", "a"));
        assert!(!reexported("export { main } from \"./bootstrap/index.ts\";", "bootstrap"));
    }

    // The files a wave's seal holds are read from the chain the repository
    // answers: a seal of the lock rewrite alone reads `lock only`, a seal of
    // repairs names them, and a wave sealing nothing has no line.
    #[test]
    fn wave_seals_read_from_the_chain() {
        let root = scratch("wave-seals", &[(".gitignore", ".emery/\n"), ("Cargo.lock", "v1\n")]);
        let commit = |message: &str| {
            git(&root, &["add", "--all"]).unwrap();
            git(&root, &["commit", "--quiet", "--message", message]).unwrap();
        };
        git(&root, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        commit("Initialise");
        fs::create_dir_all(root.join("crates/start/src")).unwrap();
        fs::write(root.join("crates/start/src/lib.rs"), "pub mod api;\n").unwrap();
        commit("SLICE-001 start\n\nSlice: SLICE-001\nRevision: feb4bd36\nWave: 1");
        fs::write(root.join("Cargo.lock"), "v2\n").unwrap();
        commit("Wave 1 verified\n\nRevision: feb4bd36\nSlices: SLICE-001");
        fs::write(root.join("crates/start/src/api.rs"), "pub fn mount() {}\n").unwrap();
        commit("SLICE-002 start-api\n\nSlice: SLICE-002\nRevision: feb4bd36\nWave: 2");
        fs::write(root.join("Cargo.lock"), "v3\n").unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(root.join("crates/start/Cargo.toml"), "[package]\n").unwrap();
        commit("Wave 2 verified\n\nRevision: feb4bd36\nSlices: SLICE-002");
        fs::write(root.join("crates/start/src/more.rs"), "pub fn mount() {}\n").unwrap();
        commit("SLICE-003 start-more\n\nSlice: SLICE-003\nRevision: feb4bd36\nWave: 3");
        let history = log(&root, "HEAD");
        fs::remove_dir_all(&root).unwrap();

        assert_eq!(history.len(), 6);
        assert_eq!(history[1].sealed(), Some(2));
        assert_eq!(history[0].sealed(), None, "a merge seals no wave");
        let mut build = build_from(0, "{}", "", history);
        build.target = Target::Omnia;
        let repairs = build.repairs();
        assert_eq!(repairs.get(&1), Some(&Vec::new()), "wave 1 sealed the lock rewrite alone");
        assert_eq!(
            repairs.get(&2).map(Vec::as_slice),
            Some(&["Cargo.toml".to_owned(), "crates/start/Cargo.toml".to_owned()][..])
        );
        assert_eq!(repairs.get(&3), None, "wave 3 sealed nothing");
    }

    // Two builds interleave and a verify follows: each completion's writes
    // are the `responded` lines of its own turn, closed by its outcome
    // line, a file written again is a rewrite, one at zero bytes is empty,
    // one at under half its last size is shrinking, both the SDK's per-file
    // `wrote=` form and the earlier `files=` with a call total read, the
    // total a file's size only when the call wrote one file, a refused call
    // counted apart; a completion's shell use is its own calls, paired by
    // subject, with the result bytes summed.
    #[test]
    fn writes_and_shell_read_per_completion() {
        let log = "\
2026-10-12T07:13:44.000000Z DEBUG complete{n=1 model=\"m\" format=schema label=\"build-SLICE-001\"}:send{round=1}: omnia_cursor::model::observe: tool call tool=shell subject=\"cargo clippy\"
2026-10-12T07:13:45.000000Z DEBUG emery_sdk::target::write: responded turn=SLICE-001 tool=\"write_files\" wrote=Some([\"crates/a/src/a.rs\"=1000, \"crates/a/Cargo.toml\"=200]) deleted=Some([]) error=None
2026-10-12T07:13:45.500000Z DEBUG emery_sdk::target::write: responded turn=SLICE-002 tool=\"write_files\" files=Some([\"crates/b/src/b.rs\", \"crates/b/Cargo.toml\"]) deleted=Some([]) bytes=1500
2026-10-12T07:13:46.000000Z DEBUG emery_sdk::target::write: responded turn=SLICE-002 tool=\"write_files\" files=Some([\"crates/b/src/b.rs\"]) deleted=Some([]) bytes=900
2026-10-12T07:13:47.500000Z DEBUG complete{n=1 model=\"m\" format=schema label=\"build-SLICE-001\"}:send{round=1}: omnia_cursor::model::observe: tool call completed tool=shell subject=\"cargo clippy\" result_bytes=1024
2026-10-12T07:13:48.000000Z DEBUG emery_sdk::target::write: responded turn=SLICE-001 tool=\"write_files\" wrote=Some([\"crates/a/src/a.rs\"=400]) deleted=Some([]) error=None
2026-10-12T07:13:49.000000Z DEBUG emery_sdk::target::write: responded turn=SLICE-001 tool=\"write_files\" wrote=None deleted=None error=Some(\"write_files: `crates/a/src/a.rs` would shrink\")
2026-10-12T07:13:50.000000Z DEBUG emery_sdk::target::write: responded turn=SLICE-001 tool=\"edit_files\" edited=Some([\"src/routes.rs\"]) error=None
2026-10-12T07:13:51.000000Z DEBUG emery_sdk::target::write: responded turn=SLICE-002 tool=\"write_files\" files=Some([\"crates/b/src/b.rs\"]) deleted=Some([]) bytes=0
2026-10-12T07:13:52.000000Z DEBUG complete{n=2 model=\"m\" format=schema label=\"build-SLICE-002\"}:send{round=1}: omnia_cursor::model::observe: tool call tool=shell subject=\"cargo test\"
2026-10-12T07:13:53.000000Z  INFO complete{n=1 model=\"m\" format=schema label=\"build-SLICE-001\"}: omnia_cursor::model::observe: completion outcome=\"ok\" attempts=1 duration_ms=9000 input_tokens=5 cache_read_tokens=0 output_tokens=2 reasoning_tokens=0
2026-10-12T07:13:55.000000Z DEBUG complete{n=2 model=\"m\" format=schema label=\"build-SLICE-002\"}:send{round=1}: omnia_cursor::model::observe: tool call completed tool=shell subject=\"cargo test\" result_bytes=2048
2026-10-12T07:13:56.000000Z  INFO complete{n=2 model=\"m\" format=schema label=\"build-SLICE-002\"}: omnia_cursor::model::observe: completion outcome=\"ok\" attempts=1 duration_ms=10000 input_tokens=5 cache_read_tokens=0 output_tokens=2 reasoning_tokens=0
2026-10-12T07:13:57.000000Z DEBUG complete{n=3 model=\"m\" format=schema label=\"verify\"}:send{round=1}: omnia_cursor::model::observe: tool call tool=shell subject=\"cargo build\"
2026-10-12T07:13:58.000000Z DEBUG emery_sdk::target::write: responded turn=verify tool=\"write_files\" wrote=Some([\"crates/a/src/a.rs\"=0]) deleted=Some([]) error=None
2026-10-12T07:14:00.000000Z DEBUG complete{n=3 model=\"m\" format=schema label=\"verify\"}:send{round=1}: omnia_cursor::model::observe: tool call completed tool=shell subject=\"cargo build\" result_bytes=10
2026-10-12T07:14:01.000000Z  INFO complete{n=3 model=\"m\" format=schema label=\"verify\"}: omnia_cursor::model::observe: completion outcome=\"ok\" attempts=1 duration_ms=4000 input_tokens=5 cache_read_tokens=0 output_tokens=2 reasoning_tokens=0
";
        let completions = completions(log);
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["build-SLICE-001", "build-SLICE-002", "verify"]);
        assert_eq!(
            completions[0].writes,
            Writes {
                calls: 2,
                rewrites: 1,
                empty: 0,
                shrinking: 1,
                edits: 1,
                refused: 1
            }
        );
        assert_eq!(
            completions[0].writes.to_string(),
            "write_files 2 calls, 1 rewrites, 0 empty, 1 shrinking; edit_files 1 calls; 1 refused"
        );
        assert_eq!(
            completions[0].shell,
            ShellUse {
                calls: 1,
                time: Duration::from_millis(3500),
                bytes: 1024
            }
        );
        assert_eq!(
            completions[1].writes,
            Writes {
                calls: 3,
                rewrites: 2,
                empty: 1,
                shrinking: 1,
                edits: 0,
                refused: 0
            },
            "a size a many-file call leaves unknown makes the next write a rewrite, not shrinking"
        );
        assert_eq!(
            completions[1].shell,
            ShellUse {
                calls: 1,
                time: Duration::from_secs(3),
                bytes: 2048
            }
        );
        assert_eq!(
            completions[2].writes,
            Writes {
                calls: 1,
                rewrites: 0,
                empty: 1,
                shrinking: 0,
                edits: 0,
                refused: 0
            },
            "a verify's sizes start afresh, so its first write of a path is no rewrite"
        );
        assert_eq!(
            completions[2].shell,
            ShellUse {
                calls: 1,
                time: Duration::from_secs(3),
                bytes: 10
            }
        );
    }

    // An envelope carrying `sealed` is read before the chain: each seal's
    // paths less the lockfile, a lock-only seal an empty list.
    #[test]
    fn wave_seals_read_from_the_envelope() {
        let stdout = r#"{"slices": [], "sealed": [
            {"wave": 1, "commit": "aaaa", "paths": ["Cargo.lock"]},
            {"wave": 3, "commit": "cccc", "paths": ["Cargo.lock", "crates/start/src/api.rs"]}
        ]}"#;
        let mut build = build_from(0, stdout, "", Vec::new());
        build.target = Target::Omnia;
        let repairs = build.repairs();
        assert_eq!(repairs.get(&1), Some(&Vec::new()));
        assert_eq!(repairs.get(&2), None);
        assert_eq!(
            repairs.get(&3).map(Vec::as_slice),
            Some(&["crates/start/src/api.rs".to_owned()][..])
        );
    }

    #[test]
    fn tests_named_by_scenarios() {
        let text = "import { test } from \"node:test\";\n\
                    test(\"an order with no items is refused\", () => {});\n\
                    it('a paid order ships', () => {});\n\
                    subtest(\"not a test\", () => {});\n\
                    test(`a template ${name}`, () => {});\n";
        let names = declared_tests(text);
        assert_eq!(names, ["an order with no items is refused", "a paid order ships"]);
        assert_eq!(named_by("An order with no items is refused", &names), Some(Named::Exact));
        assert_eq!(named_by("a paid order ships to the address", &names), Some(Named::Fuzzy));
        assert_eq!(named_by("ships", &names), None);
        assert_eq!(named_by("an order is cancelled", &names), None);
    }

    #[test]
    fn layout_membership() {
        let stems: BTreeSet<&str> = ["orders", "start"].into_iter().collect();
        for path in [
            "package.json",
            "src/index.ts",
            "src/main.ts",
            "src/orders/index.ts",
            "src/orders/reading/package.json",
            "test/orders.test.ts",
            "test/orders/reading.test.ts",
        ] {
            assert!(Target::Node.within_layout(path, &stems), "`{path}` is within the layout");
        }
        for path in [
            "README.md",
            "scripts/check.mjs",
            "src/orders-reading/index.ts",
            "test/helpers.ts",
            "src/x.ts",
        ] {
            assert!(!Target::Node.within_layout(path, &stems), "`{path}` is outside the layout");
        }
    }

    #[test]
    fn omnia_layout_membership() {
        let stems: BTreeSet<&str> = ["orders", "start"].into_iter().collect();
        for path in [
            "Cargo.toml",
            "Cargo.lock",
            "rust-toolchain.toml",
            "clippy.toml",
            "deny.toml",
            ".gitignore",
            "src/lib.rs",
            "src/routes.rs",
            "src/messaging.rs",
            "tests/routes.rs",
            "tests/messaging.rs",
            "examples/runtime.rs",
            ".github/workflows/ci.yaml",
            "supply-chain/config.toml",
            "crates/common/src/topic.rs",
            "crates/common/src/config.rs",
            "crates/orders/Cargo.toml",
            "crates/orders/src/reading.rs",
            "crates/orders/tests/reading.rs",
            "crates/start/src/lib.rs",
        ] {
            assert!(Target::Omnia.within_layout(path, &stems), "`{path}` is within the layout");
        }
        for path in [
            "README.md",
            "src/main.rs",
            "app/src/lib.rs",
            "tests/orders.rs",
            "crates/common/src/orders.rs",
            "crates/payments/src/lib.rs",
            "scripts/check.sh",
            "package.json",
        ] {
            assert!(!Target::Omnia.within_layout(path, &stems), "`{path}` is outside the layout");
        }
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

    // Wave 2's verify answered, so its completion is `ok`: the verdict is what failed.
    #[test]
    fn build_failure_envelope() {
        let stderr = timed("build-SLICE-001", "ok", 1_000)
            + &timed("verify", "ok", 1_000)
            + &timed("build-SLICE-002", "ok", 1_000)
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
                revision: Some("feb4bd36".to_owned()),
                slice: Some("SLICE-002".to_owned()),
                wave: Some(2),
                requirements: vec!["REQ-002".to_owned(), "REQ-003".to_owned()],
                covered: vec!["REQ-002".to_owned()],
                files: vec!["src/orders/index.ts".to_owned()],
            },
            Landed {
                commit: "aaaaaaaaaaaa".to_owned(),
                subject: "SLICE-001 start".to_owned(),
                revision: Some("feb4bd36".to_owned()),
                slice: Some("SLICE-001".to_owned()),
                wave: Some(1),
                requirements: vec!["REQ-001".to_owned()],
                covered: vec!["REQ-001".to_owned()],
                files: vec!["src/index.ts".to_owned()],
            },
            Landed {
                commit: "000000000000".to_owned(),
                subject: "Initialise".to_owned(),
                revision: None,
                slice: None,
                wave: None,
                requirements: Vec::new(),
                covered: Vec::new(),
                files: vec![".gitignore".to_owned()],
            },
        ];
        let build = build_from(1, "", &stderr, history);
        assert!(!build.landed());
        assert_eq!(build.code(), Some("verify-failed"));
        assert_eq!(build.wave_failed(), Some(2));
        assert_eq!(build.waves(), 2);
        assert_eq!(build.verified(), 1);
        assert_eq!(
            build.verify_failures(),
            [
                "check 2 (tsc): src/orders/index.ts(3,1): error TS2304",
                "check 3 (npm test): not run"
            ]
        );
        let integrated = build.integrated();
        assert_eq!(integrated.len(), 2, "the root the engine laid is no slice");
        assert_eq!(integrated[0].id, "SLICE-001");
        assert_eq!(integrated[1].name, "orders");
        assert_eq!(integrated[1].uncovered(), ["REQ-003"]);
        assert_eq!(build.covered(), (2, 3));
    }

    #[test]
    fn build_merge_unresolved_wave() {
        let envelope = "{\n  \"error\": \"merge-unresolved\",\n  \"message\": \"slice `SLICE-003` \
                        (payments) failed in wave 3; SLICE-002 merged before it stays committed in \
                        `.emery/vcs/integration`: its merge conflicts at package.json, src/index.ts, \
                        and the conflict cannot be resolved: the slice removed `package.json`\",\n  \
                        \"exit-code\": 1\n}\n";
        let bare = build_from(1, "", envelope, Vec::new());
        assert_eq!(bare.code(), Some("merge-unresolved"));
        assert_eq!(bare.wave_failed(), Some(3));
        assert_eq!(bare.verify_failures(), [] as [&str; 0]);
        assert_eq!(bare.speedup(), None, "no turn read, no path to measure");
        assert_eq!(bare.verified(), 0, "no verify read, none passed");

        // SLICE-003 built in wave 2 and again in wave 3, whose verify never opened
        let stderr = completion("build-SLICE-001", "ok")
            + &completion("verify", "ok")
            + &completion("build-SLICE-002", "ok")
            + &completion("build-SLICE-003", "ok")
            + &completion("verify", "corrected")
            + &completion("build-SLICE-003", "ok")
            + envelope;
        let build = build_from(1, "", &stderr, Vec::new());
        assert_eq!(build.waves(), 3);
        assert_eq!(build.verified(), 2);
    }

    #[test]
    fn build_chain() {
        let text = "\x1ecccc\x1fWave 2 verified\x1ffeb4bd36\n\x1f\x1f\x1f\x1f\x1f\n\n\
                    package-lock.json\n\
                    \x1ebbbb\x1fSLICE-002 orders\x1ffeb4bd36\n\x1fSLICE-002\n\x1f2\n\
                    \x1fREQ-002, REQ-003\n\x1fREQ-002\n\x1f\n\nsrc/index.ts\nsrc/orders/index.ts\n\
                    \x1eaaaa\x1fInitialise\x1f\x1f\x1f\x1f\x1f\x1f\n\n.gitignore\n";
        let chain = chain(text);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].slice, None);
        assert_eq!(chain[0].revision.as_deref(), Some("feb4bd36"), "a seal is under the revision");
        assert_eq!(chain[0].files, ["package-lock.json"]);
        assert_eq!(chain[1].slice.as_deref(), Some("SLICE-002"));
        assert_eq!(chain[1].revision.as_deref(), Some("feb4bd36"));
        assert_eq!(chain[1].wave, Some(2));
        assert_eq!(chain[1].requirements, ["REQ-002", "REQ-003"]);
        assert_eq!(chain[1].covered, ["REQ-002"]);
        assert_eq!(chain[1].files, ["src/index.ts", "src/orders/index.ts"]);
        assert_eq!(chain[2].subject, "Initialise");
        assert_eq!(chain[2].revision, None, "the root is under no revision");
        assert_eq!(chain[2].files, [".gitignore"]);
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

    #[test]
    fn reference_path() {
        let one = "[[source]]\nadapter = \"typescript\"\nfixture = \"f\"\n";
        let named = case(&format!("reference = \"evals/cases/train/reference\"\n{one}"))
            .expect("a reference relative to the root");
        assert_eq!(named.reference.as_deref(), Some("evals/cases/train/reference"));
        assert_eq!(case(one).unwrap().reference, None, "a case need name no reference");

        let absolute = refused(&format!("reference = \"/tmp/reference\"\n{one}"));
        assert!(absolute.contains("not a path relative to the repository root"), "{absolute}");
        let empty = refused(&format!("reference = \"\"\n{one}"));
        assert!(empty.contains("not a path relative"), "{empty}");
    }

    // Every committed case parses under the shape, with every item at a
    // source of its case, and every committed fixture and reference tree
    // passes the credential gate.
    #[test]
    fn committed_cases_parse() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut seen = 0;
        let mut checked = BTreeSet::new();
        for entry in fs::read_dir(root.join(CASES)).unwrap() {
            let dir = entry.unwrap().path();
            let Ok(text) = fs::read_to_string(dir.join("expected.toml")) else { continue };
            let expected = case(&text).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
            for tree in expected.trees() {
                let path = root.join(tree);
                if path.is_dir() && checked.insert(tree.to_owned()) {
                    check_tree(&path, tree)
                        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
                }
            }
            seen += 1;
        }
        assert!(seen > 0, "the cases are beside the runner");
        check_tree(&root.join("examples"), "examples")
            .unwrap_or_else(|error| panic!("examples: {error}"));
    }

    #[test]
    fn fixture_credentials() {
        assert_eq!(named_secret(".env"), Some("a file that carries a credential by name"));
        assert_eq!(
            named_secret("config/.env.production"),
            Some("a file that carries a credential by name")
        );
        assert_eq!(named_secret(".env.example"), None);
        assert_eq!(named_secret(".npmrc"), None);

        let nr_populated =
            br#"{"new_relic_license_key":{"all":"0123456789abcdef0123456789abcdef01234567"}}"#;
        assert_eq!(
            carried_secret("azure/parameters.json", nr_populated).as_deref(),
            Some("parameter `new_relic_license_key` carries a credential literal")
        );
        let nr_empty = br#"{"new_relic_license_key":{"all":"","vnet-prod":""},"key_vault_secret_name_confluent_key":{"all":"realtime-confluent-key"}}"#;
        assert_eq!(carried_secret("azure/parameters.json", nr_empty), None);

        let storage_key = format!("\"{}a+b/==\"", "A".repeat(82));
        assert_eq!(
            carried_secret("config.json", storage_key.as_bytes()).as_deref(),
            Some("an Azure Storage account key")
        );
        let lockfile_hash = format!("\"integrity\": \"sha512-{}a+b/==\"", "A".repeat(82));
        assert_eq!(carried_secret("package-lock.json", lockfile_hash.as_bytes()), None);
    }

    fn spec(requirements: &Value) -> Value {
        serde_json::json!({ "requirements": requirements })
    }

    fn cited(source: &str, claim: &str, path: &str) -> Value {
        serde_json::json!({ "source": source, "claim": claim, "path": path })
    }

    // The figure reads each body's spans, in backticks or bare, by shape
    // alone, and leaves out the names the design owns a `data` type for; a
    // `mechanism` type's name is the platform's as much as a decorator is.
    #[test]
    fn mechanism_figure() {
        let spec = spec(&serde_json::json!([
            { "id": "REQ-001", "body": ["Requests pass `AuthGuard('jwt')` and read `request.user.id`."],
              "sources": [cited("docs", "auth.guard", "auth.md#L10")] },
            { "id": "REQ-002", "body": ["`RolesGuard` reads `user.role` against the `Roles` metadata."],
              "sources": [cited("code", "roles.guard", "roles.ts#L5")] },
            { "id": "REQ-003", "body": ["Set `AUTH_JWT_SECRET` in `.env`;", "`POST /api/v1/auth/login` answers `201` with `refreshToken` and `tokenExpires`."],
              "sources": [cited("docs", "auth.login", "auth.md#L20")] },
            { "id": "REQ-004", "body": ["`@Exclude({ toPlainOnly: true })` hides `password`; `UserMapper.toDomain()` maps the row."],
              "sources": [cited("code", "users.mapper", "users.ts#L8"), cited("docs", "users.mapper", "users.md#L3")] },
            { "id": "REQ-005", "body": ["The `Session` entity, a `JWT`, `user.mapper.ts`, `S3`, the token(s), and `x-custom-lang`."],
              "sources": [cited("docs", "session", "session.md#L1")] },
            { "id": "REQ-006", "body": ["Local upload accepts one multipart field via FileInterceptor, keyed by a Nest randomStringGenerator value (e.g. file.awsDefaultS3Bucket) on TypeORM; see validate()."],
              "sources": [cited("code", "files.local", "files.ts#L20")] },
        ]));
        let design = serde_json::json!({ "sections": [{ "kind": "domain-model", "blocks": [
            { "type": { "key": "RolesGuard", "signature": "class RolesGuard {}", "role": "mechanism" } },
            { "type": { "key": "UserMapper (src/users/user.mapper.ts#L8-L72)", "signature": "class UserMapper {}", "role": "data" } },
            { "type": { "key": "Session", "signature": "interface Session {}" } },
        ] }] });
        let mut run = run_from(Some(0), "", Some(spec));
        run.design = Some(design);

        assert_eq!(
            run.design_types(),
            BTreeSet::from(["Session", "UserMapper"]),
            "a `mechanism` type is no part of the program's vocabulary; a block without a role \
             is read as data"
        );
        let stated = run.mechanism();
        let listed: Vec<(&str, Vec<&str>, Vec<&str>)> = stated
            .iter()
            .map(|stated| {
                (
                    stated.id.as_str(),
                    stated.sources.iter().map(String::as_str).collect(),
                    stated.identifiers.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            vec![
                ("REQ-001", vec!["docs"], vec!["AuthGuard('jwt')"]),
                ("REQ-002", vec!["code"], vec!["RolesGuard"]),
                ("REQ-004", vec!["code", "docs"], vec!["@Exclude({ toPlainOnly: true })"]),
                ("REQ-006", vec!["code"], vec!["FileInterceptor", "TypeORM", "validate()"]),
            ],
            "the design's `UserMapper.toDomain()` left out and its mechanism `RolesGuard` \
             counted, with the program's own fields, keys, and functions left out"
        );
        assert_eq!((run.citing("docs"), run.citing("code")), (4, 3));
        for span in [
            "Authorization",
            "JWTs",
            "T",
            "sessionId (id)",
            "'true' | 'false'",
            "30d",
            "token(s)",
            "e.g.",
            "api.example.com",
        ] {
            assert_eq!(identifier(span), None, "{span}");
        }
        assert_eq!(identifier("I18nContext.current()"), Some("I18nContext"));
        assert_eq!(identifier("redis.ping()"), Some("redis"));
        assert_eq!(identifier("createApp(config)"), Some("createApp"));
        assert_eq!(identifier("@Injectable"), Some("Injectable"));
    }

    // The lexicon is the engine's: the fixed names and the packages the
    // `call` claims spell, so a term of the run's own (`acme`) is a leak as
    // a fixed name is, and a `call` naming one is where it belongs.
    #[test]
    fn technology_leaks() {
        let stderr = accepted_line(
            "code",
            &[
                r#"{"kind":"requirement","id":"start.credentials-before-clients","path":"src/start.ts#L8-L10","statement":"Broker credentials are obtained before the message clients are constructed."}"#,
                r#"{"kind":"requirement","id":"start.confluent-secret","path":"src/start.ts#L8-L10","statement":"Credentials are read from Key Vault before the clients start."}"#,
                r#"{"kind":"criterion","id":"start.vault-host","path":"src/config.ts#L6","criterion":"KEYVAULT_HOST names the vault host."}"#,
                r#"{"kind":"call","path":"src/orders.ts#L20","callee":"@acme/order-bus:Bus.publish"}"#,
                r#"{"kind":"requirement","id":"orders.publish","path":"src/orders.ts#L20","statement":"Each placed order is sent through the acme bus."}"#,
            ]
            .join(","),
        );
        let spec = serde_json::json!({
            "preamble": ["The service publishes order events to Kafka.", "Credentials rotate."],
            "requirements": [{
                "id": "REQ-001", "subject": "orders.publish", "body": ["Each placed order is published."],
                "sources": [cited("code", "orders.publish", "src/orders.ts#L20")],
                "scenarios": [{
                    "name": "An order is published to Confluent",
                    "given": ["the Azure credentials are held"],
                    "when": "an order is placed",
                    "then": "the event is on the outbound stream",
                    "and": ["the storefront receives the id"]
                }]
            }]
        });
        let run = run_from(Some(0), &stderr, Some(spec));

        assert_eq!(run.lexicon(), Lexicon::with(["acme", "orderbus"]));
        assert_eq!(
            run.leaks(),
            [
                "code:`start.confluent-secret` id names `confluent`",
                "code:`start.confluent-secret` statement names `keyvault`",
                "code:`orders.publish` statement names `acme`",
                "code:`start.vault-host` criterion names `keyvault`",
                "preamble paragraph 1 names `kafka`",
                "`orders.publish` scenario `An order is published to Confluent` name names \
                 `confluent`",
                "`orders.publish` scenario `An order is published to Confluent` given names `azure`",
            ]
        );
        assert_eq!(run_from(Some(0), "", None).leaks(), Vec::<String>::new());
    }

    // An absent span is graded by `held`, as a behaviour's claims are: a
    // requirement half or more within it is a hit, one a line wider than it
    // is not, and a span naming no lines is hit by any requirement in its
    // file. A criterion anchored within it is no hit, since the figure
    // reads what the specification states as requirements.
    #[test]
    fn absent_hits() {
        let expected = case(
            "[[source]]\nname = \"code\"\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[source]]\nname = \"docs\"\nadapter = \"documentation\"\nfixture = \"d\"\n\n\
             [[requirement]]\nsource = \"code\"\nstem = \"orders\"\nanchor = \"src/orders.ts#L10-L20\"\ngloss = \"an order is placed\"\n\n\
             [[absent]]\nsource = \"code\"\nanchor = \"src/start.ts#L21-L34\"\ngloss = \"the credentials are loaded before serving\"\n\n\
             [[absent]]\nsource = \"code\"\nanchor = \"src/secrets.ts\"\ngloss = \"the secret retriever\"\n\n\
             [[absent]]\nsource = \"docs\"\nanchor = \"ops.md#L5-L9\"\ngloss = \"the restart procedure\"\n",
        )
        .unwrap();
        let stderr = accepted_line(
            "code",
            &[
                requirement("orders.place", "src/orders.ts#L10-L20"),
                requirement("start.load-credentials", "src/start.ts#L21-L30"),
                requirement("start.serve", "src/start.ts#L1-L60"),
                requirement("start.token", "src/secrets.ts#L8-L12"),
                criterion("start.rotation-interval", "src/start.ts#L25"),
            ]
            .join(","),
        ) + &accepted_line("docs", &requirement("ops.restart", "ops.md#L5-L9"));
        let run = run_from(Some(0), &stderr, None);

        let graded = absent(&expected.absent, &expected, &run);
        assert_eq!(graded.expected, 3);
        assert_eq!(
            graded.hits,
            vec![
                "code:`src/start.ts#L21-L34` — the credentials are loaded before serving \
                 (`start.load-credentials` at `src/start.ts#L21-L30`)",
                "code:`src/secrets.ts` — the secret retriever (`start.token` at \
                 `src/secrets.ts#L8-L12`)",
                "docs:`ops.md#L5-L9` — the restart procedure (`ops.restart` at `ops.md#L5-L9`)",
            ],
            "the whole-file requirement over `start.ts` and the criterion are no hits"
        );
        assert_eq!(grade(&expected, &run).absent, graded);

        let stemmed = refused(
            "[[source]]\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[absent]]\nstem = \"start\"\nanchor = \"src/start.ts\"\ngloss = \"g\"\n",
        );
        assert!(stemmed.contains("names a stem"), "{stemmed}");
        let stranger = refused(
            "[[source]]\nadapter = \"typescript\"\nfixture = \"c\"\n\n\
             [[absent]]\nsource = \"docs\"\nanchor = \"ops.md\"\ngloss = \"g\"\n",
        );
        assert!(stranger.contains("absent `ops.md` names no source of the case"), "{stranger}");
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
