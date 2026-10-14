//! Builds each slice of a plan into an omnia guest in Rust, in the exemplar's shape, and verifies the integrated tree.
//!
//! The tree is the root-package guest [`omnia-exemplar`](https://github.com/augentic/omnia-exemplar)
//! teaches: the root package `guest` is the component, exporting WASI HTTP
//! and WASI Messaging over one provider and building both routers from two
//! lists, `src/routes.rs` and `src/messaging.rs`; `crates/common` holds the
//! route and topic tables and the configuration catalog every stem shares;
//! and each stem is a crate under `crates/<stem>/`, every slice a module of
//! it with its tests beside it, so two slices never write one file. The
//! exemplar's guest template is compiled in as the [`omnia_template`]
//! crate and is the adapter's [`seed`], laid in a tree that lacks it before
//! any slice's turn with the lists empty, so a slice appends its lines and
//! copies nothing. A build writes through `write_files` and `edit_files`,
//! runs `cargo clippy`, `cargo test`, and a `wasm32-wasip2` check through
//! the shell, and reports once they pass; a wave is verified by the same
//! checks over the integrated tree, with every file no slice edits put back
//! as the template spells it before they run and held after.
//!
//! The static files are held by the adapter itself, not by the merge: a
//! wave's verify restores every file of the template outside the lists
//! before its checks run, and the root `Cargo.toml` outside its two stem
//! lists, so a pin, a lint, the `host` feature, or a target table a slice
//! changed is put back and the lines a slice added kept; a verify whose
//! repairs changed one fails with the lines named. Once a slice's turn ends,
//! its root `Cargo.toml` is restored the same way, so the change never
//! reaches its commit. Before a slice's turn, the root `.gitignore` is put
//! back as the template spells it: the repository the engine lays holds an
//! ignore file of its own, which the seed never writes over, and the turn's
//! build output would otherwise be sealed with the slice.
//!
//! A verify also fails a wave whose stems simulate the host: a module under
//! `crates/*/src` that silences the workspace's lints for its crate, spawns
//! a task, a timer, or a thread, or reaches the environment, the file
//! system, a socket, or a process is named in the verdict with its line,
//! whatever the turn's checks said, since each is a mechanism the host owns
//! and the translation lays nothing for.
//!
//! The worker a turn's checks run in reaches the Rust toolchain beneath the
//! operator's home, `~/.rustup` and `~/.cargo/bin`, with `RUSTUP_HOME` set
//! to the former, and the crates.io index and its downloads through the
//! backend's proxy; nothing else beyond the lent tree.
//!
//! The adapter builds into a fresh tree: the template and the slices are
//! the whole of what it lays, so it declares a [`Base::Fresh`] and the build
//! refuses a base whose root holds a directory rather than let the template
//! and the verify rewrite a project it was pointed at.
//!
//! # Merge rules
//!
//! - `Cargo.lock` keeps the integrated side; the next check regenerates it.
//! - The root `Cargo.toml`, the two router lists, the two root test tables,
//!   `clippy.toml`, the tables and the catalog under `crates/common/`, every
//!   `crates/<stem>/Cargo.toml`, and every `crates/<stem>/src/lib.rs` keep
//!   both sides' lines: each holds a list slices add a line to.
//! - Every other path is unruled. `src/lib.rs` and the rest of the template
//!   no slice edits, so two slices changing one is a conflict and never a
//!   merge; a module and its tests are one slice's alone, so a conflict there
//!   is a slice writing outside its part, rebuilt over the merged tree.
//!
//! # Errors
//!
//! A directory or a symbolic link standing where a file of the template
//! goes is `bad_request`; a file that cannot be read or written is
//! `server_error`.

use std::borrow::Cow;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::target::{
    Base, Context, MergeRule, MergeStrategy, Report, Seed, TargetAdapter, Verdict, VerifyContext,
    Worker,
};
use emery_sdk::{Error, Model, bad_request, server_error, tracing};
use omnia_template::{ENTRIES, Manifest};

// The root ignore file, which the engine lays a one-line copy of at a laid
// repository's root commit.
const IGNORE: &str = ".gitignore";

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    const BASE: Base = Base::Fresh;
    const MERGE_RULES: &'static [MergeRule] = &[
        MergeRule {
            paths: Cow::Borrowed("Cargo.lock"),
            strategy: MergeStrategy::Ours,
        },
        MergeRule {
            paths: Cow::Borrowed("Cargo.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("clippy.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("src/routes.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("src/messaging.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("tests/routes.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("tests/messaging.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/common/src/routes/*.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/common/src/config.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/*/Cargo.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/*/src/lib.rs"),
            strategy: MergeStrategy::Union,
        },
    ];
    const WORKER: Worker = Worker {
        egress: &["index.crates.io", "static.crates.io"],
        read: &["~/.rustup", "~/.cargo/bin"],
        env: &[("RUSTUP_HOME", "~/.rustup")],
    };

    async fn build<P: Model>(ctx: &Context<'_, P>) -> Result<Report, Error> {
        let root = Path::new(ctx.workspace);

        // the seed never writes over a held file, so the engine's one-line
        // ignore file would let the turn's `target/` into the slice's commit
        let ignored = restore_ignore(root)?;
        if ignored {
            tracing::debug!(slice = %ctx.slice.id, "the root ignore file put back as the template spells it");
        }

        let mut report = emery_sdk::target::build(ctx, PROSE, &seed()).await?;

        // the slice's commit carries the template's pins whatever its turn did to them
        let restored = Manifest::new(root).restore().map_err(|err| refusal(&err))?;
        if !restored.is_empty() {
            tracing::warn!(
                slice = %ctx.slice.id,
                "the root manifest changed outside its lists; restored"
            );
            tracing::debug!(slice = %ctx.slice.id, %restored, "the lines restored");
            written(&mut report, Manifest::PATH);
        }
        if ignored {
            written(&mut report, IGNORE);
        }

        Ok(report)
    }

    async fn verify<P: Model>(ctx: &VerifyContext<'_, P>) -> Result<Verdict, Error> {
        let root = Path::new(ctx.workspace);
        let manifest = Manifest::new(root);

        // the checks run over the template's static files and pins, so a
        // drift inherited from the integrated tree is a repair the wave seals
        let restored = omnia_template::restore(root).map_err(|err| refusal(&err))?;
        if !restored.is_empty() {
            tracing::warn!(
                files = restored.files().len(),
                "static files of the template restored before the checks"
            );
            tracing::debug!(%restored, "the files restored");
        }
        let restored = manifest.restore().map_err(|err| refusal(&err))?;
        if !restored.is_empty() {
            tracing::warn!("the root manifest restored before the checks");
            tracing::debug!(%restored, "the lines restored");
        }

        let mut verdict = emery_sdk::target::verify(ctx, PROSE).await?;

        // a repair that changed a static file or a pin voids the checks that ran over them
        let drift = omnia_template::drift(root).map_err(|err| refusal(&err))?;
        if !drift.is_empty() {
            verdict.passed = false;
            verdict.failures.push(format!("a static file of the template changed:\n{drift}"));
        }
        let drift = manifest.drift().map_err(|err| refusal(&err))?;
        if !drift.is_empty() {
            verdict.passed = false;
            verdict
                .failures
                .push(format!("the root `Cargo.toml` changed outside its two lists:\n{drift}"));
        }

        // a module that simulates the host passes every check the turn ran
        let simulated = simulated(root)?;
        if !simulated.is_empty() {
            verdict.passed = false;
            verdict.failures.push(format!(
                "a module simulates the host, which the translation lays nothing for:\n{}",
                simulated.join("\n")
            ));
        }

        Ok(verdict)
    }
}

// What no module of a stem spells, each a mechanism the host owns: the
// workspace's lints silenced for the crate, a task, a timer, a thread, or a
// reach past the component to the system.
const SIMULATED: &[(&str, &str)] = &[
    ("#![allow(", "the workspace's lints silenced"),
    ("tokio::spawn", "a task of its own"),
    ("tokio::time::", "a timer of its own"),
    ("std::thread::", "a thread of its own"),
    ("std::env::", "the environment read past the host"),
    ("std::fs::", "the file system reached past the host"),
    ("std::net::", "a socket of its own"),
    ("std::process::", "a process of its own"),
];

// Answers `<path>:<line>: <what>` for every line under a stem's `src/` that
// spells one of `SIMULATED`, sorted; a tree with no `crates/` answers none.
fn simulated(root: &Path) -> Result<Vec<String>, Error> {
    let mut findings = Vec::new();
    for stem in entries(&root.join("crates"))? {
        for file in sources(&stem.join("src"))? {
            let text = fs::read_to_string(&file)
                .map_err(|err| server_error!("reading `{}`: {err}", file.display()))?;
            let path = file.strip_prefix(root).unwrap_or(&file).display().to_string();
            for (number, line) in text.lines().enumerate() {
                for (needle, what) in SIMULATED {
                    if line.contains(needle) {
                        findings.push(format!("{path}:{}: {what} (`{needle}`)", number + 1));
                    }
                }
            }
        }
    }
    findings.sort();
    Ok(findings)
}

// The entries of `dir`, sorted; an absent `dir` has none.
fn entries(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(server_error!("listing `{}`: {err}", dir.display())),
    };
    let mut paths = listing
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| server_error!("listing `{}`: {err}", dir.display()))?;
    paths.sort();
    Ok(paths)
}

// Every `.rs` file beneath `dir`, symbolic links left unfollowed.
fn sources(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in entries(&dir)? {
            let meta = fs::symlink_metadata(&path)
                .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

/// The prompts and the references embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose![
    "../prose/build.md",
    "../prose/verify.md",
    "../prose/references/layout.md",
    "../prose/references/handlers.md",
    "../prose/references/omnia-api.md",
    "../prose/references/testing.md",
    "../prose/references/guardrails.md",
    "../prose/references/translation.md",
];

/// Returns the files every tree of the layout starts from, laid where a working copy lacks them.
///
/// Every entry of the exemplar's guest template, [`omnia_template::ENTRIES`],
/// as a [`Seed`] at its path with its body: the root manifest and its pins,
/// the component's entry, the toolchain, the formatter, lint, deny, and vet
/// configuration, the task runner, and the workflows, with the two router
/// lists, the two root test tables, the local host, and the `common` crate
/// beside them. A slice finds them in place and copies none.
#[must_use]
pub fn seed() -> Vec<Seed> {
    ENTRIES.iter().map(|entry| Seed::new(entry.path, entry.body)).collect()
}

// Puts the template's ignore file over the one the tree holds and answers
// whether it did; a tree holding none is left for the seed to lay.
fn restore_ignore(root: &Path) -> Result<bool, Error> {
    let entry = omnia_template::entry(IGNORE)
        .ok_or_else(|| server_error!("the template lays no `{IGNORE}`"))?;
    let path = root.join(entry.path);
    let held = match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => {
            fs::read(&path).map_err(|err| server_error!("reading `{IGNORE}`: {err}"))?
        }
        Ok(_) => return Err(bad_request!("a directory or symbolic link stands at `{IGNORE}`")),
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(server_error!("reading `{IGNORE}`: {err}")),
    };
    if held == entry.body.as_bytes() {
        return Ok(false);
    }
    fs::write(&path, entry.body).map_err(|err| server_error!("writing `{IGNORE}`: {err}"))?;
    Ok(true)
}

// Names `path` in the report once, the list kept sorted.
fn written(report: &mut Report, path: &str) {
    if !report.written.iter().any(|named| named == path) {
        report.written.push(path.to_owned());
        report.written.sort();
    }
}

fn refusal(err: &omnia_template::Error) -> Error {
    match err {
        omnia_template::Error::Occupied { .. } => bad_request!("{err}"),
        _ => server_error!("{err}"),
    }
}

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
