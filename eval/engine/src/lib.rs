//! `engine` — the first-party adapters linked into the shared
//! native harness, plus the repository's trial declaration and its
//! trial and scenario data locators.

use std::fs;
use std::path::Path;

use anyhow::{Result, ensure};
use captures::Captures;
use change::Plan;
use change::plan::handlers::ExecuteBody;
use contracts::Contracts;
use contracts::validate::validate_baseline;
use documentation::Documentation;
use harness::catalog::{Binding, Catalog};
use harness::entry::Shell;
use harness::inputs::TrialInputs;
use harness::scenario::Scenarios;
use harness::trial::Profile;
use intent::Intent;
use omnia_guest::Model;
use omnia_target::Omnia;
use screenshots::Screenshots;
use typescript::Typescript;
use vectis::Vectis;

/// Every first-party adapter linked into `engine`.
///
/// One builder call per adapter: the harness monomorphizes each
/// implementor's operation legs behind compile-checked trait bounds, so
/// adding an adapter is one line here plus its Cargo dependency.
#[must_use]
pub fn catalog<M: Model>() -> Catalog<M> {
    Catalog::builder()
        .source::<Captures>()
        .target::<Contracts>()
        .source::<Documentation>()
        .source::<Intent>()
        .target::<Omnia>()
        .source::<Screenshots>()
        .source::<Typescript>()
        .target::<Vectis>()
        .build()
}

/// The adapter binding handed to the shared harness entrypoints.
#[derive(Clone, Copy, Debug)]
pub struct Adapters;

impl Binding for Adapters {
    fn catalog<M: Model>() -> Catalog<M> {
        catalog()
    }
}

/// The checked-in shared trial definition (`examples/change/trial.env`).
pub const TRIAL_ENV: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/change/trial.env");

/// The shared seed tree both rungs copy into their sandbox.
pub const SEED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/change/seed");

/// The committed prompt-scenario root (`eval/scenarios/`).
pub const SCENARIOS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../scenarios");

/// The sandbox root: the trial project, and the parent of scenario
/// scratch trees (`sandbox/<adapter>/<name>/run-…`).
pub const SANDBOX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sandbox");

/// The wrapper declaration the shared entry runs.
pub const SHELL: Shell = Shell {
    name: "engine",
    profile,
};

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

// Grade the drained plan against the contracts-bound trial contract:
// the generic drained/done invariants are the harness driver's; this
// hook owns the contracts-specific baseline checks only, returning
// one failing assertion at a time with the evidence inline.
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
