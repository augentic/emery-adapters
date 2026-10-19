//! Builds each slice of a plan into an Omnia guest in Rust, and verifies the integrated tree.
//!
//! Initialises the repository with the guest template of [`omnia_template`],
//! builds each stem as a member crate whose routers the root component
//! merges, holds the tree to the layout's rules before accepting a report or
//! verdict, and runs `cargo clippy`, `cargo test`, and the `wasm32-wasip2`
//! check at each slice and wave.
//!
//! Before a slice's turn, the adapter scaffolds the slice's skeleton from the
//! typed slice: the stem's crate where the tree lacks it, the part with a
//! handler per route the requirements state and a function per requirement
//! stating none, an ignored test per scenario, and the stem's lines in every
//! list. The turn fills the markers it scaffolded, and a covered requirement
//! whose marker or ignored test still stands is refused for correction, as
//! is a module the tree holds that does not parse, named at the line the
//! parser stopped at.

mod scaffold;
mod tree;

use std::path::Path;

use emery_sdk::target::{Grant, MergeRule, Metadata, Outcome, TargetAdapter, Task};
use emery_sdk::{Error, Model, tracing};

use self::scaffold::Scaffold;
use self::tree::Tree;

const RULES: &[MergeRule] = &[
    MergeRule::ours("Cargo.lock"),
    MergeRule::union("Cargo.toml"),
    MergeRule::union("src/routes.rs"),
    MergeRule::union("src/messaging.rs"),
    MergeRule::union("crates/common/src/topic.rs"),
    MergeRule::union("crates/*/Cargo.toml"),
    MergeRule::union("crates/*/src/lib.rs"),
];
const CARGO: Grant = Grant {
    egress: &["index.crates.io", "static.crates.io"],
    read: &[],
    env: &[],
};
const RUSTUP: Grant = Grant {
    egress: &[],
    read: &["~/.rustup", "~/.cargo/bin"],
    env: &[("RUSTUP_HOME", "~/.rustup")],
};

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            rules: RULES,
            grants: &[CARGO, RUSTUP],
        }
    }

    async fn run<P: Model>(task: Task<'_, P>) -> Result<Outcome, Error> {
        match task {
            Task::Init(ctx) => {
                let scaffolded = omnia_template::scaffold(Path::new(ctx.workspace))
                    .map_err(|err| tree::refusal(&err))?;
                tracing::info!(scaffolded = scaffolded.len(), "scaffolded the guest template");
                Ok(Outcome::Initialised)
            }
            Task::Build(ctx) => {
                let root = Path::new(ctx.workspace);
                let scaffold = Scaffold::plan(ctx.slice, root)?;
                let written = scaffold.write(root)?;
                tracing::info!(scaffolded = written.len(), "scaffolded the slice's skeleton");
                let brief = scaffold.brief();
                let ctx = ctx.with_scaffold(&brief);
                emery_sdk::target::build(&ctx, PROSE, |report| {
                    Tree::read(root)?.check_build(&scaffold, report)
                })
                .await
                .map(Outcome::Built)
            }
            Task::Verify(ctx) => {
                let root = Path::new(ctx.workspace);
                emery_sdk::target::verify(&ctx, PROSE, |_| Tree::read(root)?.check_verify())
                    .await
                    .map(Outcome::Verified)
            }
        }
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

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
