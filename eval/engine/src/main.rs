//! `engine` — native dev shim and eval harness: the shared
//! harness entrypoints over the first-party catalog. Three modes:
//! CLI (default), HTTP (`serve`), and live-model eval (`eval`).

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Result, ensure};
use change::Plan;
use change::plan::handlers::ExecuteBody;
use contracts::validate::validate_baseline;
use engine::{Adapters, SANDBOX, SCENARIOS, SEED, TRIAL_ENV};
use harness::inputs::TrialInputs;
use harness::scenario::Scenarios;
use harness::trial::{self, Profile};
use harness::{command, http};

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    match argv.get(1).map(String::as_str) {
        Some("serve") => report(http::serve::<Adapters>(&argv[1..]).await),
        Some("eval") => report(eval(&argv[1..]).await),
        _ => ExitCode::from(command::run::<Adapters>(argv).await),
    }
}

async fn eval(argv: &[String]) -> Result<ExitCode> {
    trial::run::<Adapters>(&profile()?, argv).await
}

fn report(outcome: Result<ExitCode>) -> ExitCode {
    outcome.unwrap_or_else(|err| {
        eprintln!("engine: {err:#}");
        ExitCode::FAILURE
    })
}

// The adapters-repo trial: the contracts-bound change over the shared
// `examples/change/trial.env` inputs and seed tree, graded by the
// contracts baseline checks.
fn profile() -> Result<Profile> {
    let inputs = TrialInputs::load(Path::new(TRIAL_ENV))?;
    Ok(Profile {
        sandbox: SANDBOX.into(),
        seed: Some(SEED.into()),
        init: argv(&["init", "contracts", "--name", &inputs.project_name]),
        author: argv(&[
            "plan",
            "author",
            &inputs.change,
            "--intent",
            &inputs.intent,
            "--source",
            &inputs.source,
        ]),
        change: inputs.change,
        authored: None,
        grade,
        scenarios: Some(Scenarios {
            dir: SCENARIOS.into(),
        }),
    })
}

fn argv(parts: &[&str]) -> Vec<String> {
    parts.iter().map(ToString::to_string).collect()
}

/// Grade the drained plan against the contracts-bound trial contract.
/// The generic drained/done invariants are the harness driver's; this
/// hook owns the contracts-specific baseline checks only.
///
/// # Errors
///
/// Returns one failing assertion at a time, with the evidence inline.
fn grade(root: &Path, _plan: &Plan, _executed: &ExecuteBody) -> Result<()> {
    harness::grade::provenance(&harness::grade::baseline(root)?)?;
    contracts_baseline(root)?;
    Ok(())
}

fn contracts_baseline(root: &Path) -> Result<()> {
    let contracts = root.join("contracts");
    ensure!(
        yaml_count(&contracts) > 0,
        "the merged contracts baseline at {} carries no .yaml contract",
        contracts.display()
    );
    let findings = validate_baseline(&contracts);
    ensure!(
        findings.is_empty(),
        "the merged contracts baseline has validator findings: {}",
        findings
            .iter()
            .map(|finding| {
                format!("[{}] {}: {}", finding.rule_id, finding.path.display(), finding.detail)
            })
            .collect::<Vec<_>>()
            .join("; ")
    );
    Ok(())
}

fn yaml_count(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                yaml_count(&path)
            } else {
                usize::from(path.extension().is_some_and(|ext| ext == "yaml" || ext == "yml"))
            }
        })
        .sum()
}
