//! Experiment / non-production: rebuilds a web page as Vue from its rendered-layout facts.
//!
//! The slice carries the page's facts as its layout. A build puts two
//! turns: an extraction into one static `src/Page.vue`, then a refactor
//! into `src/Page.vue` and components from the facts stripped of their
//! document structure. Each turn captures its own render in a browser, and
//! the check scores that capture against the original facts, naming the
//! red overlays of a failing score. A verification captures the integrated
//! tree once more and holds the verdict to the score.

mod check;
mod tree;

use std::num::NonZeroU32;

use emery_sdk::target::{Grant, MergeRule, Metadata, Outcome, TargetAdapter, Task};
use emery_sdk::{Doc, Error, Model};

const RULES: &[MergeRule] = &[MergeRule::ours("pnpm-lock.yaml")];
const NPM: Grant = Grant {
    egress: &["registry.npmjs.org"],
    read: &[],
    env: &[],
};
const NVM: Grant = Grant {
    egress: &[],
    read: &["~/.nvm/versions/node"],
    env: &[],
};
const VOLTA: Grant = Grant {
    egress: &[],
    read: &["~/.volta"],
    env: &[("VOLTA_HOME", "~/.volta")],
};
const FNM: Grant = Grant {
    egress: &[],
    read: &["~/.local/share/fnm"],
    env: &[],
};
// Playwright's browsers, where `npx playwright install` puts them on macOS
// and on Linux; the browser runs inside the lend, so only its files are
// read. The worker's HOME is not the operator's, so each cache is named to
// the capture script, which takes the one of the platform it runs on.
const PLAYWRIGHT: Grant = Grant {
    egress: &[],
    read: &["~/Library/Caches/ms-playwright", "~/.cache/ms-playwright"],
    env: &[
        ("PLAYWRIGHT_CACHE_MACOS", "~/Library/Caches/ms-playwright"),
        ("PLAYWRIGHT_CACHE_LINUX", "~/.cache/ms-playwright"),
    ],
};

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            rules: RULES,
            grants: &[NPM, NVM, VOLTA, FNM, PLAYWRIGHT],
            budget: NonZeroU32::new(12),
        }
    }

    async fn run<P: Model>(task: Task<'_, P>) -> Result<Outcome, Error> {
        match task {
            Task::Init(ctx) => tree::init(ctx.workspace).map(|()| Outcome::Initialised),
            Task::Build(ctx) => tree::build(&ctx).await.map(Outcome::Built),
            Task::Verify(ctx) => tree::verify(&ctx).await.map(Outcome::Verified),
        }
    }
}

/// The prompts and references embedded in the adapter, at their paths under `prose/`.
pub static PROSE: &[Doc] = emery_sdk::prose![
    "../prose/build.md",
    "../prose/refactor.md",
    "../prose/verify.md",
    "../prose/references/capture.md",
    "../prose/references/facts.md",
    "../prose/references/tree.md",
    "../prose/references/vue-style.md",
];

const CAPTURE: Doc =
    Doc::new("references/capture.md", include_str!("../prose/references/capture.md"));
const FACTS: Doc = Doc::new("references/facts.md", include_str!("../prose/references/facts.md"));
const TREE: Doc = Doc::new("references/tree.md", include_str!("../prose/references/tree.md"));
const STYLE: Doc =
    Doc::new("references/vue-style.md", include_str!("../prose/references/vue-style.md"));

/// The extraction turn's table: `build.md` and the references it links.
pub static EXTRACT: &[Doc] =
    &[Doc::new("build.md", include_str!("../prose/build.md")), CAPTURE, FACTS, TREE];
/// The refactor turn's table, `refactor.md` put under the name `build.md`
/// the SDK prompts a build with.
pub static REFACTOR: &[Doc] =
    &[Doc::new("build.md", include_str!("../prose/refactor.md")), CAPTURE, FACTS, TREE, STYLE];
/// The verification turn's table: `verify.md` and the references it links.
pub static VERIFY: &[Doc] =
    &[Doc::new("verify.md", include_str!("../prose/verify.md")), CAPTURE, TREE];

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
