//! Builds each slice of a plan as an omnia HTTP guest in Rust, and verifies the integrated tree.
//!
//! The tree is one cargo workspace: a root package, `app`, whose component
//! serves every route over one provider, and one crate per stem under
//! `crates/<stem>/`. A slice named by its stem builds the crate whole; any
//! other slice builds a part of it, a module `src/<part>.rs` with its tests
//! under `tests/<part>.rs`, so the slices a stem is cut into never write one
//! module. Each slice reaches the application through a `mount` fn and one
//! line in the root router, and nothing of the root is edited beyond that
//! line and the stem's line in the root manifest. A build writes through
//! `write_files`, runs `cargo clippy`, `cargo test`, and a `wasm32-wasip2`
//! check through the shell, and reports once they pass; a wave is verified
//! by the same checks over the integrated tree, repaired where they fail,
//! and run again before the verdict.
//!
//! # Merge rules
//!
//! - `Cargo.lock` keeps the integrated side; the next check regenerates it.
//! - `Cargo.toml`, the root's and every stem's, keeps both sides' lines: each
//!   ends in one list slices add a line to.
//! - `src/lib.rs` at the root, and `crates/<stem>/src/lib.rs` where the stem
//!   is built in parts, keep both sides' lines for the same reason.
//! - Every other path is unruled: a part's module and tests are its slice's
//!   alone, so a conflict is a slice writing outside its part, rebuilt over
//!   the merged tree.

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
            paths: Cow::Borrowed("Cargo.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("src/lib.rs"),
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
