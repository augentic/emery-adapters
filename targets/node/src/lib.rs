//! Builds each slice of a plan as strict TypeScript on Node, and verifies the integrated tree.
//!
//! A slice lands in a directory of its own: `src/<stem>/` when the slice is
//! named by its stem and builds it whole, `src/<stem>/<part>/` otherwise, so
//! the slices a stem is cut into never write one file. Each directory holds
//! its module, its `index.ts`, and a `package.json` naming the packages its
//! files import, which the static root manifest's `workspaces` installs; its
//! acceptance scenarios are tests under `test/<stem>.test.ts` or
//! `test/<stem>/<part>.test.ts`. The entry `src/index.ts`, and a cut stem's
//! `src/<stem>/index.ts`, are lists of one line per module. A module the
//! application reaches exports `register(app)`, and the `start` module's
//! `main()` calls every one the entry reaches, so no slice edits another's
//! directory to be wired. A build writes through `write_files`, runs
//! `npm install`, `tsc --noEmit`, and `npm test` through the shell, and
//! reports once they pass; a wave is verified by the same checks over the
//! integrated tree, repaired where they fail, and run again before the
//! verdict.
//!
//! # Merge rules
//!
//! - `package-lock.json` keeps the integrated side; the next check
//!   regenerates it.
//! - `src/index.ts`, and every `index.ts` beneath it, keeps both sides'
//!   lines: each is a list of one line per module.
//! - Every other path is unruled: the root manifests are static and each
//!   directory's manifest is its slice's alone, so a conflict is a slice
//!   writing outside its directory, rebuilt over the merged tree.

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
            paths: Cow::Borrowed("package-lock.json"),
            strategy: MergeStrategy::Ours,
        },
        MergeRule {
            paths: Cow::Borrowed("src/index.ts"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("**/index.ts"),
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

/// The prompts and the reference embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] =
    emery_sdk::prose!["../prose/build.md", "../prose/verify.md", "../prose/references/layout.md",];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
