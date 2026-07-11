#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
anyhow = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
---

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use anyhow::{bail, ensure, Context as _, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    target_directory: PathBuf,
    workspace_members: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    manifest_path: PathBuf,
    name: String,
    version: String,
}

fn main() -> Result<()> {
    let root = env::current_dir().context("reading the current directory")?;
    ensure!(root.join("Cargo.toml").is_file(), "run this script from the workspace root");

    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("build-fast") => build_fast(&root, args.next(), args.next()),
        Some("publish") => {
            ensure!(args.next().is_none(), "publish accepts no arguments");
            publish(&root)
        }
        Some(command) => bail!("unknown command `{command}`; expected `build-fast` or `publish`"),
        None => bail!("expected `build-fast` or `publish`"),
    }
}

fn build_fast(root: &Path, requested: Option<String>, extra: Option<String>) -> Result<()> {
    ensure!(extra.is_none(), "build-fast accepts at most one adapter name");
    let metadata = metadata(root)?;
    let packages = adapters(root, &metadata);
    let selected = match requested {
        Some(name) => {
            let package = packages
                .into_iter()
                .find(|package| package.name == name)
                .with_context(|| format!("`{name}` is not a source or target adapter"))?;
            vec![package]
        }
        None => packages,
    };

    let mut command = Command::new("cargo");
    command.current_dir(root).args(["build", "--target", "wasm32-wasip2", "--release"]);
    for package in selected {
        command.args(["-p", &format!("{}@{}", package.name, package.version)]);
    }
    command
        .env("CARGO_PROFILE_RELEASE_LTO", "false")
        .env("CARGO_PROFILE_RELEASE_OPT_LEVEL", "1")
        .env("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "16")
        .env("CARGO_PROFILE_RELEASE_STRIP", "false");
    run(&mut command, "fast component build")
}

fn publish(root: &Path) -> Result<()> {
    let metadata = metadata(root)?;
    let config = root.join(".wkg-config.toml");
    ensure!(config.is_file(), "missing {}", config.display());

    for package in adapters(root, &metadata) {
        let artifact = metadata
            .target_directory
            .join("wasm32-wasip2/release")
            .join(format!("{}.wasm", package.name));
        ensure!(
            artifact.is_file(),
            "missing {}; run `cargo make release` first",
            artifact.display()
        );
        publish_one(package, &artifact, &config, &metadata.target_directory)?;
    }
    Ok(())
}

fn publish_one(package: &Package, artifact: &Path, config: &Path, target: &Path) -> Result<()> {
    let reference = format!("specify:{}@{}", package.name, package.version);
    let probe = target.join(format!(".wkg-probe-{}-{}.wasm", std::process::id(), package.name));
    let _ = fs::remove_file(&probe);

    let output = Command::new("wkg")
        .args(["get", &reference, "--output"])
        .arg(&probe)
        .args(["--config"])
        .arg(config)
        .output()
        .with_context(|| format!("probing {reference}"))?;

    if output.status.success() {
        let published = fs::read(&probe).with_context(|| format!("reading {}", probe.display()))?;
        let local =
            fs::read(artifact).with_context(|| format!("reading {}", artifact.display()))?;
        fs::remove_file(&probe).with_context(|| format!("removing {}", probe.display()))?;
        ensure!(published == local, "{reference} already exists with different component bytes");
        println!("skip {reference}: identical package already exists");
        return Ok(());
    }
    let _ = fs::remove_file(&probe);

    let mut command = Command::new("wkg");
    command.arg("publish").arg(artifact).args(["--package", &reference, "--config"]).arg(config);
    run(&mut command, &format!("publishing {reference}"))
}

fn metadata(root: &Path) -> Result<Metadata> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("running cargo metadata")?;
    ensure!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    serde_json::from_slice(&output.stdout).context("decoding cargo metadata")
}

fn adapters<'a>(root: &Path, metadata: &'a Metadata) -> Vec<&'a Package> {
    let mut packages: Vec<_> = metadata
        .packages
        .iter()
        .filter(|package| metadata.workspace_members.contains(&package.id))
        .filter(|package| {
            package
                .manifest_path
                .strip_prefix(root)
                .ok()
                .and_then(|path| path.components().next())
                .is_some_and(|component| {
                    component.as_os_str() == OsStr::new("sources")
                        || component.as_os_str() == OsStr::new("targets")
                })
        })
        .collect();
    packages.sort_by(|left, right| left.name.cmp(&right.name));
    packages
}

fn run(command: &mut Command, action: &str) -> Result<()> {
    let status = command.status().with_context(|| action.to_owned())?;
    ensure!(status.success(), "{action} failed with {status}");
    Ok(())
}
