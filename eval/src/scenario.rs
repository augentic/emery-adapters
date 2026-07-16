//! Single-operation prompt scenarios: the fast prompt-iteration rung.
//!
//! `specify-dev eval scenario <adapter>/<name>` drives one adapter
//! operation — a `build`, or one `merge` gate — end-to-end against the
//! live cursor backend, natively through the same seam [`Provider`]
//! the trial uses. Each scenario is a data directory under
//! `eval/scenarios/<adapter>/<name>/`:
//!
//! - `scenario.toml` — the routing: the axis-qualified adapter id, the
//!   operation, the slice name, and the `expect` artifact-exists gate
//!   (mandatory and non-empty for `build` scenarios).
//! - `inputs/*.md` — the typed slice inputs, mapped by file stem
//!   (`proposal` / `design` / `tasks` / `spec*`; anything else rides
//!   as `other`).
//! - `seed/**` — files copied into the scratch project root.
//!
//! Every run seeds a fresh scratch tree under the gitignored
//! `sandbox/scenarios/<adapter>/<name>/` (collision-proof `run-*`
//! directories), pins the project cache inside it, dispatches the
//! operation, writes `report.json` beside the scratch delta, and fails
//! on a failing report or a missing `expect` artifact. The persisted
//! `outcome` is `pass` only when the report *and* the artifact gate
//! both pass. Adding a scenario is a directory plus, for a new
//! adapter, its Cargo dependency and [`crate::catalog`] entry —
//! configuration alone cannot link a Rust crate.
//!
//! This module lives in the library (not the `specify-dev` binary) so
//! the model-free wiring tests share the same config parser and
//! validator as the runner.
//!
//! `SPECIFY_EVAL_MODEL=<model-id>` overrides the model, exactly as in
//! the trial. Prompt edits under `prose/` rebuild natively in seconds,
//! so there is no overlay mode here.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail, ensure};
use project::seam::wire::{BuildReport, BuildStatus};
use project::seam::{Input, MergePhase, Target as _, WorkingTree};
use serde::Deserialize;

use crate::model::DevModel;
use crate::provider::Provider;
use crate::{catalog, env, fs as evalfs, mcp};

/// One scenario's machine-readable routing, from `scenario.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Axis-qualified adapter id (`target:contracts`).
    pub adapter: String,
    /// The seam operation the scenario drives.
    pub operation: Operation,
    /// The slice name the operation runs under.
    pub slice: String,
    /// Scratch-relative paths that must exist (a directory satisfies
    /// its entry when it holds at least one file) after a passing
    /// report — the deterministic artifact-exists gate over the
    /// scenario's expected artifacts. A success report that produced
    /// none of them is a silent no-op, and fails here. Mandatory and
    /// non-empty for `build` scenarios.
    #[serde(default)]
    pub expect: Vec<String>,
}

/// The closed operation set a scenario may drive.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Operation {
    /// The target build operation.
    Build,
    /// The merge preflight gate.
    MergePreflight,
    /// The merge postflight gate.
    MergePostflight,
}

impl Operation {
    /// Kebab-case operation label for run output.
    #[must_use]
    pub const fn label(self) -> &'static str {
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
/// Returns an unknown or malformed scenario, seeding failures, a
/// failing adapter report, and a missing `expect` artifact.
pub async fn run(id: Option<&str>) -> Result<()> {
    let Some(id) = id else {
        return list();
    };
    let dir = scenarios_dir().join(id);
    let config = load(&dir).with_context(|| format!("scenario `{id}`"))?;

    let scratch = seed(id, &dir)?;
    let _cache = env::scoped_cache(&scratch);
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
    conclude(id, &scratch, &report, &config.expect)
}

/// Parse and validate the scenario config at `dir/scenario.toml`; a
/// missing file lists the known ids in the error.
///
/// Validation is the same gate the runner applies before spending a
/// model request: the adapter must be linked into the native shim, the
/// slice name must be non-empty, `build` scenarios must declare at
/// least one `expect` artifact, and every `expect` entry must be a
/// plain relative path (no absolute paths, no `..`).
///
/// # Errors
///
/// Returns a missing or unparseable `scenario.toml` and any validation
/// failure above.
pub fn load(dir: &Path) -> Result<Config> {
    let path = dir.join("scenario.toml");
    if !path.is_file() {
        bail!(
            "no scenario.toml at {}; known scenarios: {}",
            path.display(),
            ids().unwrap_or_default().join(", ")
        );
    }
    let body = fs::read_to_string(&path)?;
    let config: Config =
        toml::from_str(&body).with_context(|| format!("parsing {}", path.display()))?;
    validate(&config)?;
    Ok(config)
}

/// The shared config gate behind [`load`].
fn validate(config: &Config) -> Result<()> {
    ensure!(
        catalog::entries().iter().any(|entry| entry.id() == config.adapter),
        "adapter `{}` is not linked into the native shim",
        config.adapter
    );
    ensure!(!config.slice.trim().is_empty(), "empty slice name");
    if config.operation == Operation::Build {
        ensure!(
            !config.expect.is_empty(),
            "build scenarios must declare at least one `expect` artifact — a success \
             report that produced nothing would otherwise pass as a silent no-op"
        );
    }
    for rel in &config.expect {
        validate_entry(rel).with_context(|| format!("expect entry `{rel}`"))?;
    }
    Ok(())
}

/// One `expect` entry must be a plain relative path: non-empty, not
/// absolute, and free of `..` / `.` components, so it cannot name
/// anything outside the scratch root.
fn validate_entry(rel: &str) -> Result<()> {
    ensure!(!rel.trim().is_empty(), "empty expect entry");
    let path = Path::new(rel);
    ensure!(path.is_relative(), "absolute paths are not allowed");
    ensure!(
        path.components().all(|component| matches!(component, Component::Normal(_))),
        "path components must be plain names (no `..` or `.`)"
    );
    Ok(())
}

/// Gate and persist one run's outcome.
///
/// Prints the findings, applies the artifact-exists gate to a passing
/// report, writes `report.json` with the final `outcome`, then fails
/// on a failing report or a missing artifact. `outcome: pass` is
/// persisted only after the report and the artifact expectations both
/// pass.
///
/// # Errors
///
/// Returns a failing adapter report, a failed artifact expectation,
/// and report-persistence I/O failures.
pub fn conclude(id: &str, scratch: &Path, report: &BuildReport, expect: &[String]) -> Result<()> {
    for finding in &report.findings {
        eprintln!(
            "finding [{}] {}: {}",
            format!("{:?}", finding.severity).to_lowercase(),
            finding.rule_id.as_deref().unwrap_or("-"),
            finding.title
        );
    }

    let gate = if report.status == BuildStatus::Success {
        enforce_expected(id, scratch, expect)
    } else {
        Ok(())
    };
    let outcome =
        if report.status == BuildStatus::Success && gate.is_ok() { "pass" } else { "fail" };

    let report_path = scratch.join("report.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&envelope(id, outcome, report)?)?)?;
    println!("eval scenario {id}: report {}", report_path.display());

    ensure!(
        report.status == BuildStatus::Success,
        "scenario `{id}` failed; report at {}, delta under {}",
        report_path.display(),
        scratch.display()
    );
    gate
}

/// The artifact-exists gate.
///
/// Every `expect` path is present in the scratch tree (directories
/// must hold at least one file). Paths that resolve outside the
/// scratch root — via absolute entries, `..`, or symlinks — never
/// satisfy an entry, and directory walks are cycle-safe.
///
/// # Errors
///
/// Returns the first unsatisfied entry, naming the missing path.
pub fn enforce_expected(id: &str, scratch: &Path, expect: &[String]) -> Result<()> {
    let root = scratch.canonicalize().context("canonical scratch root")?;
    for rel in expect {
        validate_entry(rel).with_context(|| format!("expect entry `{rel}`"))?;
        let satisfied = confined(&root, &root.join(rel)).is_some_and(|path| {
            if path.is_dir() {
                holds_a_file(&root, &path, &mut HashSet::new())
            } else {
                path.is_file()
            }
        });
        ensure!(
            satisfied,
            "scenario `{id}` reported success but produced no `{rel}` under {} — \
             a silent no-op (every sub-flow self-skipped, or the writes landed \
             elsewhere)",
            root.display()
        );
    }
    Ok(())
}

/// Canonicalize `path` and keep it only when it stays under `root`,
/// so a symlink escaping the scratch tree never satisfies a gate.
fn confined(root: &Path, path: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    canonical.starts_with(root).then_some(canonical)
}

/// Whether the canonical `dir` transitively holds at least one file
/// confined to `root`. `visited` carries the canonical directories
/// already walked, so symlink cycles terminate.
fn holds_a_file(root: &Path, dir: &Path, visited: &mut HashSet<PathBuf>) -> bool {
    if !visited.insert(dir.to_path_buf()) {
        return false;
    }
    fs::read_dir(dir).into_iter().flatten().flatten().any(|entry| {
        confined(root, &entry.path()).is_some_and(|path| {
            path.is_file() || (path.is_dir() && holds_a_file(root, &path, visited))
        })
    })
}

/// Atomically allocate a fresh run directory under `base`.
///
/// Names are collision-proof — `run-<stamp>-<pid>`, suffixed with a
/// counter when taken — so concurrent or same-second runs never reuse
/// an earlier run's tree (and never accept its stale artifacts).
///
/// # Errors
///
/// Returns directory-creation failures and clock errors.
pub fn allocate_run_dir(base: &Path) -> Result<PathBuf> {
    fs::create_dir_all(base).with_context(|| format!("creating {}", base.display()))?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let pid = std::process::id();
    for attempt in 0..u32::MAX {
        let name = if attempt == 0 {
            format!("run-{stamp}-{pid}")
        } else {
            format!("run-{stamp}-{pid}-{attempt}")
        };
        let candidate = base.join(name);
        match fs::create_dir(&candidate) {
            Ok(()) => return candidate.canonicalize().context("canonical run dir"),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => {
                return Err(err).with_context(|| format!("creating {}", candidate.display()));
            }
        }
    }
    bail!("could not allocate a unique run directory under {}", base.display())
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

/// Seed a fresh scratch project tree under the gitignored sandbox:
/// `seed/**` copied into a collision-proof run directory, retained
/// after the run for review.
fn seed(id: &str, dir: &Path) -> Result<PathBuf> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../sandbox/scenarios").join(id);
    let scratch = allocate_run_dir(&base)?;
    let seed = dir.join("seed");
    if seed.is_dir() {
        evalfs::copy_tree(&seed, &scratch)?;
    }
    Ok(scratch)
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

/// The committed scenarios root (`eval/scenarios/`).
#[must_use]
pub fn scenarios_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios")
}
