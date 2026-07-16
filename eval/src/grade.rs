//! Deterministic structural checks after execute, before finalize.

use std::fs;
use std::path::Path;

use anyhow::{Context as _, Result, ensure};
use artifacts::spec::provenance::{Requirement, RequirementStatus, parse_spec_md};
use change::{Plan, Status};
use contracts::validate::validate_baseline;
use project::config::Layout;

/// Grade the drained plan against the contracts-bound trial contract.
///
/// # Errors
///
/// Returns one failing assertion at a time, with the evidence inline.
pub fn run(root: &Path, plan: &Plan) -> Result<()> {
    lifecycle(plan)?;
    requirements(&baseline(root)?)?;
    contracts_baseline(root)?;
    Ok(())
}

fn lifecycle(plan: &Plan) -> Result<()> {
    ensure!(
        plan.entries.iter().all(|entry| entry.status == Status::Done),
        "execute must leave every entry done: {:?}",
        plan.entries
    );
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
