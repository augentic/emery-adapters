//! The live eval runner (operator-invoked, never CI): spawns the
//! sibling shipped `emery` binary over the built first-party
//! components, times init → committed generation pointer, records
//! per-operation outcomes from the typed contract, grades the
//! committed spec set, and writes the dated scorecard.
//!
//! Usage: `cargo make eval [case-id]` after `cargo make release`
//! (components) and a `cargo build --release` of the sibling
//! `../emery` checkout (override with `EMERY_BIN` / `EMERY_REPO`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

use eval::envelope;
use eval::grade::{self, Expect};
use eval::scorecard::{CaseResult, Outcome, Scorecard};

/// One graded spec-generation case: bounded source fixtures in, one
/// committed and graded spec set out.
struct Case {
    id: &'static str,
    /// Workspace-backed source components, by crate name under
    /// `sources/` (built to `target/wasm32-wasip2/release/<name>.wasm`).
    components: &'static [&'static str],
    /// The operator brief, bound as the inline `intent` value.
    intent: &'static str,
    /// Optional first-run shallow clone into the case fixture:
    /// `(url, fixture-relative destination)`.
    clone: Option<(&'static str, &'static str)>,
    /// Graded expectations over the committed spec.
    expect: Expect,
}

/// The graded case catalog. Both estates are bounded on purpose: the
/// orders docs fixture is one written specification; omnia-r9k is one
/// shallow-cloned legacy adapter.
const CASES: &[Case] = &[
    Case {
        id: "orders-docs",
        components: &["documentation"],
        intent: "Specify the orders service's externally observable behaviour — order \
                 placement, order state, and cancellation — from the bound documentation.",
        clone: None,
        expect: Expect {
            subject_fragment: "order",
        },
    },
    Case {
        id: "omnia-r9k",
        components: &["typescript"],
        intent: "Specify the externally observable behaviour of the legacy TypeScript \
                 at_r9k_position_adapter under legacy/ so it can be rebuilt as an Omnia \
                 WASM crate.",
        clone: Some((
            "https://bitbucket.org/Propellerhead/at_r9k_position_adapter.git",
            "legacy/at_r9k_position_adapter",
        )),
        expect: Expect {
            subject_fragment: "position",
        },
    },
];

fn main() {
    let mut args = std::env::args().skip(1);
    let filter = args.next();
    if let Some(unknown) = &filter
        && !CASES.iter().any(|case| case.id == unknown)
    {
        eprintln!("unknown case `{unknown}`; cases:");
        for case in CASES {
            eprintln!("  {}", case.id);
        }
        std::process::exit(2);
    }

    let paths = Paths::locate();
    let cases: Vec<CaseResult> = CASES
        .iter()
        .filter(|case| filter.as_deref().is_none_or(|id| id == case.id))
        .map(|case| run_case(case, &paths))
        .collect();

    let scorecard = Scorecard {
        date: capture("date", &["+%F"], None),
        emery_sha: capture("git", &["rev-parse", "HEAD"], Some(&paths.emery_repo)),
        adapters_sha: capture("git", &["rev-parse", "HEAD"], Some(&paths.root)),
        cases,
    };
    let rendered = scorecard.render();
    print!("{rendered}");
    let out = paths.root.join("sandbox/scorecard.md");
    std::fs::create_dir_all(out.parent().expect("sandbox parent")).expect("mkdir sandbox");
    std::fs::write(&out, &rendered).expect("write the scorecard");
    println!("\nscorecard written to {}", out.display());
    if !scorecard.green() {
        std::process::exit(1);
    }
}

/// The resolved on-disk layout of one runner invocation.
struct Paths {
    /// The emery-adapters repository root.
    root: PathBuf,
    /// The sibling `augentic/emery` checkout the exercised binary was
    /// built from.
    emery_repo: PathBuf,
    /// The shipped `emery` binary.
    emery_bin: PathBuf,
}

impl Paths {
    fn locate() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = root.canonicalize().expect("adapters root");
        let emery_repo =
            std::env::var_os("EMERY_REPO").map_or_else(|| root.join("../emery"), PathBuf::from);
        // A redirected CARGO_TARGET_DIR is shared by both checkouts —
        // the same redirect this runner was built under.
        let emery_target = std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| emery_repo.join("target"), PathBuf::from);
        let emery_bin = std::env::var_os("EMERY_BIN")
            .map_or_else(|| emery_target.join("release/emery"), PathBuf::from);
        assert!(
            emery_bin.is_file(),
            "shipped binary missing at {}; run `cargo build --release --bin emery` in the \
             sibling emery checkout (or set EMERY_BIN)",
            emery_bin.display()
        );
        Self {
            root,
            emery_repo,
            emery_bin,
        }
    }

    /// The built component for one adapter crate name.
    fn component(&self, name: &str) -> PathBuf {
        let target = std::env::var_os("CARGO_TARGET_DIR")
            .map_or_else(|| self.root.join("target"), PathBuf::from);
        let built = target.join(format!("wasm32-wasip2/release/{name}.wasm"));
        assert!(
            built.is_file(),
            "component missing at {}; run `cargo make release`",
            built.display()
        );
        built
    }
}

/// Run one case end to end and record its typed result.
fn run_case(case: &Case, paths: &Paths) -> CaseResult {
    println!("== case {}", case.id);
    let fixture = paths.root.join(format!("examples/eval/cases/{}/fixture", case.id));
    if let Some((url, dest)) = case.clone {
        ensure_clone(url, &fixture.join(dest));
    }

    // A fresh retained sandbox per run: the project directory the
    // operator can inspect after grading.
    let project = paths.root.join(format!("sandbox/{}", case.id));
    if project.exists() {
        std::fs::remove_dir_all(&project).expect("reset the case sandbox");
    }
    std::fs::create_dir_all(&project).expect("create the case sandbox");
    if fixture.is_dir() {
        copy_tree(&fixture, &project);
    }

    // Stage the built components inside the project: the launcher
    // mounts only the project root and the cache, so the guest cannot
    // read a component outside them.
    for component in case.components.iter().chain(&["intent"]) {
        let staged = project.join(format!("{component}.wasm"));
        std::fs::copy(paths.component(component), &staged).expect("stage component");
    }

    // One extract per source binding (workspace components plus the
    // inline intent) and one synthesis.
    let extracts = u32::try_from(case.components.len()).expect("case size") + 1;
    let started = Instant::now();

    let mut init: Vec<String> = vec!["--format".into(), "json".into(), "init".into()];
    for component in case.components {
        init.push(format!("{component}.wasm"));
    }
    init.push("--value".into());
    init.push(format!("intent.wasm={}", case.intent));
    let output = emery(paths, &project, &init);
    if !output.status.success() {
        return failed(case, started, &output);
    }

    let output = emery(paths, &project, &["--format".into(), "json".into(), "specify".into()]);
    let secs = started.elapsed().as_secs_f64();
    if !output.status.success() {
        return failed(case, started, &output);
    }

    let outcome = match envelope::success(&output.stdout) {
        Ok(body) => graded(case, &project, &body),
        Err(finding) => Outcome::Findings(vec![finding]),
    };
    CaseResult {
        id: case.id.to_string(),
        // Every operation behind a success envelope completed; graded
        // findings are spec-quality failures, not operation failures.
        ops_succeeded: extracts + 1,
        ops_failed: 0,
        outcome,
        secs,
    }
}

/// Grade the committed spec set behind the generation pointer.
fn graded(case: &Case, project: &Path, body: &envelope::Success) -> Outcome {
    let spec = project.join(format!(".emery/spec/generations/{}/spec.md", body.generation));
    let Ok(text) = std::fs::read_to_string(&spec) else {
        return Outcome::Findings(vec![format!(
            "the envelope names generation `{}` but {} is unreadable",
            body.generation,
            spec.display()
        )]);
    };
    let findings = grade::spec(&text, &case.expect);
    if findings.is_empty() {
        Outcome::Pass {
            generation: body.generation.clone(),
        }
    } else {
        Outcome::Findings(findings)
    }
}

/// Record a typed nonzero exit: the failure envelope is the outcome,
/// never something to grade around (T6). The failed operation counts
/// against the per-operation rate; operations the run never reached
/// stay unrecorded.
fn failed(case: &Case, started: Instant, output: &Output) -> CaseResult {
    let (error, exit_code) = match envelope::failure(&output.stderr) {
        Ok(body) => (body.error, body.exit_code),
        Err(_) => (
            format!("unparseable failure: {}", String::from_utf8_lossy(&output.stderr).trim()),
            output.status.code().and_then(|code| u8::try_from(code).ok()).unwrap_or(1),
        ),
    };
    CaseResult {
        id: case.id.to_string(),
        outcome: Outcome::TypedFailure { error, exit_code },
        secs: started.elapsed().as_secs_f64(),
        ops_succeeded: 0,
        ops_failed: 1,
    }
}

/// Spawn one `emery` invocation in the case project, isolated under
/// the sandbox `EMERY_HOME`.
fn emery(paths: &Paths, project: &Path, args: &[String]) -> Output {
    Command::new(&paths.emery_bin)
        .current_dir(project)
        .env("EMERY_HOME", paths.root.join("sandbox/emery-home"))
        .args(args)
        .output()
        .expect("spawn the emery binary")
}

/// Shallow-clone `url` into `dest` once; an existing clone is the
/// cached fixture.
fn ensure_clone(url: &str, dest: &Path) {
    if dest.is_dir() {
        return;
    }
    std::fs::create_dir_all(dest.parent().expect("clone parent")).expect("mkdir clone parent");
    let status = Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(dest)
        .status()
        .expect("spawn git clone");
    assert!(status.success(), "shallow clone of {url} failed");
}

/// Copy `from`'s tree into `to` (which exists).
fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).expect("read fixture dir") {
        let entry = entry.expect("fixture entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            std::fs::create_dir_all(&target).expect("mkdir fixture subdir");
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy fixture file");
        }
    }
}

/// One captured line of a helper command (`git rev-parse`, `date`);
/// `[unknown]` when the command is unavailable — recorded, not guessed.
fn capture(program: &str, args: &[&str], dir: Option<&Path>) -> String {
    let mut command = Command::new(program);
    command.args(args);
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    command.output().ok().filter(|output| output.status.success()).map_or_else(
        || "[unknown]".to_string(),
        |output| String::from_utf8_lossy(&output.stdout).trim().to_string(),
    )
}
