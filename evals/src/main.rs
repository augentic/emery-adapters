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

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Display, Formatter, Write as _};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, fs, io};

mod build;
mod card;
mod case;
mod fidelity;
mod grade;
mod reference;
mod run;
mod stage;

use self::build::{oracle, rebuilt, resumed};
use self::card::scorecard;
use self::case::{CASES, Case, cases};
use self::fidelity::Target;
use self::grade::{Attempt, Report, grade};
use self::reference::Reference;
use self::run::{Run, facts, run};
use self::stage::stage;

const LADDER: &str = "600/120,1200/240,2400/480";
// A build's turn writes a slice and repairs its findings, so it runs at the
// ladder's second rung from the start rather than losing a slice to the
// first.
const BUILD_RUNG: &str = "1200/240";

// The run's log filter unless the caller sets one: every adapter the case
// runs under at trace beside the SDK's, and, for either backend, its
// completion and tool-call lines and the tool-call arguments whole, which
// it logs under a target of their own. The crate is `emery-<kind>`, so the
// tracing target is `emery_<kind>`.
fn rust_log<'a>(adapters: impl IntoIterator<Item = &'a str>) -> String {
    let mut filter = "emery_sdk=trace".to_owned();
    for adapter in adapters {
        let _ = write!(filter, ",emery_{adapter}=trace");
    }
    filter
        + ",omnia_cursor=info,omnia_cursor::tools=trace,omnia_genai=debug,omnia_genai::tools=trace,\
           omnia_core=off"
}

// The backend the binary answers the model through, read from its file
// name: `emery-dev` is the genai runtime, anything else the shipped Cursor
// one. Each names its own model and budget variables, which the runner
// sets from the rung and the card records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Backend {
    Cursor,
    GenAi,
}

impl Backend {
    fn of(emery: &Path) -> Self {
        if emery.file_stem().is_some_and(|stem| stem == "emery-dev") {
            Self::GenAi
        } else {
            Self::Cursor
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Cursor => "cursor",
            Self::GenAi => "genai",
        }
    }

    // The variable naming the model, and the model the backend runs when
    // it is unset.
    const fn model(self) -> (&'static str, &'static str) {
        match self {
            Self::Cursor => ("CURSOR_MODEL", "auto"),
            Self::GenAi => ("GENAI_MODEL", "gpt-5.5"),
        }
    }

    // The pair a rung sets: the cap on one completion, then the cut-off on
    // its silence — Cursor's inactivity, which exempts a running shell;
    // genai's cap on one shell command, since it has no other reading of
    // silence — so a build rung's second figure is the longest `cargo`
    // step under genai, and the card's figure to raise where a cold compile
    // outruns it.
    const fn rung(self) -> (&'static str, &'static str) {
        match self {
            Self::Cursor => ("CURSOR_TIMEOUT_SECS", "CURSOR_INACTIVITY_SECS"),
            Self::GenAi => ("GENAI_TIMEOUT_SECS", "GENAI_SHELL_TIMEOUT_SECS"),
        }
    }

    // The budget variables the runner inherits and the card names where
    // the caller set them.
    const fn knobs(self) -> &'static [&'static str] {
        match self {
            Self::Cursor => &["CURSOR_MAX_TOOL_CALLS"],
            Self::GenAi => &[
                "GENAI_MAX_TOOL_CALLS",
                "GENAI_MAX_ROUNDS",
                "GENAI_MAX_CANDIDATES",
                "GENAI_MAX_REPEATS",
                "GENAI_MAX_TOKENS",
            ],
        }
    }
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
// mode (`--facts`, `--build`, `--rebuild <card>`, `--resume <card> --at
// <wave>` with `--drifted` to admit a seed the template has moved under,
// `--oracle <card>`) with the target a build goes through.
struct Args {
    filter: BTreeSet<String>,
    facts_only: bool,
    again: Option<Again>,
    // the card whose `omnia` build the reference's fixtures are driven through
    oracle: Option<String>,
    // the target a build runs through; none where no build runs
    target: Option<Target>,
}

// A build of a card's committed plans again: fresh, or on from a wave of the
// chain the card's build left. A resume reads the component's template
// against the seed it resumes before the put and refuses one the template
// has moved under, unless `drifted` admits it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Again {
    Rebuild { card: String },
    Resume { card: String, wave: usize, drifted: bool },
}

impl Again {
    // One case's plan built again under the new card, as this mode has it.
    fn run(
        &self, eval_dir: &Path, card_name: &str, case: &Case, settings: &Settings,
    ) -> io::Result<Run> {
        match self {
            Self::Rebuild { card } => {
                eprintln!("eval: case `{}` rebuilt from `{card}`", case.name);
                rebuilt(eval_dir, card, card_name, case, settings)
            }
            Self::Resume { card, wave, drifted } => {
                eprintln!("eval: case `{}` resumed from `{card}` at wave {wave}", case.name);
                resumed(eval_dir, card, *wave, *drifted, card_name, case, settings)
            }
        }
    }
}

impl Args {
    fn parse(argv: impl Iterator<Item = String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut argv = argv;
        let mut filter: BTreeSet<String> = BTreeSet::new();
        let mut rebuild: Option<String> = None;
        let mut resume: Option<String> = None;
        let mut at: Option<usize> = None;
        let mut oracle: Option<String> = None;
        let mut target: Option<Target> = None;
        while let Some(arg) = argv.next() {
            if arg == "--rebuild" {
                rebuild = Some(argv.next().ok_or("`--rebuild` names a card")?);
            } else if arg == "--resume" {
                resume = Some(argv.next().ok_or("`--resume` names a card")?);
            } else if arg == "--at" {
                let wave = argv.next().ok_or("`--at` names a wave")?;
                at = Some(wave.parse().map_err(|error| format!("`--at {wave}`: {error}"))?);
            } else if arg == "--oracle" {
                oracle = Some(argv.next().ok_or("`--oracle` names a card")?);
            } else if arg == "--target" {
                let name = argv.next().ok_or("`--target` names a target adapter")?;
                target = Some(Target::parse(&name)?);
            } else {
                filter.insert(arg);
            }
        }
        let again = match (rebuild, resume, at) {
            (Some(_), Some(_), _) => {
                return Err(
                    "`--rebuild` and `--resume` each build a card's plans again; name one".into()
                );
            }
            (Some(card), None, None) => Some(Again::Rebuild { card }),
            (_, None, Some(_)) => {
                return Err("`--at` names the wave a `--resume` resumes at".into());
            }
            (None, Some(card), Some(wave)) => {
                if wave < 2 {
                    return Err(
                        "`--at` names a wave past the first; a resume at wave 1 is a `--rebuild`"
                            .into(),
                    );
                }
                let drifted = filter.remove("--drifted");
                Some(Again::Resume { card, wave, drifted })
            }
            (None, Some(_), None) => {
                return Err("`--resume` names the wave to resume at through `--at`".into());
            }
            (None, None, None) => None,
        };
        if filter.remove("--drifted") {
            return Err("`--drifted` admits the seed a `--resume` resumes over".into());
        }
        let facts_only = filter.remove("--facts");
        let build = filter.remove("--build") || again.is_some();
        if facts_only && filter.is_empty() {
            return Err("`--facts` names a case or an adapter".into());
        }
        if facts_only && build {
            return Err("`--facts` spends no turn, so it cannot `--build`".into());
        }
        if again.is_some() && filter.is_empty() {
            return Err("`--rebuild` and `--resume` name a case or an adapter".into());
        }
        if oracle.is_some() {
            if filter.is_empty() {
                return Err("`--oracle` names a case or an adapter".into());
            }
            if facts_only || build {
                return Err(
                    "`--oracle` spends no turn and builds nothing, so it cannot `--facts`, \
                            `--build`, `--rebuild`, or `--resume`"
                        .into(),
                );
            }
            if target.is_some_and(|target| target != Target::Omnia) {
                return Err("`--oracle` reads an `omnia` build; `--target` names another".into());
            }
        } else if target.is_some() && !build {
            return Err(
                "`--target` chooses what `--build`, `--rebuild`, or `--resume` builds through"
                    .into(),
            );
        }
        Ok(Self {
            filter,
            facts_only,
            again,
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
        again,
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
    settings.again = again;

    if let Some(again) = settings.again.clone() {
        let started = now();
        let card_name = started.replace(':', "-");
        let mut reports = Vec::with_capacity(cases.len());
        for case in &cases {
            let run = again.run(&eval_dir, &card_name, case, &settings)?;
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
    backend: Backend,
    model: String,
    // the caller's `RUST_LOG`, when set, over the per-case default
    rust_log: Option<String>,
    // the backend's budget variables the caller set; the runner inherits
    // them, the card names them
    knobs: Vec<(String, String)>,

    // the card whose committed plans this invocation builds again, and how
    again: Option<Again>,
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
        let backend = Backend::of(&emery);
        let (model, default) = backend.model();
        let knobs = backend
            .knobs()
            .iter()
            .filter_map(|knob| Some(((*knob).to_owned(), env::var(knob).ok()?)))
            .collect();

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
            backend,
            model: env::var(model).unwrap_or_else(|_| default.to_owned()),
            rust_log: env::var("RUST_LOG").ok(),
            knobs,
            again: None,
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

// Whether a run may put a turn to the model: `None` strips `CURSOR_API_KEY`
// from the binary's environment, or points genai's `GENAI_ENDPOINT` at a
// port nothing answers on, so the backend refuses the first turn after the
// adapters have read their trees and logged their facts.
#[derive(Clone, Copy)]
enum Turns {
    Live,
    None,
}

// Where a `Turns::None` run under genai sends its first request: the
// discard port of the loopback, so the connection is refused whichever
// provider the model id routes to, a local server that needs no key included.
const NOWHERE: &str = "http://127.0.0.1:9";

#[expect(clippy::disallowed_types, reason = "the eval runs the shipped `emery` binary natively")]
fn emery(
    project: &Path, settings: &Settings, rust_log: &str, rung: Rung, args: &[&str], turns: Turns,
) -> io::Result<std::process::Output> {
    let mut command = std::process::Command::new(&settings.emery);
    let (timeout, inactivity) = settings.backend.rung();
    command
        .arg("--format")
        .arg("json")
        .args(args)
        .current_dir(project)
        .env("NO_COLOR", "1")
        .env("RUST_LOG", rust_log)
        .env(timeout, rung.timeout.to_string())
        .env(inactivity, rung.inactivity.to_string());
    if matches!(turns, Turns::None) {
        match settings.backend {
            Backend::Cursor => command.env_remove("CURSOR_API_KEY"),
            Backend::GenAi => command.env("GENAI_ENDPOINT", NOWHERE),
        };
    }
    command.output()
}

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
    use std::num::NonZeroU32;
    use std::time::Duration;

    use emery_sdk::{ClaimKind, Lexicon};
    use serde_json::Value;

    use super::*;
    use crate::build::{
        Build, Integrated, Landed, Rewound, Tree, chain, empty_before, failure, history, log,
    };
    use crate::card::factor;
    use crate::case::{Expected, carried_secret, check_tree, named_secret};
    use crate::fidelity::{
        Fidelity, HostCheck, Named, Route, Scenario, cargo_test_counts, declared_rust_tests,
        declared_tests, named_by, reexported, routes, scenarios_of, tap_counts, unrun,
    };
    use crate::grade::{absent, behaviours, identifier};
    use crate::run::{Landing, ShellUse, Writes, accepted, completions, surveyed, unreached};
    use crate::stage::{clone_root, git, repository};

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
            rewound: None,
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
    fn resume_args() {
        let args = Args::parse(
            ["--resume", "card", "--at", "3", "--target", "omnia", "express-orders"]
                .map(String::from)
                .into_iter(),
        )
        .unwrap();
        assert_eq!(
            args.again,
            Some(Again::Resume {
                card: "card".to_owned(),
                wave: 3,
                drifted: false
            })
        );
        assert_eq!(args.target, Some(Target::Omnia), "a resume builds");
        let drifted = Args::parse(
            ["--resume", "card", "--at", "2", "--drifted", "express-orders"]
                .map(String::from)
                .into_iter(),
        )
        .unwrap();
        assert_eq!(
            drifted.again,
            Some(Again::Resume {
                card: "card".to_owned(),
                wave: 2,
                drifted: true
            })
        );
        assert_eq!(drifted.filter, BTreeSet::from(["express-orders".to_owned()]));
        let rebuild =
            Args::parse(["--rebuild", "card", "express-orders"].map(String::from).into_iter())
                .unwrap();
        assert_eq!(
            rebuild.again,
            Some(Again::Rebuild {
                card: "card".to_owned()
            })
        );
        for argv in [
            &["--resume", "card", "express-orders"][..],
            &["--resume", "card", "--at", "1", "express-orders"][..],
            &["--resume", "card", "--at", "two", "express-orders"][..],
            &["--resume", "card", "--at", "2"][..],
            &["--rebuild", "card", "--at", "2", "express-orders"][..],
            &["--at", "2", "express-orders"][..],
            &["--rebuild", "card", "--resume", "card", "--at", "2", "express-orders"][..],
            &["--resume", "card", "--at", "2", "--facts", "express-orders"][..],
            &["--rebuild", "card", "--drifted", "express-orders"][..],
            &["--build", "--drifted", "express-orders"][..],
        ] {
            assert!(Args::parse(argv.iter().map(|arg| (*arg).to_owned())).is_err(), "{argv:?}");
        }
    }

    // A resumed build's chain holds the slices it resumed beside the ones it
    // built; the counts tell them apart.
    #[test]
    fn resumed_build_counts() {
        let stderr = timed("build-SLICE-004", "ok", 100_000) + &timed("verify", "ok", 20_000);
        let history = vec![
            Landed {
                commit: "444444444444".to_owned(),
                subject: "SLICE-004 customers".to_owned(),
                revision: Some("rev".to_owned()),
                slice: Some("SLICE-004".to_owned()),
                wave: Some(3),
                requirements: vec!["REQ-004".to_owned()],
                covered: vec!["REQ-004".to_owned()],
                files: vec!["crates/customers/src/lib.rs".to_owned()],
            },
            Landed {
                commit: "333333333333".to_owned(),
                subject: "SLICE-003 health".to_owned(),
                revision: Some("rev".to_owned()),
                slice: Some("SLICE-003".to_owned()),
                wave: Some(2),
                requirements: vec!["REQ-003".to_owned()],
                covered: vec!["REQ-003".to_owned()],
                files: vec!["crates/health/src/lib.rs".to_owned()],
            },
            Landed {
                commit: "111111111111".to_owned(),
                subject: "Wave 1 verified".to_owned(),
                revision: Some("rev".to_owned()),
                slice: None,
                wave: None,
                requirements: Vec::new(),
                covered: Vec::new(),
                files: vec!["Cargo.lock".to_owned()],
            },
        ];
        let mut build = build_from(1, "", &stderr, history);
        assert_eq!(build.resumption(), "", "a build from the root resumed nothing");
        build.rewound = Some(Rewound {
            card: "card".to_owned(),
            wave: 3,
            commit: "333333333333".to_owned(),
            subject: "SLICE-003 health".to_owned(),
            slices: vec!["SLICE-003".to_owned()],
        });
        assert_eq!(build.integrated().len(), 2);
        let this_run = build.built();
        assert_eq!(this_run.len(), 1);
        assert_eq!(this_run[0].id, "SLICE-004");
        assert_eq!(build.resumption(), ", 1 resumed, 1 built");
    }

    // A resume seals the earlier waves' empty slices but the first, so the
    // slices sealed are the empty ones before the wave in wave order less
    // one, none at the second wave, and the message is the engine's own.
    #[test]
    fn resumed_empty_waves_sealed() {
        let envelope = serde_json::json!({
            "slices": [
                {"id": "SLICE-001", "name": "start", "wave": 1, "covered": [],
                 "uncovered": ["REQ-001", "REQ-002"], "written": [], "commit": null},
                {"id": "SLICE-003", "name": "health", "wave": 2, "covered": ["REQ-003"],
                 "uncovered": [], "written": ["crates/health/src/health.rs"], "commit": "3333"},
                {"id": "SLICE-002", "name": "start-prefetch", "wave": 3, "covered": [],
                 "uncovered": ["REQ-004"], "written": [], "commit": null},
                {"id": "SLICE-005", "name": "orders-reading", "wave": 3, "covered": [],
                 "uncovered": ["REQ-005"], "written": [], "commit": null},
                {"id": "SLICE-004", "name": "customers", "wave": 4, "covered": [],
                 "uncovered": ["REQ-006"], "written": [], "commit": null},
            ]
        });
        let ids = |wave: usize| -> Vec<String> {
            empty_before(&envelope, wave).into_iter().map(|slice| slice.id).collect()
        };
        assert_eq!(ids(4), ["SLICE-001", "SLICE-002", "SLICE-005"]);
        assert_eq!(ids(2), ["SLICE-001"], "the second wave's resume leaves its one seed verify");
        let sealed = empty_before(&envelope, 4).swap_remove(1);
        assert_eq!(
            sealed.message("rev", "1111"),
            "SLICE-002 start-prefetch\n\nSlice: SLICE-002\nRevision: rev\nRequirements: \
             REQ-004\nCovered: \nAdapter: target\nBase: 1111\nWave: 3"
        );
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

    // Both backends' completion and tool-call lines are in the default
    // filter, so one runner reads a run under `emery` or `emery-dev`.
    #[test]
    fn rust_log_reads_both_backends() {
        let filter = rust_log(["typescript"]);
        let directives: Vec<&str> = filter.split(',').collect();
        assert!(directives.contains(&"omnia_cursor=info"));
        assert!(directives.contains(&"omnia_cursor::tools=trace"));
        assert!(directives.contains(&"omnia_genai=debug"));
        assert!(directives.contains(&"omnia_genai::tools=trace"));
        assert!(directives.contains(&"omnia_core=off"));
    }

    #[test]
    fn backend_of_binary() {
        assert_eq!(Backend::of(Path::new("../emery/target/release/emery")), Backend::Cursor);
        assert_eq!(Backend::of(Path::new("../emery/target/release/emery-dev")), Backend::GenAi);
        assert_eq!(Backend::of(Path::new("/opt/bin/emery-dev.exe")), Backend::GenAi);
        assert_eq!(Backend::Cursor.rung(), ("CURSOR_TIMEOUT_SECS", "CURSOR_INACTIVITY_SECS"));
        assert_eq!(Backend::GenAi.rung(), ("GENAI_TIMEOUT_SECS", "GENAI_SHELL_TIMEOUT_SECS"));
        assert_eq!(Backend::Cursor.model(), ("CURSOR_MODEL", "auto"));
        assert_eq!(Backend::GenAi.model(), ("GENAI_MODEL", "gpt-5.5"));
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

    // The scenarios of a requirement a merged slice left uncovered are not
    // counted, since the adapter has it keep its ignored test; those of a
    // requirement no merged slice holds are not either; and with nothing
    // merged every requirement's are.
    #[test]
    fn scenarios_counted_over_covered_requirements() {
        let slice = |name: &str, requirements: &[&str], covered: &[&str]| Integrated {
            id: format!("SLICE-{name}"),
            name: name.to_owned(),
            wave: 1,
            requirements: requirements.iter().map(|id| (*id).to_owned()).collect(),
            covered: covered.iter().map(|id| (*id).to_owned()).collect(),
            files: Vec::new(),
            commit: None,
            resolved: Vec::new(),
        };
        let integrated = [
            slice("start", &["REQ-001", "REQ-002"], &["REQ-002"]),
            slice("orders", &["REQ-003"], &["REQ-003"]),
        ];
        let spec = serde_json::json!([
            {"id": "REQ-001", "subject": "start.connect-brokers",
             "scenarios": [{"name": "The brokers are connected", "then": "[unknown]"}]},
            {"id": "REQ-002", "subject": "start.read-settings",
             "scenarios": [{"name": "The settings are read", "then": "they are held"}]},
            {"id": "REQ-003", "subject": "orders.read",
             "scenarios": [
                 {"name": "An order is answered", "then": "200"},
                 {"name": "A missing order is refused", "then": "404"},
             ]},
            {"id": "REQ-004", "subject": "payments.charge",
             "scenarios": [{"name": "A card is charged", "then": "200"}]},
        ]);
        let names = vec!["the settings are read".to_owned(), "an order is answered".to_owned()];

        let counted = scenarios_of(spec.as_array().unwrap(), &integrated, &names);
        let stems: Vec<&str> = counted.iter().map(|scenario| scenario.stem.as_str()).collect();
        assert_eq!(stems, ["start", "orders", "orders"]);
        let found: Vec<Option<Named>> = counted.iter().map(|scenario| scenario.named).collect();
        assert_eq!(found, [Some(Named::Exact), Some(Named::Exact), None]);
        assert!(counted.iter().all(|scenario| !scenario.unknown));

        let every = scenarios_of(spec.as_array().unwrap(), &[], &names);
        assert_eq!(every.len(), 5, "nothing merged counts every requirement's scenarios");
        assert_eq!(every.iter().filter(|scenario| scenario.unknown).count(), 1);
    }

    // A scratch tree of the test's own under the host's temp dir, written from
    // `(path, text)` pairs and removed first where an earlier run left it.
    #[expect(
        clippy::disallowed_methods,
        reason = "a unit test writes its tree under the host's temp dir"
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
    // spelling of its name, has no requirement carrying a typed route or
    // stating a method and path in its body or scenarios, and a module no
    // merged slice owns is no probe.
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
                    "pub(crate) const ORDERS: &str = \"/orders\";\n\
                     pub(crate) const ORDER_NOTES: &str = \"/orders/{id}/notes\";\n\n\
                     pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                     axum::Router::new().route(ORDERS, place_order_route()).route(ORDER_NOTES, notes_route())\n}\n\n\
                     pub(crate) fn topics<P: Capabilities>(router: messaging::Router<P>, env: &str) -> messaging::Router<P> {\n    \
                     router.route(config::topic_for(env, topic::ORDER_PLACED), messaging::consume(on_placed))\n}\n\n\
                     pub(crate) fn place_order_route<P: Capabilities>() -> MethodRouter<Client<P>> {\n    \
                     handle_with(\n        MethodFilter::POST,\n        place_order,\n        \
                     |raw: RawRequest<'_>| -> Result<PlaceOrderInput, DecodeError> {\n            \
                     decode_json(raw.path_params, raw.body)\n        },\n        \
                     |output: PlaceOrderOutput| (StatusCode::CREATED, Json(output)).into_response(),\n    )\n}\n",
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
             "trigger": {"http": {"method": "POST", "path": "/orders"}},
             "scenarios": [{"when": "an order is placed with valid fields", "then": "201"}]},
            {"id": "REQ-003", "body": ["Controllers never import repositories directly."], "trigger": null},
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
            ("crates/orders/src/placing.rs", "POST", "/orders", false),
            ("crates/orders/src/placing.rs", "?", "/orders/{id}/notes", false),
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

    // The genai backend's lines carry the same messages under its own
    // targets, `omnia_genai::tools` for a call and `omnia_genai::observe`
    // for the completion, with `exit=` and `duration_ms=` after the result's
    // size and the phase split after the duration; the reader keys on the
    // message, so it reads them as it reads Cursor's.
    #[test]
    fn genai_completions_read_by_message() {
        let log = "\
2026-10-19T10:00:00.000000Z DEBUG complete{n=1 model=\"openai::qwen3-coder\" format=schema label=\"build-SLICE-001\"}: omnia_genai::observe: completion started prompt_bytes=81234 tools=5
2026-10-19T10:00:01.000000Z DEBUG complete{n=1 model=\"openai::qwen3-coder\" format=schema label=\"build-SLICE-001\"}: omnia_genai::tools: tool call tool=shell subject=\"cargo clippy -p orders --all-targets -- -D warnings && cargo test -p orders\"
2026-10-19T10:00:02.000000Z DEBUG emery_sdk::target::write: responded turn=SLICE-001 tool=\"edit_files\" edited=Some([\"crates/orders/src/reading.rs\"]) error=None
2026-10-19T10:01:31.000000Z DEBUG complete{n=1 model=\"openai::qwen3-coder\" format=schema label=\"build-SLICE-001\"}: omnia_genai::tools: tool call completed tool=shell subject=\"cargo clippy -p orders --all-targets -- -D warnings && cargo test -p orders\" result_bytes=4096 exit=\"0\" duration_ms=90000
2026-10-19T10:01:32.000000Z DEBUG complete{n=1 model=\"openai::qwen3-coder\" format=schema label=\"build-SLICE-001\"}: omnia_genai::tools: tool call tool=read_doc subject=\"references/testing.md\"
2026-10-19T10:01:33.000000Z DEBUG complete{n=1 model=\"openai::qwen3-coder\" format=schema label=\"build-SLICE-001\"}: omnia_genai::tools: tool call completed tool=read_doc subject=\"references/testing.md\" result_bytes=9000 duration_ms=3
2026-10-19T10:02:00.000000Z  INFO complete{n=1 model=\"openai::qwen3-coder\" format=schema label=\"build-SLICE-001\"}: omnia_genai::observe: completion outcome=\"corrected\" attempts=2 duration_ms=120000 model_ms=25000 tool_ms=90003 check_ms=4997 input_tokens=90000 cache_read_tokens=0 cache_write_tokens=0 output_tokens=7000 reasoning_tokens=0 total_tokens=97000
2026-10-19T10:03:00.000000Z  WARN complete{n=2 model=\"openai::qwen3-coder\" format=schema label=\"verify\"}: omnia_genai::observe: completion outcome=\"timeout\" attempts=0 duration_ms=600000 model_ms=600000 tool_ms=0 check_ms=0 input_tokens=0 cache_read_tokens=0 cache_write_tokens=0 output_tokens=0 reasoning_tokens=0
";
        let completions = completions(log);
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(labels, ["build-SLICE-001", "verify"]);
        assert_eq!(completions[0].outcome, "corrected");
        assert_eq!(completions[0].duration, Duration::from_secs(120));
        assert_eq!(completions[0].tokens.input, 90000);
        assert_eq!(completions[0].tokens.output, 7000);
        assert_eq!(
            completions[0].shell,
            ShellUse {
                calls: 1,
                time: Duration::from_secs(90),
                bytes: 4096
            },
            "the shell alone is counted, never a guest tool"
        );
        assert_eq!(completions[0].writes.edits, 1);
        assert_eq!(completions[1].outcome, "timeout");
        assert_eq!(completions[1].shell, ShellUse::default());
    }

    // A run's repository as the engine leaves it: the `Revision` root the
    // head stays on, and the label a build moved over the slices it merged.
    #[test]
    fn rebuild_clones_the_sealed_root() {
        let root = scratch("sealed-root", &[("spec/spec.json", "{}\n")]);
        let commit = |message: &str| {
            git(&root, &["add", "--all"]).unwrap();
            git(&root, &["commit", "--quiet", "--message", message]).unwrap();
        };
        git(&root, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        commit("Revision feb4bd36\n\nRevision: feb4bd36");
        git(&root, &["checkout", "--quiet", "--detach"]).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        commit("Initialise");
        git(&root, &["branch", "emery/feb4bd36"]).unwrap();
        git(&root, &["checkout", "--quiet", "main"]).unwrap();
        let project = scratch("sealed-root-rebuilt", &[]);
        let clone = repository(&project, 1);

        clone_root(&root, &clone, "feb4bd36").unwrap();
        let subject = git(&clone, &["log", "-1", "--format=%s", "HEAD"]).unwrap();
        let branches = git(&clone, &["branch", "--all", "--format=%(refname:short)"]).unwrap();
        let tracked = git(&clone, &["ls-files"]).unwrap();
        let other = clone_root(&root.join("missing"), &repository(&project, 2), "feb4bd36");
        git(&root, &["reset", "--quiet", "--hard", "emery/feb4bd36"]).unwrap();
        let moved = clone_root(&root, &repository(&project, 3), "feb4bd36");
        fs::remove_dir_all(&root).unwrap();
        fs::remove_dir_all(&project).unwrap();

        assert_eq!(clone, project.join(".emery/vcs/repos/run-1"));
        assert_eq!(subject, "Revision feb4bd36");
        assert_eq!(branches, "main", "neither the label nor a remote stands in the clone");
        assert_eq!(tracked, "spec/spec.json");
        assert!(other.is_err(), "a repository nothing sealed is refused");
        let moved = moved.unwrap_err().to_string();
        assert!(moved.contains("holds `Initialise` at its head"), "{moved}");
    }

    // `apply` labels the `Revision` commit before `build` runs, and a build
    // that fails in wave 1 before any slice merges leaves only `Initialise`
    // over it in `.emery/vcs/integration`; neither commit is a build's, so
    // the run's history and tree are empty until a slice merges, and an
    // earlier run's integration copy is never read as this run's.
    #[test]
    fn history_ignores_unbuilt_revision_root() {
        let project = scratch("unbuilt-root", &[]);
        let commit = |repo: &Path, file: &str, message: &str| {
            fs::write(repo.join(file), "{}\n").unwrap();
            git(repo, &["add", "--all"]).unwrap();
            git(repo, &["commit", "--quiet", "--message", message]).unwrap();
        };
        let repo1 = repository(&project, 1);
        let repo2 = repository(&project, 2);
        let integration = project.join(".emery/vcs/integration");
        for dir in [&repo1, &repo2, &integration] {
            fs::create_dir_all(dir).unwrap();
            git(dir, &["init", "--quiet", "--initial-branch=main"]).unwrap();
        }
        commit(&repo1, "spec.json", "Revision aaaaaaaa\n\nRevision: aaaaaaaa");
        git(&repo1, &["branch", "emery/aaaaaaaa"]).unwrap();
        commit(&repo2, "spec.json", "Revision bbbbbbbb\n\nRevision: bbbbbbbb");
        git(&repo2, &["branch", "emery/bbbbbbbb"]).unwrap();
        commit(&integration, "spec.json", "Revision aaaaaaaa\n\nRevision: aaaaaaaa");
        commit(&integration, "init.txt", "Initialise");

        let unbuilt = build_from(1, "", "", history(&project, 1, "aaaaaaaa"));
        let unbuilt_tree = Tree::left(&project, 1, "aaaaaaaa", &unbuilt);

        commit(
            &integration,
            "orders.rs",
            "SLICE-001 orders\n\nSlice: SLICE-001\nRevision: aaaaaaaa\nWave: 1",
        );
        let run1 = build_from(1, "", "", history(&project, 1, "aaaaaaaa"));
        let run1_tree = Tree::left(&project, 1, "aaaaaaaa", &run1);
        let run2 = build_from(1, "", "", history(&project, 2, "bbbbbbbb"));
        let run2_tree = Tree::left(&project, 2, "bbbbbbbb", &run2);
        fs::remove_dir_all(&project).unwrap();

        assert_eq!(unbuilt.history.len(), 0);
        assert!(unbuilt_tree.is_none());
        assert_eq!(run1.history.len(), 3);
        assert!(matches!(run1_tree, Some(Tree::Integration(_))));
        assert_eq!(run2.history.len(), 0, "run 2 merged nothing and ignores run 1's copy");
        assert!(run2_tree.is_none(), "run 1's integration copy is never read for run 2");
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
    fn tests_named_across_separators() {
        let names = vec!["per_probe_budget_is_enforced".to_owned(), "orders__are_paid".to_owned()];
        assert_eq!(named_by("Per-probe budget is enforced", &names), Some(Named::Exact));
        assert_eq!(named_by("Orders are paid, then shipped", &names), Some(Named::Fuzzy));
        assert_eq!(named_by("Per-probe", &names), None);
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
            "spec/spec.json",
            "spec/plan.json",
        ] {
            assert!(Target::Node.within_layout(path, &stems), "`{path}` is within the layout");
        }
        for path in [
            "README.md",
            "scripts/check.mjs",
            "src/orders-reading/index.ts",
            "test/helpers.ts",
            "src/x.ts",
            "src/spec/spec.json",
            "spec.json",
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
            "spec/design.json",
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
        assert_eq!(integrated.len(), 2, "the root the engine created is no slice");
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
