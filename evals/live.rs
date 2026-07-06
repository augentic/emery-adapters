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

#![cfg(not(target_arch = "wasm32"))]

use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result, ensure};
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
        super::live("contracts", "describe", "user-adapter-api")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn design() -> Result<()> {
        super::live("contracts", "design", "returns-api")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn import() -> Result<()> {
        super::live("contracts", "import", "import-ticket-api-contract")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn source() -> Result<()> {
        super::live("contracts", "source", "orders-api-contract")
    }

    #[test]
    #[ignore = "live: needs an authenticated cursor-agent on PATH; run with -- --ignored"]
    fn update() -> Result<()> {
        super::live("contracts", "update", "loyalty-api-contract")
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
        super::live("vectis", "single-screen", "daily-quote")
    }
}

/// Drive one live eval: build the guests, seed the scratch tree, spawn the
/// prebuilt driver, log the run, and fail on a failing report status.
fn live(adapter: &str, scenario: &str, slice: &str) -> Result<()> {
    ensure!(
        cursor_agent_on_path(),
        "cursor-agent not found on PATH; see evals/{adapter}/README.md"
    );

    let root = workspace_root();
    let target = target_dir()?;
    cargo(&["build", "-p", adapter, "--target", "wasm32-wasip2"], root, &target)?;
    cargo(
        &["build", "-p", "evals", "--example", "eval-guest", "--target", "wasm32-wasip2"],
        root,
        &target,
    )?;
    cargo(&["build", "-p", "evals", "--example", "eval-driver"], root, &target)?;

    // Persist the scratch tree so a run's delta stays inspectable.
    let scratch = seed(adapter, scenario)?.keep();
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
    // adapter id.
    let driver = target.join("debug").join("examples").join("eval-driver");
    let output = Command::new(&driver)
        .current_dir(root)
        .env("HTTP_ADDR", &addr)
        .env(
            format!("SPECIFY_{}_MCP_URL", adapter.to_uppercase()),
            format!("http://{addr}/mcp/{adapter}"),
        )
        .args(["run", "--config"])
        .arg(&manifest_path)
        .args(["--", &format!("target:{adapter}"), slice, ".eval/inputs"])
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
    let target = target_dir()?;
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
        copy_tree(&seed, scratch.path())?;
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

/// The deployment manifest: the eval guest (the `wasi:cli/run` exporter)
/// linked to the adapter guest, sharing the scratch mount; the HTTP trigger
/// serves the adapter's MCP reference route for the spawned cursor-agent.
fn manifest(target: &Path, adapter: &str, scratch: &Path) -> String {
    let wasm = target.join("wasm32-wasip2").join("debug");
    format!(
        r#"[[guest]]
id = "eval"
source.path = "{eval}"
link = ["specify:adapter/source@0.1.0", "specify:adapter/target@0.1.0"]

[[guest]]
id = "target:{adapter}"
source.path = "{adapter_wasm}"

[[mount]]
name = "."
path = "{scratch}"
writable = true

[[route.http]]
prefix = "/mcp/{adapter}"
guest = "target:{adapter}"

[transport]
default = "in-process"
"#,
        eval = wasm.join("examples").join("eval_guest.wasm").display(),
        adapter_wasm = wasm.join(format!("{adapter}.wasm")).display(),
        scratch = scratch.display(),
    )
}

// Run one cargo invocation against the workspace, into `target`.
fn cargo(args: &[&str], root: &Path, target: &Path) -> Result<()> {
    let status = Command::new("cargo")
        .env("CARGO_TARGET_DIR", target)
        .args(args)
        .current_dir(root)
        .status()
        .context("spawning cargo")?;
    ensure!(status.success(), "cargo {} failed with {status}", args.join(" "));
    Ok(())
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

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

// This package's directory (`evals/`), the root of the scenario trees.
fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> &'static Path {
    manifest_dir().parent().expect("evals/ sits at <workspace>/evals")
}

// The cargo target dir this test binary was built into (testkit's
// convention: the test exe sits at `<target>/<profile>/deps/<exe>`).
fn target_dir() -> Result<PathBuf> {
    let test_exe = std::env::current_exe().context("test executable has a path")?;
    Ok(test_exe
        .ancestors()
        .nth(3)
        .expect("test exe sits at <target>/<profile>/deps/<exe>")
        .to_path_buf())
}
