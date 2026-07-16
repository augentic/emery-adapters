//! Single-operation prompt scenarios: the fast prompt-iteration rung.
//!
//! `specify-dev eval scenario <adapter>/<name>` drives one adapter
//! operation — a `build`, or one `merge` gate — end-to-end against the
//! live cursor backend, natively through the same seam [`Provider`]
//! the trial uses. Each scenario is a data directory under
//! `eval/scenarios/<adapter>/<name>/`:
//!
//! - `scenario.toml` — the routing: the axis-qualified adapter id, the
//!   operation, and the slice name.
//! - `inputs/*.md` — the typed slice inputs, mapped by file stem
//!   (`proposal` / `design` / `tasks` / `spec*`; anything else rides
//!   as `other`).
//! - `seed/**` — files copied into the scratch project root.
//!
//! Every run seeds a fresh scratch tree under the gitignored
//! `sandbox/scenarios/<adapter>/<name>/run-<stamp>/`, dispatches the
//! operation, writes `report.json` beside the scratch delta, and fails
//! on a failing report. Adding a scenario is a directory, not Rust —
//! third-party adapters get the same rung by dropping one in.
//!
//! `SPECIFY_EVAL_MODEL=<model-id>` overrides the model, exactly as in
//! the trial. Prompt edits under `prose/` rebuild natively in seconds,
//! so there is no overlay mode here.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail, ensure};
use eval::model::DevModel;
use eval::provider::Provider;
use eval::{fs as evalfs, mcp};
use project::seam::wire::{BuildReport, BuildStatus};
use project::seam::{Input, MergePhase, Target as _, WorkingTree};
use serde::Deserialize;

/// One scenario's machine-readable routing, from `scenario.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    /// Axis-qualified adapter id (`target:contracts`).
    adapter: String,
    /// The seam operation the scenario drives.
    operation: Operation,
    /// The slice name the operation runs under.
    slice: String,
}

/// The closed operation set a scenario may drive.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Operation {
    Build,
    MergePreflight,
    MergePostflight,
}

impl Operation {
    const fn label(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::MergePreflight => "merge-preflight",
            Self::MergePostflight => "merge-postflight",
        }
    }
}

/// Run one scenario by `<adapter>/<name>` id, or list them all.
///
/// # Errors
///
/// Returns an unknown or malformed scenario, seeding failures, and a
/// failing adapter report.
pub async fn run(id: Option<&str>) -> Result<()> {
    let Some(id) = id else {
        return list();
    };
    let dir = scenarios_dir().join(id);
    let config = load(&dir).with_context(|| format!("scenario `{id}`"))?;

    let scratch = seed(id, &dir)?;
    println!(
        "eval scenario {id}: {} `{}` slice={} scratch={}",
        config.operation.label(),
        config.adapter,
        config.slice,
        scratch.display()
    );

    let provider = provider(&scratch).await;
    let inputs = inputs(&dir.join("inputs"))?;
    let report = dispatch(&provider, &config, inputs).await?;

    let outcome = match report.status {
        BuildStatus::Success => "pass",
        BuildStatus::Failure => "fail",
    };
    for finding in &report.findings {
        eprintln!(
            "finding [{}] {}: {}",
            format!("{:?}", finding.severity).to_lowercase(),
            finding.rule_id.as_deref().unwrap_or("-"),
            finding.title
        );
    }
    let report_path = scratch.join("report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&envelope(id, outcome, &report)?)?)?;
    println!("eval scenario {id}: report {}", report_path.display());

    ensure!(
        report.status == BuildStatus::Success,
        "scenario `{id}` failed; report at {}, delta under {}",
        report_path.display(),
        scratch.display()
    );
    Ok(())
}

/// Dispatch the configured operation through the seam provider.
async fn dispatch(
    provider: &Provider<DevModel>, config: &Config, inputs: Vec<Input>,
) -> Result<BuildReport> {
    let tree = WorkingTree {
        base: "eval".to_string(),
        subpath: None,
    };
    let adapter = config.adapter.clone();
    let slice = config.slice.clone();
    let report = match config.operation {
        Operation::Build => provider.build(adapter, slice, inputs, tree).await,
        Operation::MergePreflight => {
            provider.merge(adapter, slice, MergePhase::Preflight, tree).await
        }
        Operation::MergePostflight => {
            provider.merge(adapter, slice, MergePhase::Postflight, tree).await
        }
    };
    report.map_err(|error| anyhow::anyhow!("{} failed: {error:?}", config.operation.label()))
}

/// List every scenario directory carrying a `scenario.toml`.
fn list() -> Result<()> {
    let mut ids = ids()?;
    ids.sort();
    ensure!(!ids.is_empty(), "no scenarios under {}", scenarios_dir().display());
    println!("scenarios (run with `specify-dev eval scenario <id>`):");
    for id in ids {
        println!("  {id}");
    }
    Ok(())
}

/// Every `<adapter>/<name>` id under the scenarios root.
fn ids() -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for adapter in read_dirs(&scenarios_dir())? {
        let name = adapter.file_name().unwrap_or_default().to_string_lossy().into_owned();
        for scenario in read_dirs(&adapter)? {
            if scenario.join("scenario.toml").is_file() {
                let id = scenario.file_name().unwrap_or_default().to_string_lossy();
                ids.push(format!("{name}/{id}"));
            }
        }
    }
    Ok(ids)
}

fn read_dirs(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    Ok(dirs)
}

/// Parse the scenario's `scenario.toml`; a missing file lists the
/// known ids in the error.
fn load(dir: &Path) -> Result<Config> {
    let path = dir.join("scenario.toml");
    if !path.is_file() {
        bail!(
            "no scenario.toml at {}; known scenarios: {}",
            path.display(),
            ids().unwrap_or_default().join(", ")
        );
    }
    let body = fs::read_to_string(&path)?;
    toml::from_str(&body).with_context(|| format!("parsing {}", path.display()))
}

/// Seed a fresh scratch project tree under the gitignored sandbox:
/// `seed/**` copied to the root, retained after the run for review.
fn seed(id: &str, dir: &Path) -> Result<PathBuf> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let scratch = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sandbox/scenarios")
        .join(id)
        .join(format!("run-{stamp}"));
    fs::create_dir_all(&scratch)?;
    let seed = dir.join("seed");
    if seed.is_dir() {
        evalfs::copy_tree(&seed, &scratch)?;
    }
    scratch.canonicalize().context("canonical scratch root")
}

/// Read every `inputs/*.md` (sorted by name for a deterministic prompt
/// order) into its typed input by file stem.
fn inputs(dir: &Path) -> Result<Vec<Input>> {
    let entries = fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    paths.sort();
    ensure!(!paths.is_empty(), "no `inputs/*.md` under {}", dir.display());

    let mut inputs = Vec::new();
    for path in paths {
        let body =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
        inputs.push(match stem {
            "proposal" => Input::Proposal(body),
            "design" => Input::Design(body),
            "tasks" => Input::Tasks(body),
            stem if stem.starts_with("spec") => Input::Spec(body),
            _ => Input::Other(body),
        });
    }
    Ok(inputs)
}

/// The live provider anchored at the scratch tree, with the MCP
/// shelves on an ephemeral listener when a port can be bound.
async fn provider(scratch: &Path) -> Provider<DevModel> {
    let mut provider = Provider::new(scratch, DevModel::new(scratch));
    if let Some(base) = mcp::ephemeral_base().await {
        provider = provider.mcp_base(base);
    }
    provider
}

/// The persisted run summary: the scenario id, the outcome, and the
/// full seam report.
fn envelope(id: &str, outcome: &str, report: &BuildReport) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "version": 1,
        "scenario": id,
        "profile": "adapter-live",
        "runtime": "native",
        "model": std::env::var("SPECIFY_EVAL_MODEL").unwrap_or_else(|_| "cursor-default".to_owned()),
        "outcome": outcome,
        "report": serde_json::to_value(report)?,
    }))
}

fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios")
}
