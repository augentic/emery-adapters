//! Builds each slice of a plan into an Omnia guest in Rust, and verifies the integrated tree.
//!
//! Initialises the repository with the guest template of [`omnia_template`],
//! scaffolds each slice into its stem's member crate whose routers the root
//! component merges, holds the tree to the layout's rules before accepting a
//! report or verdict, and runs `cargo clippy`, `cargo test`, and the
//! `wasm32-wasip2` check at each slice and wave.

mod change;
mod layout;
mod scaffold;
mod syntax;
mod tree;

use std::num::NonZeroU32;
use std::path::Path;

use emery_sdk::target::{self, Grant, MergeRule, Metadata, Outcome, Report, TargetAdapter, Task};
use emery_sdk::{Error, Model, push_unique, tracing};

use self::change::Change;
use self::scaffold::Scaffold;
use self::tree::Tree;

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    fn metadata() -> Metadata {
        let cargo = Grant {
            egress: vec!["index.crates.io".into(), "static.crates.io".into(), "github.com".into()],
            ..Grant::default()
        };
        let rustup = Grant {
            read: vec!["~/.rustup".into(), "~/.cargo/bin".into()],
            env: vec![("RUSTUP_HOME".into(), "~/.rustup".into())],
            ..Grant::default()
        };
        Metadata {
            rules: vec![
                MergeRule::ours("Cargo.lock"),
                MergeRule::union("Cargo.toml"),
                MergeRule::union("src/lib.rs"),
                MergeRule::union("crates/common/src/topic.rs"),
                MergeRule::union("crates/*/Cargo.toml"),
                MergeRule::union("crates/*/src/lib.rs"),
            ],
            grants: vec![cargo, rustup],
            budget: NonZeroU32::new(12),
        }
    }

    async fn run<P: Model>(task: Task<'_, P>) -> Result<Outcome, Error> {
        match task {
            Task::Init(ctx) => {
                let scaffolded = omnia_template::scaffold(Path::new(ctx.workspace))
                    .map_err(|err| layout::refusal(&err))?;
                tracing::info!(scaffolded = scaffolded.len(), "scaffolded the guest template");
                Ok(Outcome::Initialised)
            }
            Task::Scope(ctx) => Ok(Outcome::Scoped(target::scope::conservative(ctx.change))),
            Task::Build(ctx) => {
                let root = Path::new(ctx.workspace);
                let change = Change::read(ctx.slice, &Tree::read(root)?);
                let plan = Scaffold::plan(ctx.slice, root, &change)?;
                if plan.is_empty() {
                    tracing::info!(
                        slice = ctx.slice.id,
                        retiring = change.is_retiring(),
                        "nothing to scaffold and no touch points; no turn"
                    );
                    return Ok(Outcome::Built(Report {
                        covered: change.kept_requirements().to_vec(),
                        written: Vec::new(),
                    }));
                }
                let scaffold = plan.write(root)?;
                tracing::info!(
                    scaffolded = scaffold.written().len(),
                    "scaffolded the slice's skeleton"
                );
                let prepared = scaffold.prepare(PROSE);
                let ctx = ctx.with_scaffold(prepared.as_target());
                let mut report = target::build(&ctx, PROSE, |report| {
                    Tree::read(root)?.check_build(&scaffold, report)
                })
                .await?;
                for id in change.kept_requirements() {
                    push_unique(&mut report.covered, id.clone());
                }
                Ok(Outcome::Built(report))
            }
            Task::Verify(ctx) => {
                let root = Path::new(ctx.workspace);
                target::verify(&ctx, PROSE, |_| Tree::read(root)?.check_verify())
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
