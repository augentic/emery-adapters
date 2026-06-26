//! Host toolchain prerequisite probes for `verify --mode host-prereq`.
//!
//! Runs inside the WASI guest; uses environment variables and filesystem
//! probes only (no process spawn).

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const ANDROID_RUST_TARGETS: &[&str] = &["aarch64-linux-android", "armv7-linux-androideabi"];

/// Emit host-prerequisite findings for declared `project.yaml` platforms.
#[must_use]
pub fn host_prereq_findings(platforms: &[String]) -> Vec<Value> {
    let mut findings = Vec::new();

    if platforms.iter().any(|p| p == "android") {
        findings.extend(android_host_prereq());
    }

    if platforms.iter().any(|p| p == "ios") {
        findings.extend(ios_host_prereq());
    }

    findings
}

fn android_host_prereq() -> Vec<Value> {
    let mut findings = Vec::new();

    let android_home = std::env::var("ANDROID_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("ANDROID_SDK_ROOT").ok().filter(|v| !v.is_empty()));

    if android_home.is_none() {
        findings.push(error_finding(
            "android-sdk-home-unset",
            "ANDROID_HOME (or ANDROID_SDK_ROOT) must be set when android is in project platforms",
        ));
    }

    if !android_rust_targets_installed() {
        findings.push(error_finding(
            "android-rust-target-missing",
            "Rust Android targets not installed; run `rustup target add aarch64-linux-android armv7-linux-androideabi`",
        ));
    }

    findings
}

fn ios_host_prereq() -> Vec<Value> {
    #[cfg(target_os = "macos")]
    {
        if xcodebuild_available() {
            Vec::new()
        } else {
            vec![error_finding(
                "ios-xcodebuild-missing",
                "xcodebuild not found; install Xcode command-line tools when ios is in project platforms",
            )]
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "macos")]
fn xcodebuild_available() -> bool {
    if let Ok(developer_dir) = std::env::var("DEVELOPER_DIR") {
        let path = PathBuf::from(developer_dir).join("usr/bin/xcodebuild");
        if path.is_file() {
            return true;
        }
    }

    [
        "/usr/bin/xcodebuild",
        "/Applications/Xcode.app/Contents/Developer/usr/bin/xcodebuild",
    ]
    .iter()
    .any(|path| Path::new(path).is_file())
}

fn android_rust_targets_installed() -> bool {
    let Some(rustup_home) = rustup_home() else {
        return false;
    };

    let Ok(toolchains) = std::fs::read_dir(rustup_home.join("toolchains")) else {
        return false;
    };

    toolchains.flatten().any(|entry| {
        let lib = entry.path().join("lib/rustlib");
        ANDROID_RUST_TARGETS
            .iter()
            .all(|target| lib.join(target).is_dir())
    })
}

fn rustup_home() -> Option<PathBuf> {
    std::env::var("RUSTUP_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".rustup"))
        })
        .filter(|path| path.is_dir())
}

fn error_finding(id: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "severity": "error",
        "source": "deterministic",
        "message": message.into(),
    })
}
