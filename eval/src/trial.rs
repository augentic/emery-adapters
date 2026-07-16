//! Live-model trial: the operator rhythm over linked adapters, graded deterministically.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, io};

use anyhow::{Context as _, Result, ensure};
use change::Plan;
use clap::{Parser, Subcommand};
use eval::model::DevModel;
use eval::provider::Provider;
use eval::{fs as evalfs, mcp};
use omnia_guest::api::invoke::Invoker;
use project::config::Layout;

use eval::inputs::TrialInputs;
use eval::scenario;

use crate::grade;
use crate::telemetry::Telemetry;

#[derive(Debug, Parser)]
#[command(name = "eval", about = "Run the live-model trial over sandbox/eval")]
struct Args {
    #[command(subcommand)]
    phase: Option<Phase>,
}

#[derive(Clone, Debug, Subcommand)]
enum Phase {
    Init,
    Plan,
    Execute,
    Finalize,
    Clean,
    Scenario {
        /// `<adapter>/<scenario>` under `eval/scenarios/`.
        id: Option<String>,
    },
}

/// Run the trial from the CLI: one phase, or the full rhythm.
///
/// # Errors
///
/// Returns verb failures, grading failures, and sandbox I/O failures.
pub async fn run(argv: &[String]) -> Result<ExitCode> {
    let cli = Args::parse_from(argv);
    match cli.phase {
        Some(Phase::Init) => init().await?,
        Some(Phase::Plan) => plan().await?,
        Some(Phase::Execute) => execute().await?,
        Some(Phase::Finalize) => finalize().await?,
        Some(Phase::Clean) => clean()?,
        Some(Phase::Scenario { id }) => scenario::run(id.as_deref()).await?,
        None => {
            init().await?;
            plan().await?;
            execute().await?;
            finalize().await?;
            clean()?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

async fn init() -> Result<()> {
    let inputs = TrialInputs::load()?;
    let root = replace()?;
    println!("live trial project: {}", root.display());
    let _cache = eval::env::scoped_cache(&root);
    seed(&root)?;
    invoke(&root, &["init", "contracts", "--name", &inputs.project_name]).await?;
    Ok(())
}

async fn plan() -> Result<()> {
    let inputs = TrialInputs::load()?;
    let root = require()?;
    println!("live trial project: {}", root.display());
    let _cache = eval::env::scoped_cache(&root);
    let provider = provider(&root).await;

    invoke_with(
        &provider,
        &[
            "plan",
            "author",
            &inputs.change,
            "--intent",
            &inputs.intent,
            "--source",
            &inputs.source,
        ],
    )
    .await?;
    let authored = read_plan(&root)?;
    ensure!(!authored.entries.is_empty(), "plan author produced no entries");

    // Gate 1: the operator stamps `approved`.
    invoke_with(&provider, &["plan", "transition", &inputs.change, "approved"]).await?;

    report(&provider.model().counts(), authored.entries.len());
    Ok(())
}

async fn execute() -> Result<()> {
    let root = require()?;
    println!("live trial project: {}", root.display());
    let _cache = eval::env::scoped_cache(&root);
    let provider = provider(&root).await;

    invoke_with(&provider, &["plan", "execute"]).await?;

    let plan = read_plan(&root)?;
    grade::run(&root, &plan)?;
    report(&provider.model().counts(), plan.entries.len());
    Ok(())
}

async fn finalize() -> Result<()> {
    let root = require()?;
    println!("live trial project: {}", root.display());
    let _cache = eval::env::scoped_cache(&root);
    invoke(&root, &["plan", "archive"]).await?;
    Ok(())
}

fn clean() -> Result<()> {
    let root = root();
    if root.exists() {
        fs::remove_dir_all(&root).context("cleaning up the trial project")?;
    }
    Ok(())
}

async fn invoke(root: &Path, argv: &[&str]) -> Result<()> {
    invoke_with(&provider(root).await, argv).await
}

async fn invoke_with(provider: &Provider<Telemetry<DevModel>>, argv: &[&str]) -> Result<()> {
    eprintln!("==> specify {}", argv.join(" "));
    let router = transport::command::router(Invoker::new("specify", provider.clone()))
        .map_err(|error| anyhow::anyhow!("building the command router: {error}"))?;
    let mut full: Vec<String> = vec!["specify".to_string()];
    full.extend(argv.iter().map(ToString::to_string));
    let response = router.execute(full).await;
    drop(response.write_to(&mut io::stdout().lock(), &mut io::stderr().lock()));
    ensure!(response.exit == 0, "`specify {}` exited {}", argv.join(" "), response.exit);
    Ok(())
}

async fn provider(root: &Path) -> Provider<Telemetry<DevModel>> {
    let mut provider = Provider::new(root, Telemetry::new(DevModel::new(root)));
    if let Some(base) = mcp::ephemeral_base().await {
        provider = provider.mcp_base(base);
    }
    provider
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../sandbox/eval")
}

fn replace() -> Result<PathBuf> {
    let root = root();
    if root.exists() {
        fs::remove_dir_all(&root).context("replacing the previous trial project")?;
    }
    fs::create_dir_all(&root).context("creating the trial project root")?;
    root.canonicalize().context("canonical trial project root")
}

fn require() -> Result<PathBuf> {
    let root = root();
    ensure!(
        root.join(".specify/project.yaml").is_file(),
        "project is not initialised; run `cargo make eval init` first"
    );
    root.canonicalize().context("canonical trial project root")
}

fn seed(root: &Path) -> Result<()> {
    evalfs::copy_tree(&eval::inputs::seed_dir(), root)
}

fn read_plan(root: &Path) -> Result<Plan> {
    Plan::load(&Layout::new(root).plan_path()).context("loading plan.yaml")
}

// Requests beyond one per leg invocation are repairs — the early signal that a
// prompt or answer-schema change degraded the model's first answer.
fn report(counts: &std::collections::BTreeMap<String, usize>, slices: usize) {
    for (leg, requests) in counts {
        match leg.as_str() {
            "proposal" => {
                let repairs = requests.saturating_sub(1);
                eprintln!("leg proposal: {requests} request(s), {repairs} repair(s)");
            }
            "synthesis" => {
                let repairs = requests.saturating_sub(slices);
                eprintln!(
                    "leg synthesis: {requests} request(s) over {slices} slice(s), \
                     {repairs} repair(s)"
                );
            }
            other => eprintln!("leg {other}: {requests} request(s)"),
        }
    }
}
