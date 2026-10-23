//! Seals and builds a run's plan, resumes an earlier chain, and reads the build back.

use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use std::{fs, io};

use serde_json::Value;

use crate::card::short;
use crate::case::Case;
use crate::fidelity::{Fidelity, Target};
use crate::reference::Reference;
use crate::run::{Completion, Run, ShellUse, Tokens, completions, tail};
use crate::stage::{
    RUN_1, Restaged, TARGET_NAME, export_tip, git, repository, restaged, seed_drift,
};
use crate::{Settings, Turns, emery};

const APPLY: &[&str] = &["apply", "--config", "emery.toml"];

// One `emery apply` of the staged candidate `revision`, sealing it as the
// root commit of the repository the engine creates for the run's `[target]`,
// so `build` finds it there; the log and envelope are kept as
// `<tag>.apply.stderr` / `<tag>.apply.stdout`. A seal spends no turn, so one
// that fails, or seals another revision than the one staged, is the
// harness's error, never the run's.
pub fn apply(
    project: &Path, settings: &Settings, case: &Case, tag: &str, revision: &str,
) -> io::Result<()> {
    let rust_log = settings.rust_log(case);
    let output = emery(project, settings, &rust_log, settings.build_rung, APPLY, Turns::Live)?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    fs::write(project.join(format!("{tag}.apply.stderr")), stderr.as_bytes())?;
    fs::write(project.join(format!("{tag}.apply.stdout")), &output.stdout)?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "`{tag}` of `{}`: `emery apply` exited {}:\n{}",
            case.name,
            output.status.code().map_or_else(|| "killed".to_owned(), |code| code.to_string()),
            tail(&stderr)
        )));
    }
    let envelope: Value = serde_json::from_slice(&output.stdout)?;
    let sealed = envelope["revision"].as_str().unwrap_or("");
    if sealed != revision {
        return Err(io::Error::other(format!(
            "`{tag}` of `{}`: `emery apply` sealed `{sealed}`, not the staged `{revision}`",
            case.name
        )));
    }
    Ok(())
}

// One `emery build` of the revision run `n` sealed, at the build rung, into
// the repository the engine created for it, then the host's own pass over
// the tree it left; the build's log and envelope are kept as
// `<tag>.build.stderr` / `<tag>.build.stdout`, the pass as
// `<tag>.fidelity.md`.
#[expect(clippy::disallowed_methods, reason = "the eval is a native operator tool, not a guest")]
pub fn build(
    project: &Path, settings: &Settings, case: &Case, n: usize, tag: &str, revision: &str,
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
    let target =
        settings.target.ok_or_else(|| io::Error::other("a build runs through a target"))?;

    let mut build = Build {
        target,
        wall,
        exit: output.status.code(),
        envelope: serde_json::from_str(&stdout).ok(),
        failure: failure(&stderr),
        completions: completions(&stderr),
        history: history(project, n, revision),
        tail: tail(&stderr),
        fidelity: None,
        rewound: None,
    };
    // the host's pass is read beside the build, never in its place: a
    // toolchain the host lacks leaves the cells empty and the card whole
    if let Some(tree) = Tree::left(project, n, revision, &build) {
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

// Builds the plan `run-1` of `case` sealed under `card` again. The staged
// project is copied whole under the new card, less the engine's state, its
// clones and working copies, the repository the earlier run sealed and built
// into, and the earlier build's files; that repository is cloned at its root
// commit alone, the `Revision` commit holding the three documents, so the
// engine reads the revision from the head's tree, finds no label, and builds
// every slice over the root; the `[target]` table of `emery.toml` is
// rewritten for this invocation's target; and the run's specify is read back
// from the files the earlier run kept, exactly as `run` read them, so the
// card's specify columns repeat the source card's.
pub fn rebuilt(
    eval_dir: &Path, card: &str, card_name: &str, case: &Case, settings: &Settings,
) -> io::Result<Run> {
    let Restaged {
        project,
        revision,
        mut run,
    } = restaged(eval_dir, card, card_name, case, settings, false)?;
    eprintln!("eval: `{}` rebuilding revision `{revision}` at {}", case.name, settings.build_rung);
    run.build = Some(build(&project, settings, case, 1, RUN_1, &revision, run.spec.as_ref())?);
    Ok(run)
}

// Builds the plan `run-1` of `case` committed under `card` on from `wave`.
// The staged project is copied as `rebuilt` copies it, with the repository
// the earlier build created, and the label `emery/<revision>` in it is moved
// to the last commit of wave `wave - 1`, a merge of that wave or its seal,
// so the engine resumes there: the slices the chain names are not built
// again, the plan's other slices are built in waves over that tree, and the
// first slice this invocation's model builds is one of `wave`'s. A slice of
// an earlier wave that merged nothing is not in the chain, so the engine
// would build it again, a wave of its own verified again before `wave`'s
// slice; every such slice but the first is sealed over the moved label as
// an empty commit under the engine's own message, so one seed verify stands
// before the slice. Before the label is moved, the component's template is
// read against the tree at that commit for the `omnia` target, and a seed
// the template has moved under is refused with the drift unless `drifted`
// admits it. The run records the card, the wave, the commit resumed from,
// and the slices resumed, the sealed ones among them.
pub fn resumed(
    eval_dir: &Path, card: &str, wave: usize, drifted: bool, card_name: &str, case: &Case,
    settings: &Settings,
) -> io::Result<Run> {
    let Restaged {
        project,
        revision,
        mut run,
    } = restaged(eval_dir, card, card_name, case, settings, true)?;
    let repository = repository(&project, 1);
    let label = format!("emery/{revision}");
    let chain = log(&repository, &format!("refs/heads/{label}"));
    if chain.is_empty() {
        return Err(io::Error::other(format!(
            "`{RUN_1}` of `{}` left no `{label}` in `{}` to resume from",
            case.name,
            repository.display()
        )));
    }
    let before = wave - 1;
    let at = chain
        .iter()
        .find(|landed| landed.wave == Some(before) || landed.sealed() == Some(before))
        .ok_or_else(|| {
            io::Error::other(format!(
                "`{RUN_1}` of `{}` merged and sealed nothing in wave {before}, so there is no \
                 commit to resume wave {wave} from",
                case.name
            ))
        })?;
    if settings.target == Some(Target::Omnia) {
        let scratch = eval_dir.join(card_name).join(format!("{}.seed", case.name));
        match seed_drift(&repository, &at.commit, &scratch)? {
            None => eprintln!(
                "eval: `{}` seed at `{}` holds the component's template",
                case.name,
                short(&at.commit)
            ),
            Some(drift) if !drifted => {
                return Err(io::Error::other(format!(
                    "`{card}` seeds wave {wave} of `{}` with a tree laid from an earlier \
                     template than the component staged carries, so its seed verify would be \
                     refused by the template's findings before any slice is built:\n{drift}\n\
                     resume a card built under this component, `--build` one, or admit the \
                     seed with `--drifted` and read the seed verify as the drift's repair",
                    case.name
                )));
            }
            Some(drift) => eprintln!(
                "eval: `{}` resumes a seed the template has moved under, admitted by \
                 `--drifted`:\n{drift}",
                case.name
            ),
        }
    }
    git(&repository, &["branch", "-f", &label, &at.commit])?;
    let mut slices: Vec<String> = chain
        .iter()
        .skip_while(|landed| landed.commit != at.commit)
        .filter_map(|landed| landed.slice.clone())
        .collect();

    // the earlier waves' empty slices sealed, all but the first
    let envelope = fs::read_to_string(
        eval_dir.join(card).join(&case.name).join(format!("{RUN_1}.build.stdout")),
    )
    .ok()
    .and_then(|stdout| serde_json::from_str::<Value>(&stdout).ok());
    let mut empty = envelope.as_ref().map(|out| empty_before(out, wave)).unwrap_or_default();
    empty.retain(|slice| !slices.contains(&slice.id));
    for slice in empty.iter().skip(1) {
        let head = git(&repository, &["rev-parse", &format!("refs/heads/{label}")])?;
        let tree = git(&repository, &["rev-parse", &format!("{head}^{{tree}}")])?;
        let message = slice.message(&revision, &head);
        let commit = git(&repository, &["commit-tree", &tree, "-p", &head, "-m", &message])?;
        git(&repository, &["update-ref", &format!("refs/heads/{label}"), &commit])?;
        eprintln!(
            "eval: `{}` sealed `{} {}` of wave {} over the label, merged as nothing",
            case.name, slice.id, slice.name, slice.wave
        );
        slices.push(slice.id.clone());
    }
    eprintln!(
        "eval: `{}` resuming revision `{revision}` from `{}` ({}) at {}",
        case.name,
        short(&at.commit),
        at.subject,
        settings.build_rung
    );
    let mut built = build(&project, settings, case, 1, RUN_1, &revision, run.spec.as_ref())?;
    built.rewound = Some(Rewound {
        card: card.to_owned(),
        wave,
        commit: at.commit.clone(),
        subject: at.subject.clone(),
        slices,
    });
    run.build = Some(built);
    Ok(run)
}

// The reference comparison alone over the tree `run-1` of the case staged at
// `project` left: the tip exported once to `run-1.oracle/` beside the run's
// files and kept, so the next invocation reuses its build directory and an
// operator may patch the export by hand and read the oracle again, and the
// comparison written to `run-1.oracle.md` with the harness's own output in
// `run-1.oracle.log`.
pub fn oracle(project: &Path, reference: &Reference) -> io::Result<(PathBuf, String)> {
    const TAG: &str = "run-1";
    const N: usize = 1;
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
        .ok_or_else(|| io::Error::other(format!("`{TAG}` staged no revision")))?;
    let build = Build::recorded(project, N, TAG, Target::Omnia, revision)?;
    let tree = Tree::left(project, N, revision, &build).ok_or_else(|| {
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

// What one `emery build` did, read from its envelope, its log, and the
// repository it built into.
pub struct Build {
    pub target: Target,
    pub wall: Duration,
    pub exit: Option<i32>,
    // the `build` envelope on success
    pub envelope: Option<Value>,
    // the failure envelope the binary wrote among its log lines
    pub failure: Option<Failure>,
    pub completions: Vec<Completion>,
    // the first-parent chain the build left, newest first: the label the
    // run set, or the integration copy a failed run left behind
    pub history: Vec<Landed>,
    pub tail: String,
    // the host's own pass over the tree the chain stands at; none where the
    // build left no tree
    pub fidelity: Option<Fidelity>,
    // the chain this build resumed, for a `--resume`; none for a build from
    // the root
    pub rewound: Option<Rewound>,
}

// Where a `--resume` build started: the card whose chain it resumed, the
// wave it resumed at, the commit the label was moved to, and the slices
// that chain names, which the engine resumed rather than built.
pub struct Rewound {
    pub card: String,
    pub wave: usize,
    pub commit: String,
    pub subject: String,
    pub slices: Vec<String>,
}

// A slice an earlier build merged as nothing, read from its envelope: no
// commit names it in the chain, so a resume builds it again unless a commit
// does.
#[derive(Debug, PartialEq, Eq)]
pub struct Empty {
    pub id: String,
    name: String,
    wave: usize,
    requirements: Vec<String>,
    covered: Vec<String>,
}

impl Empty {
    // The message the engine seals a slice under, as `emery_engine::vcs::Message`
    // spells it, over `base`: the first line `<id> <name>`, then the seven
    // trailers, every one present, so the engine's resume reads the slice
    // back from the label's chain.
    pub fn message(&self, revision: &str, base: &str) -> String {
        format!(
            "{id} {name}\n\nSlice: {id}\nRevision: {revision}\nRequirements: {requirements}\n\
             Covered: {covered}\nAdapter: {TARGET_NAME}\nBase: {base}\nWave: {wave}",
            id = self.id,
            name = self.name,
            requirements = self.requirements.join(", "),
            covered = self.covered.join(", "),
            wave = self.wave,
        )
    }
}

// The slices of `envelope`, a landed build's, merged as nothing in a wave
// before `wave`, in wave then id order.
pub fn empty_before(envelope: &Value, wave: usize) -> Vec<Empty> {
    let mut empty: Vec<Empty> = envelope["slices"]
        .as_array()
        .map(|slices| {
            slices
                .iter()
                .filter(|slice| slice["commit"].is_null())
                .filter_map(|slice| {
                    let landed: usize = slice["wave"].as_u64()?.try_into().ok()?;
                    (landed < wave).then(|| {
                        let covered = strings(&slice["covered"]);
                        let mut requirements = covered.clone();
                        requirements.extend(strings(&slice["uncovered"]));
                        Empty {
                            id: slice["id"].as_str().unwrap_or("").to_owned(),
                            name: slice["name"].as_str().unwrap_or("").to_owned(),
                            wave: landed,
                            requirements,
                            covered,
                        }
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    empty.sort_by(|a, b| (a.wave, &a.id).cmp(&(b.wave, &b.id)));
    empty
}

// The failure envelope: its code and message.
pub struct Failure {
    pub code: String,
    pub message: String,
}

// One commit on the build's first-parent chain: a slice's merge, read from
// its trailers, a `Wave <k> verified` seal, `Initialise`, or the `Revision`
// root `apply` sealed.
#[derive(Debug, PartialEq, Eq)]
pub struct Landed {
    pub commit: String,
    pub subject: String,
    // the `Revision:` trailer a merge, a wave seal, and the `Revision` root
    // carry; `Initialise` none
    pub revision: Option<String>,
    pub slice: Option<String>,
    pub wave: Option<usize>,
    pub requirements: Vec<String>,
    pub covered: Vec<String>,
    pub files: Vec<String>,
}

impl Landed {
    // The wave a `Wave <k> verified` seal closed; a merge, `Initialise`, and
    // the root none.
    pub fn sealed(&self) -> Option<usize> {
        self.subject.strip_prefix("Wave ")?.strip_suffix(" verified")?.parse().ok()
    }

    // Whether a build of `revision` made this commit — a slice merge or a
    // wave seal — rather than the `Revision` root `apply` sealed before it
    // or `Initialise`.
    fn built(&self, revision: &str) -> bool {
        self.revision.as_deref() == Some(revision)
            && (self.slice.is_some() || self.sealed().is_some())
    }
}

// One slice as the build landed it: from the envelope when the run landed,
// else from the merge commit's trailers.
pub struct Integrated {
    pub id: String,
    pub name: String,
    pub wave: usize,
    pub requirements: Vec<String>,
    pub covered: Vec<String>,
    pub files: Vec<String>,
    pub commit: Option<String>,
    pub resolved: Vec<String>,
}

impl Integrated {
    pub fn uncovered(&self) -> Vec<&str> {
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
pub struct WaveTurns<'a> {
    pub builds: Vec<&'a Completion>,
    pub verifies: Vec<&'a Completion>,
}

impl WaveTurns<'_> {
    // The wave's line tail: its build durations, its verify's duration with
    // the shell part, and what the verify sealed — `(lock only)` for the
    // lock rewrite alone, else `repaired` with the files.
    pub fn spent(&self, sealed: Option<&Vec<String>>) -> String {
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
    pub fn critical(&self) -> Duration {
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
    fn recorded(
        project: &Path, n: usize, tag: &str, target: Target, revision: &str,
    ) -> io::Result<Self> {
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
        Ok(Self {
            target,
            wall: Duration::ZERO,
            exit,
            envelope,
            failure,
            completions: completions(&stderr),
            history: history(project, n, revision),
            tail: tail(&stderr),
            fidelity: None,
            rewound: None,
        })
    }

    pub fn summary(&self, slices: usize) -> String {
        match self.exit {
            Some(0) => format!(
                "ok in {}s, {}/{slices} merged in {} waves{}",
                self.wall.as_secs(),
                self.integrated().len(),
                self.waves(),
                self.resumption()
            ),
            Some(code) => {
                let failed = self.failure.as_ref().map_or(String::new(), |failure| {
                    let wave =
                        self.wave_failed().map_or(String::new(), |k| format!(" in wave {k}"));
                    format!(" `{}`{wave}", failure.code)
                });
                format!("exit {code}{failed} after {}s{}", self.wall.as_secs(), self.resumption())
            }
            None => format!("killed after {}s{}", self.wall.as_secs(), self.resumption()),
        }
    }

    pub fn landed(&self) -> bool {
        self.exit == Some(0)
    }

    pub fn code(&self) -> Option<&str> {
        self.failure.as_ref().map(|failure| failure.code.as_str())
    }

    // The slices this build built: the merged ones less those the resumed
    // chain already named.
    pub fn built(&self) -> Vec<Integrated> {
        let resumed = self.rewound.as_ref().map(|rewound| rewound.slices.as_slice());
        self.integrated()
            .into_iter()
            .filter(|slice| !resumed.is_some_and(|resumed| resumed.contains(&slice.id)))
            .collect()
    }

    // The resumed and built counts, led by `, `, for a build that resumed;
    // empty for one from the root.
    pub fn resumption(&self) -> String {
        self.rewound.as_ref().map_or_else(String::new, |rewound| {
            format!(", {} resumed, {} built", rewound.slices.len(), self.built().len())
        })
    }

    // The slices the build merged, in build order.
    pub fn integrated(&self) -> Vec<Integrated> {
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
    pub fn waves(&self) -> usize {
        if self.landed() {
            return self.integrated().iter().map(|slice| slice.wave).max().unwrap_or(0);
        }
        self.wave_turns().len()
    }

    // The waves whose verify passed: every one the envelope lists, else
    // every wave whose verify answered, less the one whose verdict failed
    // the run — a `verify-failed` is an answered verify, and the last.
    pub fn verified(&self) -> usize {
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
    pub fn repaired(&self) -> Vec<usize> {
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
    pub fn repairs(&self) -> BTreeMap<usize, Vec<String>> {
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
    pub fn wave_failed(&self) -> Option<usize> {
        let message = &self.failure.as_ref()?.message;
        let (_, rest) = message.split_once("wave ")?;
        rest.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    }

    // The failures a `verify-failed` run quotes, one per `- ` line.
    pub fn verify_failures(&self) -> Vec<&str> {
        match &self.failure {
            Some(failure) if failure.code == "verify-failed" => {
                failure.message.lines().filter_map(|line| line.strip_prefix("- ")).collect()
            }
            _ => Vec::new(),
        }
    }

    // The completions grouped into the waves they were spent in; a wave
    // whose verify never opened, the one a failure ended, is the last.
    pub fn wave_turns(&self) -> Vec<WaveTurns<'_>> {
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
    pub fn serial(&self) -> Duration {
        self.completions
            .iter()
            .filter(|c| c.label == "verify" || c.label.starts_with("build-"))
            .map(|completion| completion.duration)
            .sum()
    }

    // The time the waves took end to end, each at its slowest slice.
    pub fn critical(&self) -> Duration {
        self.wave_turns().iter().map(WaveTurns::critical).sum()
    }

    // How much shorter the waves ran than the same turns in a row, in
    // hundredths.
    pub fn speedup(&self) -> Option<usize> {
        let critical = self.critical().as_millis();
        if critical == 0 {
            return None;
        }
        usize::try_from(self.serial().as_millis() * 100 / critical).ok()
    }

    // The requirements covered over the requirements of the slices merged.
    pub fn covered(&self) -> (usize, usize) {
        self.integrated().iter().fold((0, 0), |(covered, all), slice| {
            (covered + slice.covered.len(), all + slice.requirements.len())
        })
    }

    pub fn tokens(&self) -> Tokens {
        self.completions
            .iter()
            .map(|completion| completion.tokens)
            .fold(Tokens::default(), Tokens::add)
    }
}

pub fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

// The failure envelope the binary prints among its log lines: pretty JSON,
// so its braces stand alone at the margin where no log line starts.
pub fn failure(stderr: &str) -> Option<Failure> {
    let lines: Vec<&str> = stderr.lines().collect();
    let open = lines.iter().rposition(|line| *line == "{")?;
    let close = open + lines[open..].iter().position(|line| *line == "}")?;
    let envelope: Value = serde_json::from_str(&lines[open..=close].join("\n")).ok()?;
    Some(Failure {
        code: envelope["error"].as_str()?.to_owned(),
        message: envelope["message"].as_str().unwrap_or("").to_owned(),
    })
}

// The chain the build of run `n` left, read where it stands: the integration
// copy a failed run leaves for inspection, else the label a landed run set
// in the repository the engine created for the run. The project is staged
// once for every run of a case and the integration copy sits at one path, so
// the copy standing may be an earlier run's, left by a build that failed
// after cutting it, when this run's failed before it cut one, and the label
// `apply` set before the build holds the `Revision` commit alone: either
// chain is this build's only where it holds a slice merge or a wave seal of
// this revision. A chain holding neither is empty.
pub fn history(project: &Path, n: usize, revision: &str) -> Vec<Landed> {
    let integration = project.join(".emery/vcs/integration");
    if integration.is_dir() {
        let left = log(&integration, "HEAD");
        if left.iter().any(|landed| landed.built(revision)) {
            return left;
        }
    }
    let repository = repository(project, n);
    if !repository.is_dir() {
        return Vec::new();
    }
    let labelled = log(&repository, &format!("refs/heads/emery/{revision}"));
    if labelled.iter().any(|landed| landed.built(revision)) {
        return labelled;
    }
    Vec::new()
}

// The first-parent chain from `tip`, newest first; empty where `tip` is
// nothing the repository holds.
pub fn log(at: &Path, tip: &str) -> Vec<Landed> {
    let format = "--format=%x1e%H%x1f%s%x1f%(trailers:key=Revision,valueonly)%x1f\
                  %(trailers:key=Slice,valueonly)%x1f%(trailers:key=Wave,valueonly)%x1f\
                  %(trailers:key=Requirements,valueonly)%x1f%(trailers:key=Covered,valueonly)%x1f";
    let log = git(at, &["log", "--first-parent", "--name-only", format, tip]);
    log.map(|text| chain(&text)).unwrap_or_default()
}

// The chain as `log` asks git to spell it: one record per commit led by
// `\x1e`, its fields split by `\x1f`, the files it changed against its
// first parent after the last.
pub fn chain(text: &str) -> Vec<Landed> {
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

// Where a build left its tree: the label in the repository the engine created
// for the run once the run landed, else the integration copy's head a failed
// run leaves, read only where its chain is this run's.
pub enum Tree {
    Label { repository: PathBuf, label: String },
    Integration(PathBuf),
}

impl Tree {
    pub fn left(project: &Path, n: usize, revision: &str, build: &Build) -> Option<Self> {
        if build.landed() {
            return Some(Self::Label {
                repository: repository(project, n),
                label: format!("emery/{revision}"),
            });
        }
        let integration = project.join(".emery/vcs/integration");
        if integration.is_dir()
            && log(&integration, "HEAD").iter().any(|landed| landed.built(revision))
        {
            return Some(Self::Integration(integration));
        }
        (!build.history.is_empty()).then(|| Self::Label {
            repository: repository(project, n),
            label: format!("emery/{revision}"),
        })
    }

    // The repository to read and the tip within it.
    pub fn at(&self) -> (&Path, &str) {
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
