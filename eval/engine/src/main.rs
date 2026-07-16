//! `engine` — native dev shim and eval harness: the shared
//! harness entrypoints over the first-party catalog. Three modes:
//! CLI (default), HTTP (`serve`), and live-model eval (`eval`).

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use anyhow::{Context as _, Result, ensure};
use artifacts::spec::provenance::{Requirement, RequirementStatus, parse_spec_md};
use change::Plan;
use change::plan::handlers::ExecuteBody;
use contracts::validate::validate_baseline;
use engine::{FirstParty, eval_sandbox, scenarios_dir, scenarios_sandbox, seed_dir, trial_env};
use harness::inputs::TrialInputs;
use harness::scenario::Scenarios;
use harness::trial::{self, Profile};
use harness::{command, http};
use project::config::Layout;

#[tokio::main]
async fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    match argv.get(1).map(String::as_str) {
        Some("serve") => report(http::serve::<FirstParty>(&argv[1..]).await),
        Some("eval") => report(eval(&argv[1..]).await),
        _ => ExitCode::from(command::run::<FirstParty>(argv).await),
    }
}

async fn eval(argv: &[String]) -> Result<ExitCode> {
    trial::run::<FirstParty>(&profile()?, argv).await
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
    let inputs = TrialInputs::load(&trial_env())?;
    Ok(Profile {
        sandbox: eval_sandbox(),
        seed: Some(seed_dir()),
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
            dir: scenarios_dir(),
            sandbox: scenarios_sandbox(),
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
    requirements(&baseline(root)?)?;
    contracts_baseline(root)?;
    Ok(())
}

fn requirements(requirements: &[Requirement]) -> Result<()> {
    ensure!(!requirements.is_empty(), "the baseline carries no requirements");
    for requirement in requirements {
        ensure!(!requirement.id.is_empty(), "requirement `{}` carries no id", requirement.name);
        if requirement.status != Some(RequirementStatus::Unknown) {
            ensure!(
                !requirement.sources.is_empty(),
                "evidenced requirement `{}` carries no provenance",
                requirement.name
            );
        }
    }
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

fn baseline(root: &Path) -> Result<Vec<Requirement>> {
    let mut requirements = Vec::new();
    let specs = Layout::new(root).specs_dir();
    for domain in fs::read_dir(&specs)
        .with_context(|| format!("reading the baseline specs dir {}", specs.display()))?
    {
        let spec = domain.context("domain dir")?.path().join("spec.md");
        if spec.is_file() {
            let body = fs::read_to_string(&spec).context("reading a baseline spec")?;
            requirements.extend(parse_spec_md(&body).requirements);
        }
    }
    Ok(requirements)
}
