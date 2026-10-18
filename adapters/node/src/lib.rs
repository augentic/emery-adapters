//! Builds each slice of a plan as strict TypeScript on Node, and verifies the integrated tree.
//!
//! Initialises the repository with the three root manifests, builds into
//! that tree with one directory and manifest per slice under the root
//! `workspaces`, and runs `npm install`, `tsc --noEmit`, and `npm test` at
//! each slice and wave.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::Path;

use emery_sdk::target::{InitContext, MergeRule, Outcome, Reach, TargetAdapter, Task, Worker};
use emery_sdk::{Error, Model, server_error, tracing};

// The root files every tree of the layout starts from, which no slice writes.
const FILES: &[(&str, &str)] = &[
    (".gitignore", include_str!("../seed/.gitignore")),
    ("package.json", include_str!("../seed/package.json")),
    ("tsconfig.json", include_str!("../seed/tsconfig.json")),
];

// npm fetches from the public registry.
const NPM: Reach = Reach {
    egress: &["registry.npmjs.org"],
    read: &[],
    env: &[],
};

// nvm keeps each installed Node under its versions directory.
const NVM: Reach = Reach {
    egress: &[],
    read: &["~/.nvm/versions/node"],
    env: &[],
};

// Volta finds its toolchains under `VOLTA_HOME`, which the worker's private
// `HOME` would otherwise hide.
const VOLTA: Reach = Reach {
    egress: &[],
    read: &["~/.volta"],
    env: &[("VOLTA_HOME", "~/.volta")],
};

// fnm keeps its installs under the XDG data directory.
const FNM: Reach = Reach {
    egress: &[],
    read: &["~/.local/share/fnm"],
    env: &[],
};

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    const MERGE_RULES: &'static [MergeRule] = &[
        MergeRule::ours("package-lock.json"),
        MergeRule::union("src/index.ts"),
        MergeRule::union("**/index.ts"),
    ];
    const WORKER: Worker = Worker::of(&[NPM, NVM, VOLTA, FNM]);

    async fn run<P: Model>(task: Task<'_, P>) -> Result<Outcome, Error> {
        match task {
            Task::Init(ctx) => init(&ctx).map(|()| Outcome::Initialised),
            Task::Build(ctx) => {
                emery_sdk::target::build(&ctx, PROSE, |_| Ok(Vec::new())).await.map(Outcome::Built)
            }
            Task::Verify(ctx) => emery_sdk::target::verify(&ctx, PROSE, |_| Ok(Vec::new()))
                .await
                .map(Outcome::Verified),
        }
    }
}

/// The prompts and the reference embedded in the adapter.
pub static PROSE: &[emery_sdk::Doc] =
    emery_sdk::prose!["../prose/build.md", "../prose/verify.md", "../prose/references/layout.md",];

// Lays each root file the tree lacks; one the tree holds, a file or a link,
// stands as it is.
fn init(ctx: &InitContext<'_>) -> Result<(), Error> {
    let root = Path::new(ctx.workspace);
    let mut laid = 0;
    for (path, content) in FILES {
        let file = root.join(path);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| server_error!("making the directories of `{path}`: {err}"))?;
        }
        match OpenOptions::new().write(true).create_new(true).open(&file) {
            Ok(mut file) => {
                file.write_all(content.as_bytes())
                    .map_err(|err| server_error!("writing `{path}`: {err}"))?;
                laid += 1;
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
            Err(err) => return Err(server_error!("laying `{path}`: {err}")),
        }
    }
    tracing::info!(laid, "laid the root manifests");
    Ok(())
}

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
