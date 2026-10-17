//! Builds each slice of a plan into an omnia guest in Rust, in the exemplar's shape, and verifies the integrated tree.
//!
//! The tree is the root-package guest [`omnia-exemplar`](https://github.com/augentic/omnia-exemplar)
//! teaches: the root package `guest` is the component, exporting WASI HTTP
//! and WASI Messaging over one provider and building both routers from two
//! lists, `src/routes.rs` and `src/messaging.rs`; `crates/common` holds the
//! route and topic tables and the configuration catalog every stem shares;
//! and each stem is a crate under `crates/<stem>/`, every slice a module of
//! it with its tests beside it, so two slices never write one file. The
//! exemplar's guest template is compiled in as the [`omnia_template`]
//! crate and is the adapter's [`seed`], laid in a tree that lacks it before
//! any slice's turn with the lists empty, so a slice appends its lines and
//! copies nothing. A build writes through `write_files` and `edit_files`,
//! runs `cargo clippy`, `cargo test`, and a `wasm32-wasip2` check through
//! the shell, and reports once they pass; a wave is verified by the same
//! checks over the integrated tree.
//!
//! What the adapter holds a tree to beyond the checks, it holds as findings
//! of the turn: a report or a verdict over a tree that breaks a rule is
//! refused with the rule named, the turn repairs the tree and answers again,
//! and nothing is written once a turn ends. The rules are:
//!
//! - the root `Cargo.toml` stands as the template spells it outside its two
//!   stem lists, so a pin, a lint, the `host` feature, or a target table a
//!   turn changed is a finding with the lines named;
//! - at a slice's build, a stem is one word: a crate the turn laid under
//!   the head of a hyphenated stem of the slice, `crates/order/` for
//!   `order-items`, is named, since the stem's crate is `crates/<stem>/`
//!   whole, and a crate the tree held before the turn is an earlier slice's;
//! - at a wave's verify, every static file of the template stands byte for
//!   byte, a file the tree lacks among them;
//! - no crate is laid at `crates/start/`: the seed and the host are the
//!   program's start, a deployment setting sits in `crates/common/src/config.rs`,
//!   a cross-cutting rule in `crates/common/`, and a stated health route in its
//!   own stem's crate, so any file under `crates/start/` is named and removed;
//! - no module under `crates/*/src` is the process the source was: a
//!   function that starts, stops, runs, listens, serves, bootstraps, or
//!   shuts it down, or a `Main`, `App`, `Server`, or `Application` type, is
//!   named with its line, and the module is removed rather than renamed;
//! - no read of the store under a constant key answers empty on a miss: a
//!   table read under one key is fetched from its upstream and stored when
//!   the store holds none, so a read whose function returns an empty
//!   collection, defaults the miss, or answers the `None` arm as nothing is
//!   named at the answering line, while a read under a key built from the
//!   input is a lookup and passes;
//! - no `Deserialize` struct under `crates/*/src` wraps an HTTP response in a
//!   single `data` field: the wire carries the response body at its root,
//!   never under the source's HTTP client property (`response.data`), so the
//!   wrapper is named at its line and removed;
//! - no module under `crates/*/src` simulates the host: one that silences
//!   the workspace's lints for its crate, spawns a task, a timer, or a
//!   thread, reaches the environment, the file system, a socket, or a
//!   process, loops to poll or retry, or keeps a flag machine — a step-done
//!   key, a stop request, an exit delay — is named with its line, since
//!   each is a mechanism the host owns and the translation lays nothing
//!   for, and the spelling is removed from the module;
//! - every constant of the two tables is spelled into its lists: a route's
//!   by a statement of the route list and an entry of the route rung, a
//!   consumed topic's by a statement of the topic list and an entry of the
//!   messaging rung, and a topic a module alone publishes by that module
//!   and neither list, so an outbound topic is no finding;
//! - every handler — a `pub async fn` under `crates/*/src` taking a
//!   `Context` — is handed to its route function or consumer by a statement
//!   of the route list or the topic list, or to `handle_with` by a route
//!   function of its stem a statement calls, since one no list reaches is
//!   code the host never runs;
//! - every setting of the configuration catalog beyond the template's own
//!   is read by a module outside `common` or a root source, since a key
//!   nothing reads is a value the deployment supplies for nothing.
//!
//! Before a slice's turn, the root `.gitignore` is put back as the template
//! spells it: the repository the engine lays holds an ignore file of its
//! own, which the seed never writes over, and the turn's build output would
//! otherwise be sealed with the slice.
//!
//! The worker a turn's checks run in reaches the Rust toolchain beneath the
//! operator's home, `~/.rustup` and `~/.cargo/bin`, with `RUSTUP_HOME` set
//! to the former, and the crates.io index and its downloads through the
//! backend's proxy; nothing else beyond the lent tree.
//!
//! The adapter builds into a fresh tree: the template and the slices are
//! the whole of what it lays, so it declares a [`Base::Fresh`] and the build
//! refuses a base whose root holds a directory rather than let the template
//! and the verify rewrite a project it was pointed at.
//!
//! # Merge rules
//!
//! - `Cargo.lock` keeps the integrated side; the next check regenerates it.
//! - The root `Cargo.toml`, the two router lists, the two root test tables,
//!   `clippy.toml`, the tables and the catalog under `crates/common/`, every
//!   `crates/<stem>/Cargo.toml`, and every `crates/<stem>/src/lib.rs` keep
//!   both sides' lines: each holds a list slices add a line to.
//! - Every other path is unruled. `src/lib.rs` and the rest of the template
//!   no slice edits, so two slices changing one is a conflict and never a
//!   merge; a module and its tests are one slice's alone, so a conflict there
//!   is a slice writing outside its part, rebuilt over the merged tree.
//!
//! # Errors
//!
//! A directory or a symbolic link standing where a file of the template
//! goes is `bad_request`; a file that cannot be read or written is
//! `server_error`.

mod tree;

use std::borrow::Cow;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use emery_sdk::target::{
    Base, Context, MergeRule, MergeStrategy, Report, Seed, TargetAdapter, Verdict, VerifyContext,
    Worker,
};
use emery_sdk::{Error, Model, bad_request, server_error, tracing};
use omnia_template::ENTRIES;

use self::tree::{Stems, Tree};

// The root ignore file, which the engine lays a one-line copy of at a laid
// repository's root commit.
const IGNORE: &str = ".gitignore";

/// The adapter implementation.
pub struct Adapter;

impl TargetAdapter for Adapter {
    const BASE: Base = Base::Fresh;
    const MERGE_RULES: &'static [MergeRule] = &[
        MergeRule {
            paths: Cow::Borrowed("Cargo.lock"),
            strategy: MergeStrategy::Ours,
        },
        MergeRule {
            paths: Cow::Borrowed("Cargo.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("clippy.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("src/routes.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("src/messaging.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("tests/routes.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("tests/messaging.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/common/src/routes/*.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/common/src/config.rs"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/*/Cargo.toml"),
            strategy: MergeStrategy::Union,
        },
        MergeRule {
            paths: Cow::Borrowed("crates/*/src/lib.rs"),
            strategy: MergeStrategy::Union,
        },
    ];
    const WORKER: Worker = Worker {
        egress: &["index.crates.io", "static.crates.io"],
        read: &["~/.rustup", "~/.cargo/bin"],
        env: &[("RUSTUP_HOME", "~/.rustup")],
    };

    async fn build<P: Model>(ctx: &Context<'_, P>) -> Result<Report, Error> {
        let root = Path::new(ctx.workspace);

        // the seed never writes over a held file, so the engine's one-line
        // ignore file would let the turn's `target/` into the slice's commit
        let ignored = restore_ignore(root)?;
        if ignored {
            tracing::debug!(slice = %ctx.slice.id, "the root ignore file put back as the template spells it");
        }

        // the crates the tree holds before the turn are earlier slices'
        let stems = Stems::read(&ctx.slice.spec, root)?;
        let mut report = emery_sdk::target::build(ctx, PROSE, &seed(), |_| {
            let tree = Tree::new(root);
            let mut findings = tree.stems(&stems)?;
            findings.extend(tree.held()?);
            Ok(findings)
        })
        .await?;
        if ignored {
            written(&mut report, IGNORE);
        }

        Ok(report)
    }

    async fn verify<P: Model>(ctx: &VerifyContext<'_, P>) -> Result<Verdict, Error> {
        let root = Path::new(ctx.workspace);
        emery_sdk::target::verify(ctx, PROSE, |_| {
            let tree = Tree::new(root);
            let mut findings = tree.statics()?;
            findings.extend(tree.held()?);
            Ok(findings)
        })
        .await
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

/// Returns the files every tree of the layout starts from, laid where a working copy lacks them.
///
/// Every entry of the exemplar's guest template, [`omnia_template::ENTRIES`],
/// as a [`Seed`] at its path with its body: the root manifest and its pins,
/// the component's entry, the toolchain, the formatter, lint, deny, and vet
/// configuration, the task runner, and the workflows, with the two router
/// lists, the two root test tables, the local host, and the `common` crate
/// beside them. A slice finds them in place and copies none.
#[must_use]
pub fn seed() -> Vec<Seed> {
    ENTRIES.iter().map(|entry| Seed::new(entry.path, entry.body)).collect()
}

// Puts the template's ignore file over the one the tree holds and answers
// whether it did; a tree holding none is left for the seed to lay.
fn restore_ignore(root: &Path) -> Result<bool, Error> {
    let entry = omnia_template::entry(IGNORE)
        .ok_or_else(|| server_error!("the template lays no `{IGNORE}`"))?;
    let path = root.join(entry.path);
    let held = match fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() => {
            fs::read(&path).map_err(|err| server_error!("reading `{IGNORE}`: {err}"))?
        }
        Ok(_) => return Err(bad_request!("a directory or symbolic link stands at `{IGNORE}`")),
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(server_error!("reading `{IGNORE}`: {err}")),
    };
    if held == entry.body.as_bytes() {
        return Ok(false);
    }
    fs::write(&path, entry.body).map_err(|err| server_error!("writing `{IGNORE}`: {err}"))?;
    Ok(true)
}

// Names `path` in the report once, the list kept sorted.
fn written(report: &mut Report, path: &str) {
    if !report.written.iter().any(|named| named == path) {
        report.written.push(path.to_owned());
        report.written.sort();
    }
}

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
