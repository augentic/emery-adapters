//! Builds each slice of a plan into an omnia guest in Rust, in the exemplar's shape, and verifies the integrated tree.
//!
//! The tree is the root-package guest [`omnia-exemplar`](https://github.com/augentic/omnia-exemplar)
//! teaches: the root package `guest` is the component, exporting WASI HTTP
//! and WASI Messaging over one provider and building both routers from two
//! lists, `src/routes.rs` and `src/messaging.rs`; `crates/common` holds the
//! route and topic tables and the configuration catalog every stem shares;
//! and each stem is a crate under `crates/<stem>/`, every slice a module of
//! it with its tests beside it, so two slices never write one file. The
//! exemplar's guest template, rendered for the package `guest`, is the
//! adapter's [`SEED`], laid in a tree that lacks it before any slice's turn
//! with the lists empty, so a slice appends its lines and copies nothing. A
//! build writes through `write_files` and `edit_files`, runs `cargo clippy`,
//! `cargo test`, and a `wasm32-wasip2` check through the shell, and reports
//! once they pass; a wave is verified by the same checks over the integrated
//! tree, with the files no slice edits held to the seed, repaired where they
//! fail, and run again before the verdict.
//!
//! The root `Cargo.toml` is held to the seed by the adapter itself, outside
//! its two stem lists: once a slice's turn ends, every line of it that is
//! neither the seed's nor a stem's entry is put back as the seed spells it
//! and the entries kept, so a pin, a lint, the `host` feature, or a target
//! table a slice changed never reaches its commit; a wave's verify restores
//! it the same way before the checks run, and a verify whose repairs changed
//! it fails with the lines named.
//!
//! The worker a turn's checks run in reaches the Rust toolchain beneath the
//! operator's home, `~/.rustup` and `~/.cargo/bin`, with `RUSTUP_HOME` set
//! to the former, and the crates.io index and its downloads through the
//! backend's proxy; nothing else beyond the lent tree.
//!
//! The adapter builds into a fresh tree: the seed and the slices are the
//! whole of what it lays, so it declares a [`Base::Fresh`] and the build
//! refuses a base whose root holds a directory rather than let the seed and
//! the verify rewrite a project it was pointed at.
//!
//! # Merge rules
//!
//! - `Cargo.lock` keeps the integrated side; the next check regenerates it.
//! - The root `Cargo.toml`, the two router lists, the two root test tables,
//!   `clippy.toml`, the tables and the catalog under `crates/common/`, every
//!   `crates/<stem>/Cargo.toml`, and every `crates/<stem>/src/lib.rs` keep
//!   both sides' lines: each holds a list slices add a line to.
//! - Every other path is unruled. `src/lib.rs` and the rest of the seed no
//!   slice edits, so two slices changing one is a conflict and never a
//!   merge; a module and its tests are one slice's alone, so a conflict there
//!   is a slice writing outside its part, rebuilt over the merged tree.

mod manifest;

use std::borrow::Cow;

use emery_sdk::target::{
    Base, Context, MergeRule, MergeStrategy, Report, Seed, TargetAdapter, Verdict, VerifyContext,
    Worker,
};
use emery_sdk::{Error, Model, tracing};

use self::manifest::Manifest;

const ROOT_MANIFEST: &str = include_str!("../seed/Cargo.toml");

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
        let mut report = emery_sdk::target::build(ctx, PROSE, SEED).await?;

        // the slice's commit carries the seed's pins whatever its turn did to them
        let restored = Manifest::new(ctx.workspace, ROOT_MANIFEST).restore()?;
        if !restored.is_empty() {
            tracing::warn!(
                slice = %ctx.slice.id,
                lines = restored.len(),
                "the root manifest changed outside its lists; restored"
            );
            tracing::debug!(slice = %ctx.slice.id, %restored, "the lines restored");
            if !report.written.iter().any(|path| path == manifest::FILE) {
                report.written.push(manifest::FILE.to_owned());
                report.written.sort();
            }
        }

        Ok(report)
    }

    async fn verify<P: Model>(ctx: &VerifyContext<'_, P>) -> Result<Verdict, Error> {
        let manifest = Manifest::new(ctx.workspace, ROOT_MANIFEST);

        // the checks run over the seed's pins, so a drift inherited from the
        // integrated tree is a repair the wave seals
        let restored = manifest.restore()?;
        if !restored.is_empty() {
            tracing::warn!(lines = restored.len(), "the root manifest restored before the checks");
            tracing::debug!(%restored, "the lines restored");
        }

        let mut verdict = emery_sdk::target::verify(ctx, PROSE).await?;

        // a repair that changed the pins voids the checks that ran over them
        let drift = manifest.drift()?;
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
    "../prose/references/seed.md",
    "../prose/references/handlers.md",
    "../prose/references/omnia-api.md",
    "../prose/references/testing.md",
    "../prose/references/guardrails.md",
];

/// The files every tree of the layout starts from, laid where a working copy lacks them.
///
/// Byte for byte the files `seed.md` fences: the exemplar's guest template
/// rendered for the package `guest` — the root manifest and its pins, the
/// component's entry, the toolchain, the formatter, lint, deny, and vet
/// configuration, the task runner, and the workflows — with the two router
/// lists, the two root test tables, the local host, and the `common` crate
/// beside them. A slice finds them in place and copies none.
pub static SEED: &[Seed] = &[
    Seed::new("Cargo.toml", ROOT_MANIFEST),
    Seed::new("src/lib.rs", include_str!("../seed/src/lib.rs")),
    Seed::new("src/routes.rs", include_str!("../seed/src/routes.rs")),
    Seed::new("src/messaging.rs", include_str!("../seed/src/messaging.rs")),
    Seed::new("tests/routes.rs", include_str!("../seed/tests/routes.rs")),
    Seed::new("tests/messaging.rs", include_str!("../seed/tests/messaging.rs")),
    Seed::new("examples/runtime.rs", include_str!("../seed/examples/runtime.rs")),
    Seed::new("crates/common/Cargo.toml", include_str!("../seed/crates/common/Cargo.toml")),
    Seed::new("crates/common/src/lib.rs", include_str!("../seed/crates/common/src/lib.rs")),
    Seed::new("crates/common/src/routes.rs", include_str!("../seed/crates/common/src/routes.rs")),
    Seed::new(
        "crates/common/src/routes/http.rs",
        include_str!("../seed/crates/common/src/routes/http.rs"),
    ),
    Seed::new(
        "crates/common/src/routes/topic.rs",
        include_str!("../seed/crates/common/src/routes/topic.rs"),
    ),
    Seed::new("crates/common/src/config.rs", include_str!("../seed/crates/common/src/config.rs")),
    Seed::new("Makefile", include_str!("../seed/Makefile")),
    Seed::new("mise.toml", include_str!("../seed/mise.toml")),
    Seed::new("rust-toolchain.toml", include_str!("../seed/rust-toolchain.toml")),
    Seed::new("rustfmt.toml", include_str!("../seed/rustfmt.toml")),
    Seed::new("taplo.toml", include_str!("../seed/taplo.toml")),
    Seed::new("clippy.toml", include_str!("../seed/clippy.toml")),
    Seed::new("deny.toml", include_str!("../seed/deny.toml")),
    Seed::new(".gitignore", include_str!("../seed/.gitignore")),
    Seed::new(".vscode/settings.json", include_str!("../seed/.vscode/settings.json")),
    Seed::new("supply-chain/README.md", include_str!("../seed/supply-chain/README.md")),
    Seed::new("supply-chain/config.toml", include_str!("../seed/supply-chain/config.toml")),
    Seed::new("supply-chain/audits.toml", include_str!("../seed/supply-chain/audits.toml")),
    Seed::new(".github/dependabot.yml", include_str!("../seed/.github/dependabot.yml")),
    Seed::new(".github/workflows/audit.yaml", include_str!("../seed/.github/workflows/audit.yaml")),
    Seed::new(".github/workflows/ci.yaml", include_str!("../seed/.github/workflows/ci.yaml")),
    Seed::new(".github/workflows/patch.yaml", include_str!("../seed/.github/workflows/patch.yaml")),
    Seed::new(
        ".github/workflows/release.yaml",
        include_str!("../seed/.github/workflows/release.yaml"),
    ),
    Seed::new(
        ".github/workflows/publish.yaml",
        include_str!("../seed/.github/workflows/publish.yaml"),
    ),
];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
