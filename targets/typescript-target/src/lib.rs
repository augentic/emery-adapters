//! Builds each slice of a plan as strict TypeScript on Node, and verifies the integrated tree.
//!
//! A slice lands as one module directory, `src/<stem>/`, with its acceptance
//! scenarios as tests under `test/<stem>.test.ts`, and is exported from the
//! package entry `src/index.ts` by one line of its own. The slice that owns
//! the bootstrap lays the manifests; every other slice lays the same bytes
//! when it finds none, and adds a dependency only when its module needs one.
//! A build runs `npm install`, `tsc --noEmit`, and `npm test` through the
//! shell before it reports, and a wave is verified by the same checks over
//! the integrated tree, with `git status` for what the checks wrote outside
//! the lockfile.
//!
//! # Merge rules
//!
//! - `package-lock.json` keeps the integrated side; the next check
//!   regenerates it.
//! - `src/index.ts`, and every `index.ts` beneath it, keeps both sides'
//!   lines: the entry is a list of one line per module.
//! - `package.json` is unruled, so two slices that both change it conflict
//!   and the second is rebuilt over the first.

#[cfg(target_arch = "wasm32")]
mod guest {
    use std::borrow::Cow;

    use emery_sdk::target::{
        Context, MergeRule, MergeStrategy, Report, TargetAdapter, Verdict, VerifyContext,
    };
    use emery_sdk::{Error, Model};

    use crate::PROSE;

    struct Adapter;

    emery_sdk::target_adapter!(Adapter);

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
}

/// The prompts and the reference embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] =
    emery_sdk::prose!["../prose/build.md", "../prose/verify.md", "../prose/references/layout.md",];
