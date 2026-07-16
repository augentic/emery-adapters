//! Repository-relative data locators for the trial and the scenarios.

use std::path::{Path, PathBuf};

/// The checked-in shared trial definition (`examples/change/trial.env`).
#[must_use]
pub fn trial_env() -> PathBuf {
    examples_change().join("trial.env")
}

/// The shared seed tree both rungs copy into their sandbox.
#[must_use]
pub fn seed_dir() -> PathBuf {
    examples_change().join("seed")
}

/// The committed prompt-scenario root (`eval/scenarios/`).
#[must_use]
pub fn scenarios_dir() -> PathBuf {
    manifest().join("../scenarios")
}

/// The persistent trial sandbox project root (`sandbox/eval`).
#[must_use]
pub fn eval_sandbox() -> PathBuf {
    repo_root().join("sandbox/eval")
}

/// The scenario scratch base (`sandbox/scenarios`).
#[must_use]
pub fn scenarios_sandbox() -> PathBuf {
    repo_root().join("sandbox/scenarios")
}

fn examples_change() -> PathBuf {
    repo_root().join("examples/change")
}

fn repo_root() -> PathBuf {
    manifest().join("../..")
}

fn manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}
