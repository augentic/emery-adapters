//! Live adapter-guest quality tests over the per-adapter scenario trees.
//!
//! Each `#[ignore]`d test seeds a scratch project tree from
//! `harness/<adapter>/scenarios/<scenario>/` (`seed/` copied to the root,
//! `inputs/*.md` to `.eval/inputs/`), builds the adapter and eval guests,
//! writes the deployment manifest, and drives one command-mode eval by
//! spawning the prebuilt `eval-driver` example against the live cursor
//! backend. The report JSON line and full log land under
//! `harness/<adapter>/runs/<scenario>/`; the test fails on a failing report.
//!
//! Requires `cursor-agent` on `PATH`, authenticated via `CURSOR_API_KEY` or
//! a prior `cursor-agent login`:
//!
//! ```text
//! cargo test -p harness --test live -- --ignored --nocapture contracts::
//! ```
//!
//! The non-ignored `wiring` test beside each adapter's live tests is the
//! model-free smoke: it seeds every scenario and renders the manifest
//! without building guests or spawning anything, so CI catches
//! scenario-tree drift without a model or a cursor-agent install.
//!
//! `SPECIFY_PROSE_OVERLAY=1` switches a live run into overlay mode:
//! the adapter's prose trees seed `<scratch>/.eval/prose/`, the env var
//! is forwarded to the guest (whose registry probes the overlay at
//! runtime), and once the three artifacts exist on disk the cargo legs
//! are skipped entirely — a prose-only edit re-invokes the driver with
//! no build.

#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail, ensure};
use tempfile::TempDir;

mod contracts {
    use anyhow::Result;

    #[test]
    fn wiring() -> Result<()> {
        super::wiring("contracts")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn metadata() -> Result<()> {
        super::live("contracts", "describe", "build", "user-adapter-api")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn design() -> Result<()> {
        super::live("contracts", "design", "build", "returns-api")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn import() -> Result<()> {
        super::live("contracts", "import", "build", "import-ticket-api-contract")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn source() -> Result<()> {
        super::live("contracts", "source", "build", "orders-api-contract")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn update() -> Result<()> {
        super::live("contracts", "update", "build", "loyalty-api-contract")
    }
}

mod vectis {
    use anyhow::Result;

    #[test]
    fn wiring() -> Result<()> {
        super::wiring("vectis")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn single_screen() -> Result<()> {
        super::live("vectis", "single-screen", "build", "daily-quote")
    }
}

mod overlay {
    use anyhow::Result;

    // Every embed key must have a seeded overlay file, including symlinked paths.
    #[test]
    fn seeding() -> Result<()> {
        for adapter in ["contracts", "vectis"] {
            super::seeding_parity(adapter)?;
        }
        Ok(())
    }
}

/// Drive one live eval of `operation` (the eval guest's operation
/// selector): build the guests, seed the scratch tree, spawn the prebuilt
/// driver, log the run, and fail on a failing exit status.
fn live(adapter: &str, scenario: &str, operation: &str, slice: &str) -> Result<()> {
    ensure!(
        cursor_agent_on_path(),
        "cursor-agent not found on PATH; see harness/{adapter}/README.md"
    );

    let root = workspace_root();
    let target = harness::target_dir()?;
    let overlay = overlay_active();
    build(adapter, root, &target, overlay)?;

    // Persist the scratch tree so a run's delta stays inspectable.
    let scratch = seed(adapter, scenario)?.keep();
    if overlay {
        seed_overlay(adapter, &scratch)?;
    }
    let manifest_path = scratch.join("omnia.toml");
    fs::write(&manifest_path, manifest(&target, adapter, &scratch))?;

    let addr = http_addr()?;
    let runs = manifest_dir().join(adapter).join("runs").join(scenario);
    fs::create_dir_all(&runs)?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let log = runs.join(format!("run-{stamp}.log"));
    let report_path = runs.join(format!("run-{stamp}.json"));
    println!(
        "eval {adapter}/{scenario}: slice={slice} scratch={} log={}",
        scratch.display(),
        log.display()
    );

    // The runtime supplies argv[0] (the deployment name); ours start at the
    // operation selector. Every scenario tree today exercises a target
    // adapter, so the axis-qualified id stays `target:` — the guest passes
    // it through verbatim.
    let driver = driver(&target);
    let mut command = Command::new(&driver);
    command.current_dir(root).env("HTTP_ADDR", &addr).env(
        format!("SPECIFY_{}_MCP_URL", adapter.to_uppercase()),
        format!("http://{addr}/mcp/{adapter}"),
    );
    if overlay {
        // Forwarded to the guest through the same host-env channel the
        // MCP URL uses; the registry probes `.eval/prose/` only under
        // this grant.
        command.env("SPECIFY_PROSE_OVERLAY", "1");
    }
    let output = command
        .args(["run", "--config"])
        .arg(&manifest_path)
        .args(["--", operation, &format!("target:{adapter}"), slice, ".eval/inputs"])
        .output()
        .with_context(|| format!("spawning {}", driver.display()))?;

    let mut body = output.stdout.clone();
    body.extend_from_slice(&output.stderr);
    fs::write(&log, &body)?;
    println!("{}", String::from_utf8_lossy(&body));

    let adapter_report = String::from_utf8_lossy(&output.stdout)
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .unwrap_or(serde_json::Value::Null);
    let outcome = if output.status.success() { "pass" } else { "fail" };
    let report = serde_json::json!({
        "version": 1,
        "scenario": format!("{adapter}/{scenario}"),
        "profile": "adapter-live",
        "runtime": "wasm",
        "model": std::env::var("SPECIFY_EVAL_MODEL").unwrap_or_else(|_| "cursor-default".to_owned()),
        "gate": "adapter-prompt-quality",
        "outcome": outcome,
        "run": {
            "id": format!("{adapter}-{scenario}-{stamp}"),
            "started-at-unix": stamp,
            "log": log.display().to_string(),
            "scratch": scratch.display().to_string(),
        },
        "hard-assertions": [{
            "id": "adapter-report-success",
            "outcome": outcome,
            "evidence": adapter_report,
        }],
        "semantic-rubrics": [],
    });
    fs::write(&report_path, serde_json::to_vec_pretty(&report)?)?;

    ensure!(
        output.status.success(),
        "eval {adapter}/{scenario} failed ({}); log at {}, delta under {}",
        output.status,
        log.display(),
        scratch.display()
    );
    println!(
        "eval {adapter}/{scenario}: delta under {}",
        scratch.join(".specify").join("slices").join(slice).display()
    );
    println!("eval {adapter}/{scenario}: structured report {}", report_path.display());
    Ok(())
}

/// The model-free smoke: seed every scenario of `adapter` into a (dropped)
/// scratch tree and render its manifest, proving the scenario trees and
/// manifest writer are well-formed without guests or a model.
fn wiring(adapter: &str) -> Result<()> {
    let scenarios = manifest_dir().join(adapter).join("scenarios");
    let target = harness::target_dir()?;
    let mut seen = 0;
    for entry in fs::read_dir(&scenarios)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let scenario = entry.file_name().to_string_lossy().into_owned();
        let scratch = seed(adapter, &scenario)?;
        let body = manifest(&target, adapter, scratch.path());
        ensure!(
            body.contains(&format!("guest = \"target:{adapter}\"")),
            "manifest for {adapter}/{scenario} misses the adapter route"
        );
        seen += 1;
    }
    ensure!(seen > 0, "no scenarios under {}", scenarios.display());
    Ok(())
}

/// Seed a scratch project tree for `adapter`/`scenario`: `seed/**` copied to
/// the root, `inputs/*.md` to `.eval/inputs/`.
fn seed(adapter: &str, scenario: &str) -> Result<TempDir> {
    let scenario_dir = manifest_dir().join(adapter).join("scenarios").join(scenario);
    ensure!(scenario_dir.is_dir(), "unknown scenario `{adapter}/{scenario}`");

    let scratch =
        tempfile::Builder::new().prefix(&format!("specify-eval-{scenario}.")).tempdir()?;
    let seed = scenario_dir.join("seed");
    if seed.is_dir() {
        harness::copy_tree(&seed, scratch.path())?;
    }

    let inputs = scratch.path().join(".eval").join("inputs");
    fs::create_dir_all(&inputs)?;
    let mut copied = 0;
    for entry in fs::read_dir(scenario_dir.join("inputs"))? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            fs::copy(&path, inputs.join(path.file_name().expect("input file name")))?;
            copied += 1;
        }
    }
    ensure!(copied > 0, "scenario `{adapter}/{scenario}` has no `inputs/*.md`");
    Ok(scratch)
}

fn overlay_active() -> bool {
    std::env::var("SPECIFY_PROSE_OVERLAY").is_ok_and(|value| value == "1")
}

/// Build the three run artifacts — or, in overlay mode, skip cargo when
/// all three exist. Overlay selection is a runtime env grant, so there
/// is only one flavor of each binary and presence alone is proof enough.
fn build(adapter: &str, root: &Path, target: &Path, overlay: bool) -> Result<()> {
    if overlay && artifacts(target, adapter).iter().all(|path| path.is_file()) {
        println!("eval {adapter}: overlay active, artifacts present; cargo builds skipped");
        return Ok(());
    }
    harness::cargo(&["build", "-p", adapter, "--target", "wasm32-wasip2"], root, target)?;
    harness::cargo(
        &["build", "-p", "harness", "--example", "eval-guest", "--target", "wasm32-wasip2"],
        root,
        target,
    )?;
    harness::cargo(&["build", "-p", "harness", "--example", "eval-driver"], root, target)?;
    Ok(())
}

fn artifacts(target: &Path, adapter: &str) -> [PathBuf; 3] {
    let wasm = target.join("wasm32-wasip2").join("debug");
    [adapter_wasm(target, adapter), wasm.join("examples").join("eval_guest.wasm"), driver(target)]
}

fn adapter_wasm(target: &Path, adapter: &str) -> PathBuf {
    target.join("wasm32-wasip2").join("debug").join(format!("{adapter}.wasm"))
}

fn driver(target: &Path) -> PathBuf {
    target.join("debug").join("examples").join("eval-driver")
}

/// Seed the prose overlay: copy the adapter's `prose/` tree into
/// `<scratch>/.eval/prose/` under the registry key convention (keys
/// omit the on-disk `prose/` prefix), resolving symlinks the way the
/// build-time embed does. The embed set is whatever is on disk under
/// `prose/`, mirroring `prose::emit_from`.
fn seed_overlay(adapter: &str, scratch: &Path) -> Result<()> {
    let prose = adapter_dir(adapter)?.join("prose");
    let overlay = scratch.join(".eval").join("prose");
    if prose.is_dir() {
        harness::copy_tree(&prose, &overlay)?;
    }
    Ok(())
}

fn adapter_dir(adapter: &str) -> Result<PathBuf> {
    for axis in ["targets", "sources"] {
        let dir = workspace_root().join(axis).join(adapter);
        if dir.is_dir() {
            return Ok(dir);
        }
    }
    bail!("no adapter directory for `{adapter}` under targets/ or sources/")
}

/// Prove seeding parity for one adapter: run the build-time embed walk
/// (`prose::emit_from`) into a scratch dir, then assert the seeded
/// overlay carries a file for every emitted key.
fn seeding_parity(adapter: &str) -> Result<()> {
    let out = TempDir::new()?;
    prose::emit_from(&adapter_dir(adapter)?, out.path()).map_err(anyhow::Error::msg)?;
    let table = fs::read_to_string(out.path().join("registry_docs.rs"))?;
    let keys = doc_keys(&table);
    ensure!(!keys.is_empty(), "embed walk found no documents for {adapter}");

    let scratch = TempDir::new()?;
    seed_overlay(adapter, scratch.path())?;
    let overlay = scratch.path().join(".eval").join("prose");
    for key in &keys {
        ensure!(overlay.join(key).is_file(), "overlay misses `{key}` for {adapter}");
    }
    Ok(())
}

// The `path:` keys of a generated `registry_docs.rs` table — coupled to
// the exact `Doc { path: "…", body: … }` line shape `prose::emit_from` writes.
fn doc_keys(table: &str) -> Vec<String> {
    table
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Doc { path: \""))
        .filter_map(|rest| rest.split('"').next())
        .map(str::to_owned)
        .collect()
}

/// The deployment manifest: the eval guest (the `wasi:cli/run` exporter)
/// linked to the adapter guest, sharing the scratch mount; the HTTP trigger
/// serves the adapter's MCP reference route for the spawned cursor-agent.
fn manifest(target: &Path, adapter: &str, scratch: &Path) -> String {
    let wasm = target.join("wasm32-wasip2").join("debug");
    let guests = [
        harness::Guest {
            id: "eval".to_owned(),
            wasm: wasm.join("examples").join("eval_guest.wasm"),
            link: vec![
                "specify:adapter/source@0.1.0".to_owned(),
                "specify:adapter/target@0.1.0".to_owned(),
            ],
            route: None,
        },
        harness::Guest {
            id: format!("target:{adapter}"),
            wasm: wasm.join(format!("{adapter}.wasm")),
            link: Vec::new(),
            route: Some(format!("/mcp/{adapter}")),
        },
    ];
    harness::manifest(&guests, scratch)
}

// Honour an operator-set HTTP_ADDR, else grab an ephemeral port so
// parallel scenarios never contend.
fn http_addr() -> Result<String> {
    if let Ok(addr) = std::env::var("HTTP_ADDR") {
        return Ok(addr);
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.to_string())
}

fn cursor_agent_on_path() -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|dir| dir.join("cursor-agent").is_file())
    })
}

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> &'static Path {
    manifest_dir().parent().expect("harness/ sits at the workspace root")
}
