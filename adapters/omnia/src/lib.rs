//! Builds each slice of a plan into an Omnia guest in Rust, and verifies the integrated tree.
//!
//! Seeds a fresh workspace from [`omnia_template`], builds each stem as a
//! member crate wired into the root component's HTTP and messaging routers,
//! holds the tree to the layout and host-boundary rules before accepting a
//! report or verdict, and runs `cargo clippy`, `cargo test`, and the
//! `wasm32-wasip2` check at each slice and wave.

mod tree;

use std::path::Path;

use emery_sdk::target::{
    Base, Context, MergeRule, Report, Seed, TargetAdapter, Verdict, VerifyContext, Worker,
};
use emery_sdk::{Error, Model};
use omnia_template::ENTRIES;

use self::tree::{Stems, Tree};

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    const BASE: Base = Base::Fresh;
    const MERGE_RULES: &'static [MergeRule] = &[
        MergeRule::ours("Cargo.lock"),
        MergeRule::union("Cargo.toml"),
        MergeRule::union("clippy.toml"),
        MergeRule::union("src/routes.rs"),
        MergeRule::union("src/messaging.rs"),
        MergeRule::union("tests/routes.rs"),
        MergeRule::union("tests/messaging.rs"),
        MergeRule::union("crates/common/src/routes/*.rs"),
        MergeRule::union("crates/common/src/config.rs"),
        MergeRule::union("crates/*/Cargo.toml"),
        MergeRule::union("crates/*/src/lib.rs"),
    ];
    const WORKER: Worker = Worker {
        egress: &["index.crates.io", "static.crates.io"],
        read: &["~/.rustup", "~/.cargo/bin"],
        env: &[("RUSTUP_HOME", "~/.rustup")],
    };

    async fn build<P: Model>(ctx: &Context<'_, P>) -> Result<Report, Error> {
        let root = Path::new(ctx.workspace);
        let ignored = tree::restore_ignore(root, &ctx.slice.id)?;
        let stems = Stems::read(&ctx.slice.spec, root)?;
        let mut report = emery_sdk::target::build(ctx, PROSE, &seed(), |_| {
            Tree::read(root)?.check_build(&stems)
        })
        .await?;
        if ignored {
            tree::record_ignore(&mut report);
        }

        Ok(report)
    }

    async fn verify<P: Model>(ctx: &VerifyContext<'_, P>) -> Result<Verdict, Error> {
        let root = Path::new(ctx.workspace);
        emery_sdk::target::verify(ctx, PROSE, |_| Tree::read(root)?.check_verify()).await
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
    "../prose/references/translation.md",
];

/// Returns the exemplar guest template's entries as [`Seed`]s.
#[must_use]
pub fn seed() -> Vec<Seed> {
    ENTRIES.iter().map(|entry| Seed::new(entry.path, entry.body)).collect()
}

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
