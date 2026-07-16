//! Live-model trial: the operator rhythm over linked adapters, graded deterministically.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context as _, Result, ensure};
use clap::{Parser, Subcommand};
use harness::model::DevModel;
use harness::provider::Provider;
use harness::telemetry::{self, Telemetry};
use harness::{command, fs as evalfs, mcp, sandbox};
use specify_dev::inputs::{self, TrialInputs};
use specify_dev::{catalog, scenario};

use crate::grade;

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
    let root = sandbox::replace(&root())?;
    println!("live trial project: {}", root.display());
    let _cache = harness::env::scoped_cache(&root);
    seed(&root)?;
    invoke(&root, &["init", "contracts", "--name", &inputs.project_name]).await?;
    Ok(())
}

async fn plan() -> Result<()> {
    let inputs = TrialInputs::load()?;
    let root = sandbox::require(&root())?;
    println!("live trial project: {}", root.display());
    let _cache = harness::env::scoped_cache(&root);
    let provider = provider(&root).await;

    command::invoke(
        &provider,
        &["plan", "author", &inputs.change, "--intent", &inputs.intent, "--source", &inputs.source],
    )
    .await?;
    let authored = sandbox::read_plan(&root)?;
    ensure!(!authored.entries.is_empty(), "plan author produced no entries");

    // Gate 1: the operator stamps `approved`.
    command::invoke(&provider, &["plan", "transition", &inputs.change, "approved"]).await?;

    telemetry::report(&provider.model().counts(), authored.entries.len());
    Ok(())
}

async fn execute() -> Result<()> {
    let root = sandbox::require(&root())?;
    println!("live trial project: {}", root.display());
    let _cache = harness::env::scoped_cache(&root);
    let provider = provider(&root).await;

    command::invoke(&provider, &["plan", "execute"]).await?;

    let plan = sandbox::read_plan(&root)?;
    grade::run(&root, &plan)?;
    telemetry::report(&provider.model().counts(), plan.entries.len());
    Ok(())
}

async fn finalize() -> Result<()> {
    let root = sandbox::require(&root())?;
    println!("live trial project: {}", root.display());
    let _cache = harness::env::scoped_cache(&root);
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
    command::invoke(&provider(root).await, argv).await
}

async fn provider(root: &Path) -> Provider<Telemetry<DevModel>> {
    let catalog = catalog::catalog();
    let base = mcp::ephemeral_base(&catalog).await;
    let mut provider = Provider::new(root, Telemetry::new(DevModel::new(root)), catalog);
    if let Some(base) = base {
        provider = provider.mcp_base(base);
    }
    provider
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sandbox/eval")
}

fn seed(root: &Path) -> Result<()> {
    evalfs::copy_tree(&inputs::seed_dir(), root)
}
