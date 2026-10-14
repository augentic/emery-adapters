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
//! reaches its commit.
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
use std::path::Path;

use emery_sdk::target::{
    Base, Context, MergeRule, MergeStrategy, Report, Seed, TargetAdapter, Verdict, VerifyContext,
    Worker,
};
use emery_sdk::{Error, Model, bad_request, server_error, tracing};
use omnia_template::{ENTRIES, Manifest};

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
        let mut report = emery_sdk::target::build(ctx, PROSE, &seed()).await?;

        // the slice's commit carries the template's pins whatever its turn did to them
        let restored =
            Manifest::new(Path::new(ctx.workspace)).restore().map_err(|err| refusal(&err))?;
        if !restored.is_empty() {
            tracing::warn!(
                slice = %ctx.slice.id,
                "the root manifest changed outside its lists; restored"
            );
            tracing::debug!(slice = %ctx.slice.id, %restored, "the lines restored");
            if !report.written.iter().any(|path| path == Manifest::PATH) {
                report.written.push(Manifest::PATH.to_owned());
                report.written.sort();
            }
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

        Ok(verdict)
    }
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

fn refusal(err: &omnia_template::Error) -> Error {
    match err {
        omnia_template::Error::Occupied { .. } => bad_request!("{err}"),
        _ => server_error!("{err}"),
    }
}

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
