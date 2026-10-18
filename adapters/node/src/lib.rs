//! Builds each slice of a plan as strict TypeScript on Node, and verifies the integrated tree.
//!
//! Initialises the repository with the three root manifests, builds into
//! that tree with one directory and manifest per slice under the root
//! `workspaces`, and runs `npm install`, `tsc --noEmit`, and `npm test` at
//! each slice and wave.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::Path;

use emery_sdk::target::{Grant, InitContext, MergeRule, Metadata, Outcome, TargetAdapter, Task};
use emery_sdk::{Error, Model, server_error, tracing};

const FILES: &[(&str, &str)] = &[
    (".gitignore", include_str!("../seed/.gitignore")),
    ("package.json", include_str!("../seed/package.json")),
    ("tsconfig.json", include_str!("../seed/tsconfig.json")),
];
const RULES: &[MergeRule] = &[
    MergeRule::ours("package-lock.json"),
    MergeRule::union("src/index.ts"),
    MergeRule::union("**/index.ts"),
];

// npm fetches from the public registry.
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

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    fn metadata() -> Metadata {
        Metadata {
            rules: RULES,
            grants: &[NPM, NVM, VOLTA, FNM],
        }
    }

    async fn run<P: Model>(task: Task<'_, P>) -> Result<Outcome, Error> {
        match task {
            Task::Init(ctx) => init(&ctx).map(|()| Outcome::Initialised),
            Task::Build(ctx) => emery_sdk::target::build(&ctx, PROSE, None, |_| Ok(Vec::new()))
                .await
                .map(Outcome::Built),
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
