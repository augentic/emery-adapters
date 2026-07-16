//! The shared change-trial inputs: one checked-in definition of the
//! operator arguments both live rungs replay.
//!
//! [`examples/change/trial.env`] carries the project name, change
//! name, source binding, and intent as shell-sourceable `KEY="value"`
//! lines: the wasm change example's Cargo Make task `source`s it, and
//! the native trial parses the same file here — so the two rungs
//! cannot drift apart on what they ask the workflow to do. The seed
//! tree is shared the same way: both rungs copy
//! `examples/change/seed/` into their own sandbox.
//!
//! The parser accepts exactly the sourceable subset the file's header
//! documents — blank lines, `#` comments, and double-quoted values
//! free of shell expansion — and rejects anything else, so a value
//! that would mean something different to `source` never parses here.
//!
//! [`examples/change/trial.env`]: ../../examples/change/trial.env

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail, ensure};

/// The operator inputs of one change trial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialInputs {
    /// The project name passed to `specify init --name`.
    pub project_name: String,
    /// The change name `plan author` and `plan transition` operate on.
    pub change: String,
    /// The `--source` binding (`<key>=<adapter>:<path>`).
    pub source: String,
    /// The operator intent bound as the `intent` source.
    pub intent: String,
}

impl TrialInputs {
    /// Load the shared definition from `examples/change/trial.env`.
    ///
    /// # Errors
    ///
    /// Returns a missing file and any parse failure from [`Self::parse`].
    pub fn load() -> Result<Self> {
        let path = path();
        let body =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&body).with_context(|| format!("parsing {}", path.display()))
    }

    /// Parse the shell-sourceable `KEY="value"` body.
    ///
    /// # Errors
    ///
    /// Returns lines outside the documented subset, duplicate or
    /// unknown keys, missing keys, and values a shell `source` would
    /// interpret differently (expansion characters or inner quotes).
    pub fn parse(body: &str) -> Result<Self> {
        let mut values: BTreeMap<&str, String> = BTreeMap::new();
        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, raw)) = line.split_once('=') else {
                bail!("expected `KEY=\"value\"`, got `{line}`");
            };
            let value = raw
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .with_context(|| format!("`{key}` value must be double-quoted"))?;
            ensure!(
                !value.contains(['"', '$', '`', '\\']),
                "`{key}` value contains a character the shell would expand"
            );
            ensure!(!value.trim().is_empty(), "`{key}` value is empty");
            ensure!(
                values.insert(key, value.to_owned()).is_none(),
                "duplicate key `{key}`"
            );
        }

        let mut take = |key: &str| {
            values.remove(key).with_context(|| format!("missing `{key}=\"…\"` line"))
        };
        let inputs = Self {
            project_name: take("TRIAL_PROJECT_NAME")?,
            change: take("TRIAL_CHANGE")?,
            source: take("TRIAL_SOURCE")?,
            intent: take("TRIAL_INTENT")?,
        };
        ensure!(
            values.is_empty(),
            "unknown keys: {}",
            values.keys().copied().collect::<Vec<_>>().join(", ")
        );
        Ok(inputs)
    }
}

/// The checked-in shared definition (`examples/change/trial.env`).
#[must_use]
pub fn path() -> PathBuf {
    examples_change().join("trial.env")
}

/// The shared seed tree both rungs copy into their sandbox
/// (`examples/change/seed/`).
#[must_use]
pub fn seed_dir() -> PathBuf {
    examples_change().join("seed")
}

fn examples_change() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/change")
}
