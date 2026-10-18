//! Builds each slice of a plan into an Omnia guest in Rust, and verifies the integrated tree.
//!
//! Initialises the repository with the guest template of [`omnia_template`],
//! builds each stem as a member crate whose routers the root component
//! merges, holds the tree to the layout's rules before accepting a report or
//! verdict, and runs `cargo clippy`, `cargo test`, and the `wasm32-wasip2`
//! check at each slice and wave.

mod tree;

use std::path::Path;

use emery_sdk::target::{Grant, MergeRule, Metadata, Outcome, TargetAdapter, Task};
use emery_sdk::{Error, Model, tracing};

use self::tree::{Stems, Tree};

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
                let laid = omnia_template::lay(Path::new(ctx.workspace))
                    .map_err(|err| tree::refusal(&err))?;
                tracing::info!(laid = laid.len(), "laid the guest template");
                Ok(Outcome::Initialised)
            }
            Task::Build(ctx) => {
                let root = Path::new(ctx.workspace);
                let stems = Stems::read(&ctx.slice.spec, root)?;
                emery_sdk::target::build(&ctx, PROSE, |_| Tree::read(root)?.check_build(&stems))
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
