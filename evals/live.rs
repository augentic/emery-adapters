//! Live adapter-guest evals over the per-adapter scenario trees.
//!
//! Each `#[ignore]`d test seeds a scratch project tree from
//! `evals/<adapter>/scenarios/<scenario>/` (`seed/` copied to the root,
//! `inputs/*.md` to `.eval/inputs/`), builds the adapter and eval guests,
//! writes the deployment manifest, and drives one command-mode eval by
//! spawning the prebuilt `eval-driver` example against the live cursor
//! backend. The report JSON line and full log land under
//! `evals/<adapter>/runs/<scenario>/`; the test fails on a failing report.
//!
//! Requires `cursor-agent` on `PATH`, authenticated via `CURSOR_API_KEY` or
//! a prior `cursor-agent login`:
//!
//! ```text
//! cargo test -p evals --test live -- --ignored --nocapture contracts::
//! ```
//!
//! The non-ignored `wiring` test beside each adapter's live tests is the
//! model-free smoke: it seeds every scenario and renders the manifest
//! without building guests or spawning anything, so CI catches
//! scenario-tree drift without a model or a cursor-agent install.
//!
//! `SPECIFY_EVAL_OVERLAY=1` switches a live run into prose-overlay mode
//! (RFC-62): the adapter guest builds with the `adapter/prose-overlay`
//! feature, the adapter's prose trees seed `<scratch>/.eval/prose/`, and
//! once the three artifacts exist on disk the cargo legs are skipped
//! entirely — a prose-only edit re-invokes the driver with no build.

#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, bail, ensure};
use sha2::{Digest as _, Sha256};
use tempfile::TempDir;

mod contracts {
    use anyhow::Result;

    // The slice each scenario builds (mirrors targets/contracts/tests/).
    #[test]
    fn wiring() -> Result<()> {
        super::wiring("contracts")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn describe() -> Result<()> {
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

    // The model-free seeding parity check: every key the build-time embed
    // would emit must have a seeded overlay file, including keys reached
    // through the prose-tree symlinks (RFC-62 criterion 2).
    #[test]
    fn seeding() -> Result<()> {
        for adapter in ["contracts", "vectis"] {
            super::seeding_parity(adapter)?;
        }
        Ok(())
    }

    // The skip predicate over the real target dir: run once after the
    // one-time overlay build to prove a re-invocation spawns no cargo
    // (RFC-62 criterion 6) — and that the skip is sound: the stamp must
    // match the current adapter wasm, not merely exist.
    #[test]
    #[ignore = "needs the three prebuilt artifacts; run after an overlay-mode build"]
    fn artifacts_present() -> Result<()> {
        let target = harness::target_dir()?;
        for adapter in ["contracts", "vectis"] {
            for path in super::artifacts(&target, adapter) {
                anyhow::ensure!(path.is_file(), "missing artifact {}", path.display());
            }
            anyhow::ensure!(
                super::overlay_fresh(&target, adapter)?,
                "overlay stamp for {adapter} is missing or stale — an unflagged build \
                 overwrote the flagged wasm; re-run the overlay build"
            );
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
        "cursor-agent not found on PATH; see evals/{adapter}/README.md"
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
    let output = Command::new(&driver)
        .current_dir(root)
        .env("HTTP_ADDR", &addr)
        .env(
            format!("SPECIFY_{}_MCP_URL", adapter.to_uppercase()),
            format!("http://{addr}/mcp/{adapter}"),
        )
        .args(["run", "--config"])
        .arg(&manifest_path)
        .args(["--", operation, &format!("target:{adapter}"), slice, ".eval/inputs"])
        .output()
        .with_context(|| format!("spawning {}", driver.display()))?;

    let mut body = output.stdout.clone();
    body.extend_from_slice(&output.stderr);
    fs::write(&log, &body)?;
    println!("{}", String::from_utf8_lossy(&body));

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
    std::env::var("SPECIFY_EVAL_OVERLAY").is_ok_and(|value| value == "1")
}

/// Build the three run artifacts — or, in overlay mode, skip cargo
/// entirely when all three exist and the adapter wasm still matches the
/// stamp from the last overlay-flagged build. A prose edit changes no
/// Rust, but unflagged builds (the composed `tests/` suite, non-overlay
/// runs) share the same artifact path, so presence alone cannot prove
/// the feature is compiled in — the stamp is what guards against
/// spawning a guest that would silently serve embedded bodies. A Rust
/// edit under the overlay is still the RFC's stale-artifact trap:
/// re-run without it.
fn build(adapter: &str, root: &Path, target: &Path, overlay: bool) -> Result<()> {
    // `-p {adapter}` assumes the eval name is the cargo package name;
    // `omnia` breaks this (package `omnia-adapter`, lib `omnia`) — map
    // the package name here before adding omnia evals.
    ensure!(
        adapter != "omnia",
        "eval `omnia` builds as package `omnia-adapter`; teach `build` the mapping first"
    );
    if overlay && overlay_fresh(target, adapter)? {
        println!("eval {adapter}: overlay active, stamped artifacts present; cargo builds skipped");
        return Ok(());
    }
    let mut guest = vec!["build", "-p", adapter, "--target", "wasm32-wasip2"];
    if overlay {
        // Enables the `adapter` dependency's feature on the guest build;
        // the eval guest reads no prose, so only this leg carries it.
        guest.extend(["--features", "adapter/prose-overlay"]);
    }
    harness::cargo(&guest, root, target)?;
    if overlay {
        fs::write(stamp_path(target, adapter), digest(&adapter_wasm(target, adapter))?)?;
    }
    harness::cargo(
        &["build", "-p", "evals", "--example", "eval-guest", "--target", "wasm32-wasip2"],
        root,
        target,
    )?;
    harness::cargo(&["build", "-p", "evals", "--example", "eval-driver"], root, target)?;
    if overlay {
        println!("eval {adapter}: cargo builds ran (prose-overlay)");
    }
    Ok(())
}

// The skip predicate: all three artifacts exist and the adapter wasm's
// digest matches the stamp written after the last overlay-flagged build.
fn overlay_fresh(target: &Path, adapter: &str) -> Result<bool> {
    if !artifacts(target, adapter).iter().all(|path| path.is_file()) {
        return Ok(false);
    }
    let Ok(stamp) = fs::read(stamp_path(target, adapter)) else {
        return Ok(false);
    };
    Ok(stamp == digest(&adapter_wasm(target, adapter))?)
}

// The overlay stamp beside the adapter wasm: the raw SHA-256 of the last
// overlay-flagged build of `<adapter>.wasm`.
fn stamp_path(target: &Path, adapter: &str) -> PathBuf {
    target.join("wasm32-wasip2").join("debug").join(format!(".{adapter}.overlay-stamp"))
}

fn digest(path: &Path) -> Result<Vec<u8>> {
    Ok(Sha256::digest(fs::read(path)?).to_vec())
}

// The three prebuilt artifacts a run spawns: the adapter guest, the eval
// guest, and the native eval-driver example.
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

/// Seed the prose overlay: copy the adapter's embedded prose trees into
/// `<scratch>/.eval/prose/<tree>` under the registry key convention (keys
/// omit the on-disk `prose/` prefix), resolving symlinks the way the
/// build-time embed does.
fn seed_overlay(adapter: &str, scratch: &Path) -> Result<()> {
    let prose = adapter_dir(adapter)?.join("prose");
    let overlay = scratch.join(".eval").join("prose");
    for tree in embedded_trees(adapter) {
        let from = prose.join(tree);
        if from.is_dir() {
            harness::copy_tree(&from, &overlay.join(tree))?;
        }
    }
    Ok(())
}

// The adapter's on-disk directory: eval adapter names match the directory
// name under `targets/` or `sources/` (not necessarily the package name —
// `targets/omnia` builds `omnia-adapter`).
fn adapter_dir(adapter: &str) -> Result<PathBuf> {
    for axis in ["targets", "sources"] {
        let dir = workspace_root().join(axis).join(adapter);
        if dir.is_dir() {
            return Ok(dir);
        }
    }
    bail!("no adapter directory for `{adapter}` under targets/ or sources/")
}

// The prose trees the adapter's core embeds — mirrors the `emit_core`
// call in `<adapter>/core/build.rs` (pinned by `overlay::seeding`).
fn embedded_trees(adapter: &str) -> &'static [&'static str] {
    match adapter {
        "vectis" => &["prompts", "references", "rules"],
        _ => &["prompts", "references"],
    }
}

/// Prove seeding parity for one adapter: run the build-time embed walk
/// (`prose::emit`) into a scratch dir, then assert the seeded overlay
/// carries a file for every emitted key.
fn seeding_parity(adapter: &str) -> Result<()> {
    let trees = embedded_trees(adapter);
    let build_rs = adapter_dir(adapter)?.join("core").join("build.rs");
    let declared = emit_core_trees(&fs::read_to_string(&build_rs)?)?;
    ensure!(
        declared.iter().map(String::as_str).eq(trees.iter().copied()),
        "embedded_trees({adapter:?}) = {trees:?} drifted from {}: {declared:?}",
        build_rs.display()
    );

    let out = TempDir::new()?;
    prose::emit(&adapter_dir(adapter)?, trees, out.path()).map_err(anyhow::Error::msg)?;
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

// The tree list from a core `build.rs`'s `emit_core(&[...])` call.
fn emit_core_trees(build_rs: &str) -> Result<Vec<String>> {
    let list = build_rs
        .split_once("emit_core(&[")
        .and_then(|(_, rest)| rest.split_once("])"))
        .map(|(list, _)| list)
        .context("core build.rs calls emit_core(&[...])")?;
    Ok(list.split('"').skip(1).step_by(2).map(str::to_owned).collect())
}

// The `path:` keys of a generated `registry_docs.rs` table — coupled to
// the exact `Doc { path: "…", body: … }` line shape `prose::emit` writes.
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

// The HTTP trigger address: honour an operator-set HTTP_ADDR, else grab an
// ephemeral port so parallel scenarios never contend.
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

// This package's directory (`evals/`), the root of the scenario trees.
fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> &'static Path {
    manifest_dir().parent().expect("evals/ sits at <workspace>/evals")
}
