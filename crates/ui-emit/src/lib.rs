//! A ui-ir design system (`composition.json`, `tokens.json`, `assets.json`) turned into a Vue app.
//!
//! Copied from vx's `vx-emit` (`augentic/vx`, `crates/vx-emit`) and trimmed to the deterministic
//! Vue path: the composition model, the copy and event contracts it derives, the tokens and their
//! CSS theme, and the Vue views. The Swift, Kotlin and Dioxus emitters, the Crux contract's Rust
//! output, capabilities, services and permissions did not come across. Input is JSON only.
//!
//! Everything here is deterministic. The same design system produces the same bytes, which is
//! what makes the output reviewable as a diff and testable as a golden — and what makes it safe
//! to overwrite on every run.
//!
//! # Ownership
//!
//! A generated file carries [`HEADER`] and is rewritten wholesale each time. A file without it is
//! somebody's, and is never touched. That line is also what lets stale output be pruned: a screen
//! deleted from the composition leaves a file behind, and deleting a file nobody claims is only
//! safe when the claim is written on it.

#![cfg_attr(
    test,
    allow(
        clippy::assert_is_empty,
        reason = "the unit tests are vx-emit's and vx-spec's, kept as written"
    )
)]

pub mod contract;
pub mod copy;
pub mod design;
pub mod format;
pub mod model;
pub mod naming;
pub mod spec;
pub mod theme;
pub mod tokens;
pub mod view;

use std::path::{Path, PathBuf};
use std::{fs, io};

use serde_json::Value;

/// The line every generated file carries.
pub const HEADER: &str = "Generated from design-system/ — do not edit manually.";

/// The substring that marks a file as this tool's to overwrite and to delete.
///
/// Deliberately shorter than [`HEADER`], so that a generated file whose header wording changes is
/// still recognised as one and pruned rather than orphaned.
pub const OWNED_MARKER: &str = "do not edit manually.";

/// Something that stopped the emit.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The composition is not the shape the schema guarantees.
    #[error("{0}")]
    Malformed(String),

    /// A design-system document could not be read as JSON.
    #[error("{}: {source}", .path.display())]
    Parse {
        /// The file that would not parse.
        path: PathBuf,

        /// What the parser said.
        source: serde_json::Error,
    },

    /// A file the emit would overwrite is not one it wrote.
    ///
    /// Refused rather than clobbered. Somebody edited a generated file and removed its header, or
    /// wrote their own at a path the emitter now claims, and either way overwriting it silently
    /// loses work.
    #[error(
        "{} exists and is not generated — it has no `{OWNED_MARKER}` line, so it will not be \
         overwritten",
        .0.display()
    )]
    NotOurs(PathBuf),

    /// The filesystem refused something.
    #[error("{}: {source}", .path.display())]
    Io {
        /// The path being read or written.
        path: PathBuf,

        /// What the filesystem said.
        source: io::Error,
    },
}

/// One file the emit produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedFile {
    /// Where it goes, relative to the app workspace root.
    pub path: String,

    /// What goes in it.
    pub body: String,
}

impl GeneratedFile {
    /// A file at `path` holding `body`.
    #[must_use]
    pub fn new(path: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            body: body.into(),
        }
    }
}

/// What an emit did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// Files whose contents changed, in path order.
    pub written: Vec<String>,

    /// Files already holding exactly what would be written.
    pub unchanged: Vec<String>,

    /// Generated files deleted because nothing produces them any more.
    pub pruned: Vec<String>,
}

impl Report {
    /// Whether the workspace was left as it was found.
    #[must_use]
    pub const fn is_clean(&self) -> bool {
        self.written.is_empty() && self.pruned.is_empty()
    }
}

/// Reads and parses a design-system document.
///
/// # Errors
///
/// When the file cannot be read or is not JSON.
pub fn read(path: &Path) -> Result<Value, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;

    serde_json::from_str(&text).map_err(|source| Error::Parse {
        path: path.to_owned(),
        source,
    })
}

/// Whether a reconciliation changes the workspace or only measures it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Write and prune.
    Apply,

    /// Touch nothing, and report what [`Mode::Apply`] would have done.
    Check,
}

/// Writes a set of generated files into an app workspace, pruning stale ones beside them.
///
/// A file whose contents already match is left alone, so that an emit that changes nothing leaves
/// no mtimes behind for a build system to react to.
///
/// # Errors
///
/// When a destination exists and is not generated, or when the filesystem refuses.
pub fn write(root: &Path, files: &[GeneratedFile], prune: &[&Path]) -> Result<Report, Error> {
    reconcile(root, files, prune, Mode::Apply)
}

/// Reports what [`write()`] would change, without changing it.
///
/// This is what makes emitter drift a CI failure rather than a surprise. Every file here is derived
/// from the design system, so a difference between what is on disk and what the emitters produce
/// means one of three things, and all three are worth failing a build over: the design system moved
/// and nobody re-emitted, somebody hand-edited generated output, or the emitters changed and the
/// committed output is stale.
///
/// Compares whole file bodies rather than mtimes or hashes of inputs. Slower and completely
/// unambiguous, which is the right trade for a check whose whole job is to be believed.
///
/// # Errors
///
/// When a destination exists and is not generated, or when a file cannot be read.
pub fn drift(root: &Path, files: &[GeneratedFile], prune: &[&Path]) -> Result<Report, Error> {
    reconcile(root, files, prune, Mode::Check)
}

fn reconcile(
    root: &Path, files: &[GeneratedFile], prune: &[&Path], mode: Mode,
) -> Result<Report, Error> {
    let mut report = Report::default();

    for file in files {
        let path = root.join(&file.path);

        if path.exists() {
            let existing = fs::read_to_string(&path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;

            // Refused in both modes. A generated path occupied by somebody's own file is a
            // collision that has to be resolved by a person either way, and a check that stayed
            // quiet about it would just defer the same failure to the next real emit.
            if !existing.contains(OWNED_MARKER) {
                return Err(Error::NotOurs(path));
            }
            if existing == file.body {
                report.unchanged.push(file.path.clone());
                continue;
            }
        }

        report.written.push(file.path.clone());

        if mode == Mode::Check {
            continue;
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_owned(),
                source,
            })?;
        }
        fs::write(&path, &file.body).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
    }

    let kept: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    for directory in prune {
        report.pruned.extend(match mode {
            Mode::Apply => stale(root, directory, &kept)?,
            Mode::Check => orphans(root, directory, &kept)?,
        });
    }
    report.pruned.sort();

    Ok(report)
}

/// Deletes generated files in `directory` that this emit did not produce.
fn stale(root: &Path, directory: &Path, kept: &[&str]) -> Result<Vec<String>, Error> {
    let orphaned = orphans(root, directory, kept)?;

    for relative in &orphaned {
        let path = root.join(relative);
        fs::remove_file(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
    }

    Ok(orphaned)
}

/// Generated files in `directory` that this emit did not produce, without deleting them.
///
/// Only files carrying [`OWNED_MARKER`] are considered. A directory holding both generated and
/// handwritten files is the normal case, and the marker is the only thing that tells them apart.
fn orphans(root: &Path, directory: &Path, kept: &[&str]) -> Result<Vec<String>, Error> {
    let absolute = root.join(directory);
    if !absolute.is_dir() {
        return Ok(Vec::new());
    }

    let mut orphaned = Vec::new();
    let entries = fs::read_dir(&absolute).map_err(|source| Error::Io {
        path: absolute.clone(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: absolute.clone(),
            source,
        })?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let relative = directory.join(entry.file_name()).to_string_lossy().replace('\\', "/");
        if kept.contains(&relative.as_str()) {
            continue;
        }

        if fs::read_to_string(&path).unwrap_or_default().contains(OWNED_MARKER) {
            orphaned.push(relative);
        }
    }

    Ok(orphaned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generated(path: &str, body: &str) -> GeneratedFile {
        GeneratedFile {
            path: path.to_owned(),
            body: format!("//! {HEADER}\n{body}"),
        }
    }

    #[test]
    fn a_file_that_already_says_what_it_would_say_is_left_alone() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let files = [generated("shared/src/contract.rs", "struct A;")];

        let first = write(dir.path(), &files, &[]).expect("the first write");
        assert_eq!(first.written, ["shared/src/contract.rs"]);

        let second = write(dir.path(), &files, &[]).expect("the second write");
        assert!(second.written.is_empty());
        assert_eq!(second.unchanged, ["shared/src/contract.rs"]);
        assert!(second.is_clean());
    }

    #[test]
    fn a_file_nobody_generated_is_not_overwritten() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("shared/src/contract.rs");
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(&path, "// mine\n").expect("a file");

        let error = write(dir.path(), &[generated("shared/src/contract.rs", "")], &[])
            .expect_err("the file is not ours");

        assert!(matches!(error, Error::NotOurs(_)));
        assert_eq!(fs::read_to_string(&path).expect("mine"), "// mine\n");
        assert!(!error.to_string().contains("delete it once"), "{error}");
    }

    #[test]
    fn a_screen_that_was_deleted_takes_its_file_with_it() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let directory = Path::new("shared/src/contract");

        let before = [
            generated("shared/src/contract/task_list.rs", "struct A;"),
            generated("shared/src/contract/task_detail.rs", "struct B;"),
        ];
        write(dir.path(), &before, &[directory]).expect("the first write");

        let after = [generated("shared/src/contract/task_list.rs", "struct A;")];
        let report = write(dir.path(), &after, &[directory]).expect("the second write");

        assert_eq!(report.pruned, ["shared/src/contract/task_detail.rs"]);
        assert!(!dir.path().join("shared/src/contract/task_detail.rs").exists());
        assert!(dir.path().join("shared/src/contract/task_list.rs").exists());
    }

    #[test]
    fn a_workspace_holding_exactly_the_emitted_output_has_not_drifted() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let files = [generated("shared/src/contract.rs", "struct A;")];

        write(dir.path(), &files, &[]).expect("the write");
        let report = drift(dir.path(), &files, &[]).expect("the check");

        assert!(report.is_clean());
        assert_eq!(report.unchanged, ["shared/src/contract.rs"]);
    }

    /// The whole point of the check: it has to notice, and it has to notice without repairing.
    #[test]
    fn a_hand_edited_generated_file_is_drift_and_stays_hand_edited() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let files = [generated("shared/src/contract.rs", "struct A;")];
        write(dir.path(), &files, &[]).expect("the write");

        let path = dir.path().join("shared/src/contract.rs");
        let edited = format!("//! {HEADER}\nstruct Edited;");
        fs::write(&path, &edited).expect("a hand edit");

        let report = drift(dir.path(), &files, &[]).expect("the check");

        assert_eq!(report.written, ["shared/src/contract.rs"], "the drift went unnoticed");
        assert!(!report.is_clean());
        assert_eq!(fs::read_to_string(&path).expect("still edited"), edited, "the check wrote");
    }

    #[test]
    fn a_file_the_emitters_no_longer_produce_is_drift_and_is_not_deleted() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let directory = Path::new("shared/src/contract");

        let before = [
            generated("shared/src/contract/task_list.rs", "struct A;"),
            generated("shared/src/contract/task_detail.rs", "struct B;"),
        ];
        write(dir.path(), &before, &[directory]).expect("the write");

        let after = [generated("shared/src/contract/task_list.rs", "struct A;")];
        let report = drift(dir.path(), &after, &[directory]).expect("the check");

        assert_eq!(report.pruned, ["shared/src/contract/task_detail.rs"]);
        assert!(
            dir.path().join("shared/src/contract/task_detail.rs").exists(),
            "the check deleted a file"
        );
    }

    #[test]
    fn an_unemitted_workspace_is_entirely_drift() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let files = [generated("shared/src/contract.rs", "struct A;")];

        let report = drift(dir.path(), &files, &[]).expect("the check");

        assert_eq!(report.written, ["shared/src/contract.rs"]);
        assert!(!dir.path().join("shared/src/contract.rs").exists(), "the check wrote");
    }

    /// A collision with somebody's own file has to be resolved by a person, so staying quiet about
    /// it in check mode would only defer the same failure to the next real emit.
    #[test]
    fn a_file_nobody_generated_is_refused_by_the_check_too() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = dir.path().join("shared/src/contract.rs");
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(&path, "// mine\n").expect("a file");

        let error = drift(dir.path(), &[generated("shared/src/contract.rs", "")], &[])
            .expect_err("the file is not ours");

        assert!(matches!(error, Error::NotOurs(_)));
    }

    #[test]
    fn a_handwritten_file_beside_a_generated_one_survives_the_prune() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let directory = Path::new("shared/src/contract");

        write(dir.path(), &[generated("shared/src/contract/a.rs", "")], &[directory])
            .expect("the first write");

        let mine = dir.path().join("shared/src/contract/helpers.rs");
        fs::write(&mine, "// mine\n").expect("a file");

        let report = write(dir.path(), &[], &[directory]).expect("the second write");

        assert_eq!(report.pruned, ["shared/src/contract/a.rs"]);
        assert!(mine.exists(), "a file without the marker is nobody's to delete");
    }
}
