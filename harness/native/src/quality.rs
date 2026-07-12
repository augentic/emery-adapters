//! The native-live quality runner over the in-process guest loop.
//!
//! Repeated trials of a canonical workflow scenario driven by
//! [`crate::guest_loop`], graded through the pinned `scenario` crate
//! (hard assertions via `grade::hard_with`, semantic rubrics via the
//! [`Judge`] seam on the cursor backend), and persisted as a
//! `scenario::bundle` under `quality/runs/` — the same layout and
//! completeness validation the engine repo's orchestrator writes, so
//! reports stay comparable.
//!
//! This runner is the adapters repo's prompt/adapter iteration gate:
//! it consumes the linked adapter crates from the working tree and the
//! engine crates at their declared pin. Never CI: requires an
//! authenticated cursor-agent on `PATH`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::Arc;
use std::time::Instant;
use std::{env, fs};

use anyhow::{Context as _, Result, anyhow, ensure};
use clap::Parser;
use omnia::Backend as _;
use omnia_wasi_model::{self as wire, WasiModelCtx as _};
use scenario::bundle::Bundle;
use scenario::evaluate::semantic::{self, Judge, Rubrics};
use scenario::grade::{Evaluators, Execution};
use scenario::{
    AssertionId, Grading, ModelBackend, Outcome, Profile, RunMetadata, Runtime, Scenario,
    ScenarioReport, ScenarioReportVersion, TrialMetrics, TrialResult, catalog, evaluate,
};

use crate::model::LocalToolHost;
use crate::{guest_loop, verify};

/// `specify-dev quality` — the native-live runner's CLI face.
#[derive(Debug, Parser)]
#[command(name = "quality", about = "Run native-live quality trials and write a report bundle")]
struct Args {
    /// Live native profile id.
    #[arg(long, default_value = "native-live")]
    profile: String,
    /// Canonical scenario id.
    #[arg(long, default_value = guest_loop::SCENARIO)]
    scenario: String,
    /// Trial count override (defaults to the profile's declared count;
    /// the `TRIALS` environment variable wins over both).
    #[arg(long)]
    trials: Option<usize>,
}

/// Run repeated native-live trials and write the validated report
/// bundle; exit 0 only when every trial passed.
///
/// # Errors
///
/// Returns setup and bundle-write failures; assertion and rubric
/// failures are data on the report (and a non-zero exit).
pub async fn run(arguments: &[String]) -> Result<ExitCode> {
    let args = Args::parse_from(arguments);
    let scenario = catalog::load(&args.scenario)
        .map_err(|error| anyhow!("loading scenario `{}`: {error}", args.scenario))?;
    let profile = scenario
        .profiles
        .iter()
        .find(|profile| profile.id == args.profile)
        .with_context(|| {
            format!("scenario `{}` declares no `{}` profile", args.scenario, args.profile)
        })?;
    ensure!(
        profile.runtime == Runtime::Native && profile.model == ModelBackend::Live,
        "profile `{}` is not native-live; this runner owns the linked-adapter live loop only",
        args.profile
    );
    ensure!(
        env::var("SPECIFY_DEV_MODEL").as_deref() != Ok("replay"),
        "SPECIFY_DEV_MODEL=replay contradicts the live profile"
    );
    let judge = LiveJudge::connect().await.context(
        "cursor-agent not runnable; install it, then `cursor-agent login` or export \
         CURSOR_API_KEY",
    )?;
    let trials = env::var("TRIALS")
        .ok()
        .map(|value| value.parse::<usize>().context("TRIALS must be a number"))
        .transpose()?
        .or(args.trials)
        .unwrap_or(profile.trials);
    ensure!(trials > 0, "at least one trial is required");

    let started_at = jiff::Timestamp::now();
    let stamp = started_at.strftime("%Y%m%dT%H%M%SZ").to_string();
    let run_id = format!("{}-{}-{stamp}", args.scenario, args.profile);
    let bundle = Bundle::new(
        env::var_os("RUN_BUNDLE")
            .filter(|value| !value.is_empty())
            .map_or_else(|| repo_root().join("quality/runs").join(&run_id), PathBuf::from),
    );

    let rubrics =
        Rubrics::embedded().map_err(|error| anyhow!("parsing the rubric catalog: {error}"))?;
    let evaluators = Evaluators::default()
        .with(AssertionId::GuestJournalCadence, evaluate::guest::journal_cadence)
        .with(AssertionId::GuestGeneratedCrateVerifies, verify::generated_crates_verify);
    let setting = Setting {
        scenario: &scenario,
        profile,
        evaluators: &evaluators,
        rubrics: &rubrics,
        bundle: &bundle,
    };

    println!("== {run_id}: {trials} trial(s) ==");
    let mut results = Vec::new();
    for trial in 1..=trials {
        bundle.create_trial(trial).context("creating the trial directory")?;
        println!("== trial {trial}/{trials} ==");
        let started = Instant::now();
        let execution = drive_trial(&bundle, trial).await?;
        let duration = usize::try_from(started.elapsed().as_millis()).unwrap_or(usize::MAX);
        let result = grade(&setting, &execution, &judge, trial, duration).await?;
        println!("trial {trial}: {:?}", result.outcome);
        results.push(result);
    }

    let outcome = if results.iter().all(|trial| trial.outcome == Outcome::Pass) {
        Outcome::Pass
    } else {
        Outcome::Fail
    };
    let report = ScenarioReport {
        version: ScenarioReportVersion,
        scenario: args.scenario.clone(),
        outcome,
        run: metadata(run_id, &args, &judge, started_at)?,
        trials: results,
    };
    scenario::bundle::validate(&scenario, &report)
        .map_err(|error| anyhow!("report completeness: {error}"))?;
    let path = bundle.write_report(&report).context("writing the report")?;
    println!("{}", path.display());
    Ok(if outcome == Outcome::Pass { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

/// Drive one trial through the in-process guest loop and persist the
/// per-step transcript as the driver log.
async fn drive_trial(bundle: &Bundle, trial: usize) -> Result<Execution> {
    let sandbox = bundle.workspace(trial);
    let steps = guest_loop::drive(&sandbox).await?;
    let execution = Execution::new(&sandbox, steps);
    let mut transcript = String::new();
    for (id, step) in execution.steps() {
        use std::fmt::Write as _;
        let _ = writeln!(
            transcript,
            "==> {id} (exit {})\n{}{}",
            step.exit_code, step.stdout, step.stderr
        );
    }
    fs::write(bundle.driver_log(trial), transcript).context("writing the driver log")?;
    Ok(execution)
}

/// The run-constant grading inputs shared by every trial.
struct Setting<'a> {
    scenario: &'a Scenario,
    profile: &'a Profile,
    evaluators: &'a Evaluators,
    rubrics: &'a Rubrics,
    bundle: &'a Bundle,
}

/// Grade one completed execution and persist the per-trial artifacts —
/// the same profile-agnostic pass the engine orchestrator runs, over
/// the same pinned grading pipeline and bundle layout.
async fn grade(
    setting: &Setting<'_>, execution: &Execution, judge: &impl Judge, trial: usize,
    duration_ms: usize,
) -> Result<TrialResult> {
    let hard_assertions =
        scenario::grade::hard_with(setting.scenario, execution, setting.evaluators);

    let mut semantic_rubrics = Vec::new();
    let mut outputs: Vec<PathBuf> = vec!["driver.log".into()];
    if setting.profile.grading == Grading::Semantic {
        for rubric in &setting.scenario.semantic_rubrics {
            let graded =
                semantic::grade(rubric, setting.rubrics, execution.root(), judge).await;
            let verdict = setting.bundle.rubric_verdict(trial, rubric.id);
            fs::write(&verdict, &graded.raw).context("writing the rubric verdict")?;
            outputs.push(verdict.file_name().map(Into::into).unwrap_or_default());
            semantic_rubrics.push(graded.result);
        }
    }

    let missing_outputs = scenario::grade::missing_outputs(setting.scenario, execution);
    let passed = hard_assertions.iter().all(|result| result.outcome == Outcome::Pass)
        && semantic_rubrics.iter().all(|result| result.outcome == Outcome::Pass)
        && missing_outputs.is_empty();
    let result = TrialResult {
        trial,
        profile: setting.profile.id.clone(),
        outcome: if passed { Outcome::Pass } else { Outcome::Fail },
        hard_assertions,
        semantic_rubrics,
        missing_outputs,
        metrics: TrialMetrics {
            // The cursor backend exposes no token usage yet; keep the
            // counters stubbed and flagged unavailable.
            usage_available: false,
            input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
            duration_ms,
        },
        outputs,
    };
    setting.bundle.write_trial_result(&result).context("writing the trial result")?;
    Ok(result)
}

/// Assemble the run-level provenance and timing record: the adapters
/// checkout revision plus the declared engine pin, and a digest over
/// the embedded scenario document and rubric catalog.
fn metadata(
    run_id: String, args: &Args, judge: &LiveJudge, started_at: jiff::Timestamp,
) -> Result<RunMetadata> {
    let entry = catalog::CATALOG
        .iter()
        .find(|entry| entry.id == args.scenario)
        .context("scenario missing from the embedded catalog")?;
    let mut bytes = entry.yaml.as_bytes().to_vec();
    bytes.extend(semantic::CATALOG_YAML.as_bytes());
    Ok(RunMetadata {
        id: run_id,
        runner: format!("specify-dev quality {}", args.profile),
        revisions: BTreeMap::from([
            ("specify".to_owned(), engine_pin()?),
            ("specify-adapters".to_owned(), git_head(&repo_root())?),
        ]),
        model: Some(
            env::var("SPECIFY_EVAL_MODEL")
                .ok()
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| "cursor-default".to_owned()),
        ),
        judge_model: Some(judge.model_identity()),
        prompt_digest: Some(format!("sha256:{}", schema::digest::sha256_hex(&bytes))),
        // Native trials link adapter rlibs; no components are deployed.
        component_digests: BTreeMap::new(),
        started_at,
        completed_at: jiff::Timestamp::now(),
    })
}

/// The adapters repository root (this harness builds from checkout).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The declared engine pin, scanned from the embedded harness
/// manifest's `specify.git` dependency line.
fn engine_pin() -> Result<String> {
    include_str!("../Cargo.toml")
        .lines()
        .find(|line| line.contains("github.com/augentic/specify.git"))
        .and_then(|line| line.split("rev = \"").nth(1))
        .map(|rest| rest.chars().take_while(char::is_ascii_hexdigit).collect())
        .context("no engine pin found in the harness manifest")
}

fn git_head(repository: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repository)
        .output()
        .context("running git rev-parse")?;
    ensure!(
        output.status.success(),
        "git rev-parse failed in {}: {}",
        repository.display(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The verdict shape the judge must return; mirrors what
/// `scenario::evaluate::semantic` validates.
const VERDICT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "score": { "type": "integer", "minimum": 0, "maximum": 100 },
    "outcome": { "type": "string", "enum": ["pass", "fail"] },
    "detail": { "type": "string" }
  },
  "required": ["score", "outcome", "detail"],
  "additionalProperties": false
}"#;

/// Cursor-backed live judge — one completion per rubric, the trial
/// workspace lent through the minimal tool host.
struct LiveJudge {
    client: omnia_cursor::Client,
    model: Option<String>,
}

impl LiveJudge {
    /// Connect the cursor backend. The optional `SPECIFY_JUDGE_MODEL`
    /// override selects the judge's model independently of the
    /// subject model.
    async fn connect() -> Result<Self> {
        let client = omnia_cursor::Client::connect().await.context("connecting cursor-agent")?;
        Ok(Self {
            client,
            model: env::var("SPECIFY_JUDGE_MODEL").ok().filter(|id| !id.trim().is_empty()),
        })
    }

    /// The judge's model identity for `RunMetadata.judge_model`.
    fn model_identity(&self) -> String {
        self.model.clone().unwrap_or_else(|| "cursor-default".to_owned())
    }
}

impl Judge for LiveJudge {
    async fn judge(&self, prompt: String, workspace: &Path) -> Result<String, String> {
        let request = wire::Request {
            model: self.model.clone(),
            system: None,
            messages: vec![wire::Message {
                role: wire::Role::User,
                content: prompt,
            }],
            generation: None,
            format: wire::Format::Schema(wire::Schema {
                name: "verdict".to_owned(),
                schema: VERDICT_SCHEMA.to_owned(),
            }),
            tools: vec![],
            grants: wire::Grants {
                references: None,
                workspace: None,
                verify: vec![],
            },
        };
        let host = Arc::new(LocalToolHost {
            workspace: Some(workspace.to_owned()),
        });
        let answer =
            self.client.complete(request, host).await.map_err(|error| format!("{error:#}"))?;
        serde_json::to_string(&answer.value).map_err(|error| error.to_string())
    }
}
