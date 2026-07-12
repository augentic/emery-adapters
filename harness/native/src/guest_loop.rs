//! In-process driver for the canonical `guest-execute-loop` scenario.
//!
//! Executes the scenario's workflow steps — `plan author` → the
//! operator's `approved` stamp → `plan execute` — through the shared
//! typed command router against one native [`Provider`], exactly the
//! dispatch surface the `specify-dev` CLI serves, with the model bound
//! by [`DevModel::from_env`] (live cursor-agent unless
//! `SPECIFY_DEV_MODEL=replay`). The driver owns execution only:
//! grading stays with the caller, against the captured step results.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, io};

use anyhow::{Context as _, Result, anyhow};
use clap::Parser;
use omnia_guest::api::command::Router;
use omnia_guest::api::invoke::Invoker;
use serde::Serialize;
use transport::command::Globals;

use crate::model::DevModel;
use crate::provider::Provider;
use crate::mcp;

/// The canonical scenario this driver executes.
pub const SCENARIO: &str = "guest-execute-loop";

/// Captured result of one driven step, in execution order.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct StepOutcome {
    /// Scenario workflow step id (`init` for the clerical seed step).
    pub id: String,
    /// Process-equivalent exit code.
    pub exit_code: i32,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
}

/// Drive the full loop inside `sandbox` (created when absent) and
/// return every captured step. Driving stops at the first failing
/// step; the failure stays in the returned steps for grading.
///
/// # Errors
///
/// Returns setup errors only — an unloadable scenario, an unusable
/// sandbox, or a router that cannot be built. Step failures are data,
/// not errors.
pub async fn drive(sandbox: &Path) -> Result<Vec<StepOutcome>> {
    let scenario = scenario::catalog::load(SCENARIO)
        .map_err(|error| anyhow!("loading the canonical scenario: {error}"))?;
    fs::create_dir_all(sandbox)
        .with_context(|| format!("creating the sandbox at {}", sandbox.display()))?;
    let sandbox = sandbox.canonicalize().context("resolving the sandbox root")?;

    let model = DevModel::from_env(&sandbox)?;
    let mut provider = Provider::new(sandbox, model);
    if let Some(base) = mcp::ephemeral_base().await {
        provider = provider.mcp_base(base);
    }
    let router = transport::command::router(Invoker::new("specify", provider))
        .map_err(|error| anyhow!("building the command router: {error}"))?;

    let mut steps = Vec::new();
    // The clerical seed the scenario presumes: a scaffolded project
    // bound to the omnia target through the linked-crate catalog.
    let init = ["specify", "init", "omnia", "--name", "demo", "--scaffold-only"]
        .map(str::to_owned)
        .to_vec();
    if !execute(&router, "init", init, &mut steps).await {
        return Ok(steps);
    }
    for step in &scenario.workflow {
        let argv = step.argv().map_err(|error| anyhow!("{error}"))?;
        if !execute(&router, &step.id, argv, &mut steps).await {
            break;
        }
    }
    Ok(steps)
}

/// Execute one argv through the router, record the outcome, and
/// report whether the step succeeded.
async fn execute(
    router: &Router<Provider<DevModel>, Globals>, id: &str, argv: Vec<String>,
    steps: &mut Vec<StepOutcome>,
) -> bool {
    eprintln!("==> {}", argv.join(" "));
    let response = router.execute(argv).await;
    let outcome = StepOutcome {
        id: id.to_owned(),
        exit_code: i32::from(response.exit),
        stdout: String::from_utf8_lossy(&response.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&response.stderr).into_owned(),
    };
    eprint!("{}", outcome.stdout);
    eprint!("{}", outcome.stderr);
    let succeeded = outcome.exit_code == 0;
    steps.push(outcome);
    succeeded
}

/// `specify-dev guest-loop` — the driver's CLI face.
#[derive(Debug, Parser)]
#[command(name = "guest-loop", about = "Drive the guest-execute-loop scenario in-process")]
struct Args {
    /// Sandbox project root (created when absent).
    #[arg(long)]
    sandbox: PathBuf,
}

/// Run the driver from the CLI: progress on stderr, the captured step
/// array as JSON on stdout, exit 0 only when every step succeeded.
///
/// # Errors
///
/// Returns the same setup errors as [`drive`], plus serialisation and
/// stdout I/O failures.
pub async fn run(argv: &[String]) -> Result<ExitCode> {
    let options = Args::parse_from(argv);
    let steps = drive(&options.sandbox).await?;
    let body = serde_json::to_string_pretty(&steps).context("serialising the step results")?;
    io::Write::write_all(&mut io::stdout().lock(), body.as_bytes())
        .context("writing the step results")?;
    println!();
    let drained = steps.iter().all(|step| step.exit_code == 0);
    Ok(if drained { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
