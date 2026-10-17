//! Builds each slice of a plan as strict TypeScript on Node, and verifies the integrated tree.
//!
//! Builds into a fresh workspace with one directory and manifest per slice
//! under the root `workspaces`, running `npm install`, `tsc --noEmit`, and
//! `npm test` at each slice and wave.

use emery_sdk::target::{
    Base, Context, MergeRule, Report, TargetAdapter, Verdict, VerifyContext, Worker,
};
use emery_sdk::{Error, Model};

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    const BASE: Base = Base::Fresh;
    const MERGE_RULES: &'static [MergeRule] = &[
        MergeRule::ours("package-lock.json"),
        MergeRule::union("src/index.ts"),
        MergeRule::union("**/index.ts"),
    ];
    const WORKER: Worker = Worker {
        egress: &["registry.npmjs.org"],
        read: &["~/.nvm/versions/node", "~/.volta", "~/.local/share/fnm"],
        env: &[("VOLTA_HOME", "~/.volta")],
    };

    async fn build<P: Model>(ctx: &Context<'_, P>) -> Result<Report, Error> {
        emery_sdk::target::build(ctx, PROSE, &[], |_| Ok(Vec::new())).await
    }

    async fn verify<P: Model>(ctx: &VerifyContext<'_, P>) -> Result<Verdict, Error> {
        emery_sdk::target::verify(ctx, PROSE, |_| Ok(Vec::new())).await
    }
}

/// The prompts and the reference embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] =
    emery_sdk::prose!["../prose/build.md", "../prose/verify.md", "../prose/references/layout.md",];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
