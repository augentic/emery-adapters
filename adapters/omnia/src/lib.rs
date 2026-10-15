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
//! - at a wave's verify, every static file of the template stands byte for
//!   byte, a file the tree lacks among them;
//! - no module under `crates/start/src` is the process the source was: a
//!   function that starts, stops, runs, listens, serves, bootstraps, or
//!   shuts it down, or a `Main`, `App`, `Server`, or `Application` type, is
//!   named with its line, and the module is removed rather than renamed;
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
//!   code the host never runs.
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

use std::borrow::Cow;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use emery_sdk::target::{
    Base, Context, MergeRule, MergeStrategy, Report, Seed, TargetAdapter, Verdict, VerifyContext,
    Worker,
};
use emery_sdk::{Error, Model, bad_request, server_error, tracing};
use omnia_template::{ENTRIES, Manifest};

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

        let mut report =
            emery_sdk::target::build(ctx, PROSE, &seed(), |_| Tree::new(root).held()).await?;
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

// The lent tree, read for what a turn must leave standing. Each rule answers
// the findings the turn repairs, one per rule with every line named, so a
// report or a verdict is refused once with everything the tree breaks.
struct Tree<'a> {
    root: &'a Path,
}

impl<'a> Tree<'a> {
    const fn new(root: &'a Path) -> Self {
        Self { root }
    }

    // The rules a build and a verify share: the root manifest outside its
    // lists, no simulated host, the tables spelled into their lists, every
    // handler listed.
    fn held(&self) -> Result<Vec<String>, Error> {
        let mut findings = Vec::new();
        findings.extend(self.manifest()?);
        findings.extend(self.simulated()?);
        findings.extend(self.tables()?);
        findings.extend(self.unlisted()?);
        Ok(findings)
    }

    // Every static file of the template that differs: the lines where an
    // edit puts them back, the template's body whole for a file the tree
    // lacks or rewrote past `LISTED` lines, since the lines alone cannot lay
    // it.
    fn statics(&self) -> Result<Vec<String>, Error> {
        let drift = omnia_template::drift(self.root).map_err(|err| refusal(&err))?;
        let mut findings = Vec::new();
        for file in drift.files() {
            let finding = if self.lacks(file.path) {
                format!(
                    "`{}` is a static file of the template the tree lacks; lay it whole through \
                     `write_files`, as the template spells it:\n{}",
                    file.path,
                    template(file.path)?
                )
            } else if file.lines.len() > LISTED {
                format!(
                    "`{}` is a static file of the template and differs from it in {} lines; lay it \
                     whole through `write_files`, as the template spells it:\n{}",
                    file.path,
                    file.lines.len(),
                    template(file.path)?
                )
            } else {
                format!(
                    "`{}` is a static file of the template and differs from how the template \
                     spells it; put the `-` lines back and drop the `+` lines through \
                     `edit_files`:\n{}",
                    file.path,
                    file.lines.join("\n")
                )
            };
            findings.push(finding);
        }
        Ok(findings)
    }

    // The root manifest outside its two stem lists, the template's whole for
    // a tree that lacks it.
    fn manifest(&self) -> Result<Vec<String>, Error> {
        let drift = Manifest::new(self.root).drift().map_err(|err| refusal(&err))?;
        if drift.is_empty() {
            return Ok(Vec::new());
        }
        let finding = if self.lacks(Manifest::PATH) {
            format!(
                "the tree lacks the root `{}`; lay it whole through `write_files` as the template \
                 spells it, then add each stem's line to its two lists:\n{}",
                Manifest::PATH,
                template(Manifest::PATH)?
            )
        } else {
            format!(
                "the root `{}` changed outside its two stem lists; the pins, the lints, the `host` \
                 feature, and the target tables stand as the template spells them, so put the `-` \
                 lines back and drop the `+` lines through `edit_files`:\n{drift}",
                Manifest::PATH
            )
        };
        Ok(vec![finding])
    }

    fn lacks(&self, path: &str) -> bool {
        matches!(
            fs::symlink_metadata(self.root.join(path)),
            Err(err) if err.kind() == ErrorKind::NotFound
        )
    }

    // Two findings at most, each naming `<path>:<line>: <what>` for every
    // line under a stem's `src/`, sorted: the words of `LIFECYCLE` under
    // `crates/start/src`, whose repair removes the module, and, in the
    // modules that remain, the spellings of `SIMULATED` under any stem,
    // whose repair removes the spelling. A tree with no `crates/` answers
    // none.
    fn simulated(&self) -> Result<Vec<String>, Error> {
        let mut lifecycle = Vec::new();
        let mut simulated = Vec::new();
        for file in self.modules()? {
            let text = code(&read(&file)?);
            let path = self.relative(&file);
            let mut process = Vec::new();
            if path.starts_with(START) {
                for (number, line) in text.lines().enumerate() {
                    for (word, what) in LIFECYCLE {
                        if spells(line, word) {
                            process.push(format!("{path}:{}: {what} (`{word}`)", number + 1));
                        }
                    }
                }
            }
            if !process.is_empty() {
                lifecycle.extend(process);
                continue;
            }
            for (number, line) in text.lines().enumerate() {
                for (needle, what) in SIMULATED {
                    let found = if *needle == "loop {" {
                        spells(line, needle)
                    } else {
                        line.contains(needle)
                    };
                    if found {
                        simulated.push(format!("{path}:{}: {what} (`{needle}`)", number + 1));
                    }
                }
            }
        }
        let mut findings = Vec::new();
        if !lifecycle.is_empty() {
            lifecycle.sort();
            findings.push(format!(
                "a module under `{START}` is the process the source was, which the host is and the \
                 tree never simulates; remove each module named, with its `pub mod` line and its \
                 test, through `delete` rather than rename it, and leave what it stood for to the \
                 report:\n{}",
                lifecycle.join("\n")
            ));
        }
        if !simulated.is_empty() {
            simulated.sort();
            findings.push(format!(
                "a module simulates the host, which the translation lays nothing for; remove each \
                 spelling from its module through `edit_files`, never by renaming it: a loop's \
                 actions run once, back to back, a failure carried as the classified error; a \
                 flag's step is read at the moment of use; a task's, a timer's, or a reach past \
                 the host's work is laid as the translation says or left out:\n{}",
                simulated.join("\n")
            ));
        }
        Ok(findings)
    }

    // One finding naming every constant of the two tables its lists
    // disagree with, and every list statement naming no constant: a route's
    // constant stands in the route list and the route rung; a consumed
    // topic's in the topic list and the messaging rung; a topic a module
    // alone names is published and stands in neither.
    fn tables(&self) -> Result<Vec<String>, Error> {
        let mut lines = Vec::new();

        // the path table: every constant in the route list and the route rung
        let routes = code(&self.text(HTTP.list)?);
        let rung = code(&self.text(HTTP.rung)?);
        for (number, name) in self.constants(HTTP.path)? {
            let qualified = format!("{}{name}", HTTP.prefix);
            if !spells(&routes, &qualified) {
                lines.push(format!(
                    "{}:{number}: route `{name}` is named by no statement of `{}`",
                    HTTP.path, HTTP.list
                ));
            }
            if !spells(&rung, &qualified) {
                lines.push(format!(
                    "{}:{number}: route `{name}` is named by no entry of `{}` in `{}`",
                    HTTP.path, HTTP.entries, HTTP.rung
                ));
            }
        }

        // the topic table: a consumed constant in the topic list and the
        // messaging rung, a published one in a stem's module and neither
        let topics = code(&self.text(TOPIC.list)?);
        let rung = code(&self.text(TOPIC.rung)?);
        let constants = self.constants(TOPIC.path)?;
        let mut modules = Vec::new();
        if !constants.is_empty() {
            for file in self.modules()? {
                if !self.relative(&file).starts_with(COMMON) {
                    modules.push(code(&read(&file)?));
                }
            }
        }
        for (number, name) in constants {
            let qualified = format!("{}{name}", TOPIC.prefix);
            let finding = match (spells(&topics, &qualified), spells(&rung, &qualified)) {
                (true, true) => continue,
                (true, false) => format!(
                    "topic `{name}` is consumed by `{}` and named by no entry of `{}` in `{}`",
                    TOPIC.list, TOPIC.entries, TOPIC.rung
                ),
                (false, true) => format!(
                    "topic `{name}` is named by an entry of `{}` in `{}` and consumed by no \
                     statement of `{}`",
                    TOPIC.entries, TOPIC.rung, TOPIC.list
                ),
                (false, false) if modules.iter().any(|text| spells(text, &name)) => continue,
                (false, false) => format!(
                    "topic `{name}` is consumed by no statement of `{}` and published by no module",
                    TOPIC.list
                ),
            };
            lines.push(format!("{}:{number}: {finding}", TOPIC.path));
        }

        // every statement of a list over a constant of its table
        for (table, text) in [(&HTTP, &routes), (&TOPIC, &topics)] {
            for (at, statement) in calls(text, ".route(") {
                if !statement.contains(table.prefix) {
                    let number = text[..at].matches('\n').count() + 1;
                    lines.push(format!(
                        "{}:{number}: a statement names no constant of `{}`",
                        table.list, table.path
                    ));
                }
            }
        }

        if lines.is_empty() {
            return Ok(Vec::new());
        }
        lines.sort();
        Ok(vec![format!(
            "a table and its lists disagree: a route's constant is named by one statement of \
             `src/routes.rs` and one entry of `ROUTES` in `tests/routes.rs`, a consumed topic's \
             by one statement of `src/messaging.rs` and one entry of `TOPICS` in \
             `tests/messaging.rs`, and a topic only published by the module that publishes it and \
             neither list; add the missing line at the end of its list through `edit_files`, \
             remove an entry nothing consumes, and rewrite no other line:\n{}",
            lines.join("\n")
        )])
    }

    // The text at `path`, empty where the tree holds no file there.
    fn text(&self, path: &str) -> Result<String, Error> {
        let file = self.root.join(path);
        if file.is_file() { read(&file) } else { Ok(String::new()) }
    }

    // Every `pub const` the table at `path` declares, each its 1-based line
    // and name; an absent table declares none.
    fn constants(&self, path: &str) -> Result<Vec<(usize, String)>, Error> {
        let text = self.text(path)?;
        let constants = text
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let rest = line.trim_start().strip_prefix("pub const ")?;
                let name = rest.split(|ch: char| !identifier(ch)).next()?;
                (!name.is_empty()).then(|| (index + 1, name.to_owned()))
            })
            .collect();
        Ok(constants)
    }

    // One finding naming every handler under a stem's `src/` — a `pub async
    // fn` whose signature takes a `Context` — that no `.route(..)` statement
    // of the two lists reaches: handed to the route function or consumer the
    // statement calls, or to `handle_with` by a route function of a stem the
    // statement calls with no argument.
    fn unlisted(&self) -> Result<Vec<String>, Error> {
        let mut lists = Vec::new();
        for list in LISTS {
            let path = self.root.join(list);
            if path.is_file() {
                lists.push(code(&read(&path)?));
            }
        }
        let mut modules = Vec::new();
        for file in self.modules()? {
            let path = self.relative(&file);
            if let Some(module) = module_path(&path) {
                modules.push((path, module, code(&read(&file)?)));
            }
        }

        // what the lists hand, then what a route function they call hands
        let mut reached: Vec<String> = lists
            .iter()
            .flat_map(|text| calls(text, ".route("))
            .filter_map(|(_, statement)| handed(statement).map(str::to_owned))
            .collect();
        for (_, module, text) in &modules {
            for (route, handler) in constructed(module, text) {
                if reached.contains(&route) && !reached.contains(&handler) {
                    reached.push(handler);
                }
            }
        }

        let mut lines = Vec::new();
        for (path, module, text) in &modules {
            for (number, name) in handlers(text) {
                let full = format!("{module}::{name}");
                if !reached.contains(&full) {
                    lines.push(format!(
                        "{path}:{number}: handler `{name}` is reached by no list statement"
                    ));
                }
            }
        }
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        lines.sort();
        Ok(vec![format!(
            "a handler is reached by no `.route(..)` statement of `src/routes.rs` or \
             `src/messaging.rs`, so the host never runs it; list it under the route or topic its \
             requirement states, or, where it is a helper and no handler, hand it the provider \
             rather than the `Context`:\n{}",
            lines.join("\n")
        )])
    }

    // Every `.rs` file under a stem's `src/`, its `tests/` unread.
    fn modules(&self) -> Result<Vec<PathBuf>, Error> {
        let mut files = Vec::new();
        for stem in entries(&self.root.join("crates"))? {
            files.extend(sources(&stem.join("src"))?);
        }
        Ok(files)
    }

    fn relative(&self, file: &Path) -> String {
        file.strip_prefix(self.root).unwrap_or(file).display().to_string()
    }
}

// What no module of a stem spells, each a mechanism the host owns: the
// workspace's lints silenced for the crate, a task, a timer, a thread, a
// reach past the component to the system, a loop that polls or retries, or
// a flag machine standing in for a lifecycle.
const SIMULATED: &[(&str, &str)] = &[
    ("#![allow(", "the workspace's lints silenced"),
    ("tokio::spawn", "a task of its own"),
    ("tokio::time", "a timer of its own"),
    ("std::thread", "a thread of its own"),
    ("std::env", "the environment read past the host"),
    ("std::fs", "the file system reached past the host"),
    ("std::net", "a socket of its own"),
    ("std::process", "a process of its own"),
    ("loop {", "a loop that polls or retries"),
    ("_STARTED_KEY", "a start-up flag of its own"),
    ("_INITIALIZED_KEY", "an initialisation flag of its own"),
    ("_LOADED_KEY", "a load-done flag of its own"),
    ("STOP_REQUESTED", "a stop request of its own"),
    ("EXIT_DELAY", "an exit delay of its own"),
    ("b\"1\"", "a step-done write of its own"),
];

// The modules of the `start` stem, where the source's process would be laid.
const START: &str = "crates/start/src/";

// The shared crate, whose tables declare a constant and publish to none.
const COMMON: &str = "crates/common/src/";

// What no module under `START` spells, each the process the source was: a
// function that starts, stops, runs, listens, serves, bootstraps, or shuts
// it down, or a type that is it. Each is matched as a word, so
// `fn start_at` and `struct AppError` are none.
const LIFECYCLE: &[(&str, &str)] = &[
    ("fn start", "a start of its own"),
    ("fn stop", "a stop of its own"),
    ("fn run", "a run of its own"),
    ("fn listen", "a listener of its own"),
    ("fn serve", "a server of its own"),
    ("fn bootstrap", "a bootstrap of its own"),
    ("fn shutdown", "a shutdown of its own"),
    ("struct Main", "a lifecycle type of its own"),
    ("struct App", "a lifecycle type of its own"),
    ("struct Server", "a lifecycle type of its own"),
    ("struct Application", "a lifecycle type of its own"),
];

// A table of `common` with the list and the rung its constants are spelled
// into, in full under `prefix`.
struct Table {
    path: &'static str,
    prefix: &'static str,
    list: &'static str,
    rung: &'static str,
    entries: &'static str,
}

const HTTP: Table = Table {
    path: "crates/common/src/routes/http.rs",
    prefix: "routes::http::",
    list: "src/routes.rs",
    rung: "tests/routes.rs",
    entries: "ROUTES",
};

const TOPIC: Table = Table {
    path: "crates/common/src/routes/topic.rs",
    prefix: "routes::topic::",
    list: "src/messaging.rs",
    rung: "tests/messaging.rs",
    entries: "TOPICS",
};

// The two lists whose `.route(..)` statements reach a handler.
const LISTS: &[&str] = &["src/routes.rs", "src/messaging.rs"];

// The most differing lines a finding asks an edit to put back; past it the
// file is laid whole.
const LISTED: usize = 24;

fn read(file: &Path) -> Result<String, Error> {
    fs::read_to_string(file).map_err(|err| server_error!("reading `{}`: {err}", file.display()))
}

// The body the template lays at `path`.
fn template(path: &str) -> Result<&'static str, Error> {
    omnia_template::entry(path)
        .map(|entry| entry.body)
        .ok_or_else(|| server_error!("the template lays no `{path}`"))
}

// Every call of `callee` in `text` — the spelling with its opening
// parenthesis, `.route(` — as the offset it starts at and its argument text,
// parentheses balanced, so a call spread over lines is one.
fn calls<'t>(text: &'t str, callee: &str) -> Vec<(usize, &'t str)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(callee) {
        let start = from + at;
        let open = start + callee.len();
        let mut depth = 1usize;
        let mut end = None;
        for (offset, ch) in text[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };
        found.push((start, &text[open..end]));
        from = end;
    }
    found
}

// The path a `.route(..)` statement hands, read from the call its second
// argument makes: the call's first argument where it has one,
// `get(<handler>)`, `consume(<handler>)`, `consume_with(<handler>,
// <decode>)`; the callee itself where it has none, a route function of a
// stem, `orders::placing::place_order_route()`. A statement in another
// shape hands none.
fn handed(statement: &str) -> Option<&str> {
    let call = arguments(statement).get(1)?.trim();
    let open = call.find('(')?;
    let close = call.rfind(')')?;
    let inner = call.get(open + 1..close)?;
    match arguments(inner).first().map(|argument| argument.trim()) {
        Some("") | None => qualified(&call[..open]),
        Some(handler) => qualified(handler),
    }
}

// Every route function `text` declares in `module` with the handler it
// hands `handle_with` as the call's second argument, each qualified as
// `(<fn>, <handler>)`.
fn constructed(module: &str, text: &str) -> Vec<(String, String)> {
    calls(text, "handle_with(")
        .into_iter()
        .filter_map(|(at, inner)| {
            let route = declared(&text[..at])?;
            let handler = qualify(module, arguments(inner).get(1)?)?;
            Some((format!("{module}::{route}"), handler))
        })
        .collect()
}

// The name of the last `pub fn` declared in `head`, the one a call there
// sits in.
fn declared(head: &str) -> Option<&str> {
    head.lines().rev().find_map(|line| {
        let rest = line.trim_start().strip_prefix("pub fn ")?;
        let name = rest.split(|ch: char| !identifier(ch)).next()?;
        (!name.is_empty()).then_some(name)
    })
}

// `text` split at the commas no parenthesis, bracket, or brace encloses.
fn arguments(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (at, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&text[start..at]);
                start = at + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

// The Rust module path of a `.rs` file under `crates/<stem>/src/`,
// `orders::reading` for `crates/orders/src/reading.rs`.
fn module_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("crates/")?.strip_suffix(".rs")?;
    let (stem, module) = rest.split_once("/src/")?;
    let stem = stem.replace('-', "_");
    let module = module.strip_suffix("/mod").unwrap_or(module);
    if module == "lib" {
        Some(stem)
    } else {
        Some(format!("{stem}::{}", module.replace('/', "::")))
    }
}

// A `::`-separated path of two or more identifiers,
// `orders::placing::place_order`; a text in another shape is none.
fn qualified(path: &str) -> Option<&str> {
    let path = path.trim();
    let mut segments = path.split("::");
    let first = segments.next()?;
    let second = segments.next()?;
    (ident(first) && ident(second) && segments.all(ident)).then_some(path)
}

// `path` qualified under `module` (`orders::placing`): a bare identifier
// sits in `module`, a `crate::` path in its stem, and a qualified path
// stands as it is.
fn qualify(module: &str, path: &str) -> Option<String> {
    let path = path.trim();
    if ident(path) {
        return Some(format!("{module}::{path}"));
    }
    if let Some(rest) = path.strip_prefix("crate::") {
        let stem = module.split("::").next()?;
        let full = format!("{stem}::{rest}");
        return qualified(&full).map(str::to_owned);
    }
    qualified(path).map(str::to_owned)
}

fn ident(name: &str) -> bool {
    !name.is_empty() && name.chars().all(identifier)
}

// `text` with every comment line blanked and its newlines kept, so a
// statement a doc comment spells is none and line numbers hold.
fn code(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| if line.trim_start().starts_with("//") { "\n" } else { line })
        .collect()
}

// Whether `text` spells `word` whole: no identifier character continues it
// on either side, so `fn start` is spelled by `pub fn start<P>(` and not by
// `fn start_at(`, and `routes::http::ORDER` not by `routes::http::ORDERS`.
fn spells(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let mut from = 0;
    while let Some(at) = text[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = text[..start].chars().next_back().is_none_or(|ch| !identifier(ch));
        let after = text[end..].chars().next().is_none_or(|ch| !identifier(ch));
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

fn identifier(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

// Every handler `text` declares, each its 1-based line and name: a `pub
// async fn` whose signature, read to its body, takes a `Context<`.
fn handlers(text: &str) -> Vec<(usize, &str)> {
    let mut found = Vec::new();
    let mut offset = 0;
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        if let Some(rest) = raw.trim_start().strip_prefix("pub async fn ") {
            let name = rest.split(|ch: char| !identifier(ch)).next().unwrap_or_default();
            let head = &text[offset..];
            let signature = head.find('{').map_or(head, |body| &head[..body]);
            if !name.is_empty() && signature.contains("Context<") {
                found.push((index + 1, name));
            }
        }
        offset += raw.len();
    }
    found
}

// The entries of `dir`, sorted; an absent `dir` has none.
fn entries(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(server_error!("listing `{}`: {err}", dir.display())),
    };
    let mut paths = listing
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| server_error!("listing `{}`: {err}", dir.display()))?;
    paths.sort();
    Ok(paths)
}

// Every `.rs` file beneath `dir`, symbolic links left unfollowed.
fn sources(dir: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for path in entries(&dir)? {
            let meta = fs::symlink_metadata(&path)
                .map_err(|err| server_error!("reading `{}`: {err}", path.display()))?;
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    Ok(files)
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

fn refusal(err: &omnia_template::Error) -> Error {
    match err {
        omnia_template::Error::Occupied { .. } => bad_request!("{err}"),
        _ => server_error!("{err}"),
    }
}

#[cfg(target_arch = "wasm32")]
emery_sdk::export_target!(Adapter);
