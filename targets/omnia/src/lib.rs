//! Builds each slice of a plan as an omnia HTTP guest in Rust, and verifies the integrated tree.
//!
//! The tree is one cargo workspace: a root manifest holding every shared pin,
//! a root crate `app/` whose component serves every route over one provider,
//! and one crate per stem under `crates/<stem>/`. Every slice builds one
//! module of its stem's crate, `src/<part>.rs` with its tests under
//! `tests/<part>.rs`, so two slices never write one module. Each slice
//! reaches the application through a `mount` fn and one line in the router
//! list, and edits nothing outside `crates/` beyond that line and the stem's
//! line in the root crate's manifest. A build writes through `write_files`,
//! runs `cargo clippy`, `cargo test`, and a `wasm32-wasip2` check through the
//! shell, and reports once they pass; a wave is verified by the same checks
//! over the integrated tree, with the files no slice edits held to the
//! layout, repaired where they fail, and run again before the verdict.
//!
//! # Merge rules
//!
//! - `Cargo.lock` keeps the integrated side; the next check regenerates it.
//! - `app/Cargo.toml` and every `crates/<stem>/Cargo.toml` keep both sides'
//!   lines: each ends in one list slices add a line to.
//! - `app/src/routes.rs`, the router list, and every `crates/<stem>/src/lib.rs`,
//!   a stem's module list, keep both sides' lines for the same reason.
//! - Every other path is unruled. The root `Cargo.toml` and `app/src/lib.rs`
//!   hold the pins and the component's entry, which no slice edits, so two
//!   slices changing one is a conflict and never a merge; a module and its
//!   tests are one slice's alone, so a conflict there is a slice writing
//!   outside its part, rebuilt over the merged tree.

use std::borrow::Cow;

use emery_sdk::target::{
    Context, MergeRule, MergeStrategy, Report, TargetAdapter, Verdict, VerifyContext,
};
use emery_sdk::{Error, Model};

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    const MERGE_RULES: &'static [MergeRule] = &[
        MergeRule {
            paths: Cow::Borrowed("Cargo.lock"),
            strategy: MergeStrategy::Ours,
        },
        MergeRule {
            paths: Cow::Borrowed("app/Cargo.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/*/Cargo.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("app/src/routes.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/*/src/lib.rs"),
            strategy: MergeStrategy::Union,
        },
    ];

    async fn build<P: Model>(ctx: &Context<'_, P>) -> Result<Report, Error> {
        emery_sdk::target::build(ctx, PROSE).await
    }

    async fn verify<P: Model>(ctx: &VerifyContext<'_, P>) -> Result<Verdict, Error> {
        emery_sdk::target::verify(ctx, PROSE).await
    }
}

/// The prompts and the references embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] = emery_sdk::prose![
    "../prose/build.md",
    "../prose/verify.md",
    "../prose/references/layout.md",
    "../prose/references/handlers.md",
    "../prose/references/testing.md",
    "../prose/references/guardrails.md",
];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
