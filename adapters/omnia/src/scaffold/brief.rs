//! Prepares what a build turn reads of the scaffold: the brief — each stem's
//! files and lines, the design types laid, the markers to fill, the check —
//! and, apart from it, the reference passages the slice's shapes need, with
//! the guarded files and the laid design types beside them.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use emery_sdk::target::{self, Guarded};
use emery_sdk::{Doc, INLINE_BYTES};

use super::lists::{Line, Placement};
use super::names::escape;
use super::reference::{self, Laid, Needs};
use super::stem::{Handler, Kind, Stem};
use super::{Scaffold, render, trigger};
use crate::layout::{self, TOPICS, crate_dir};

const CHECK: &str = "cargo check --target wasm32-wasip2 && cargo +nightly fmt --all -- --check";

const HELD: &str =
    "The tree already held it, so nothing was written over it; read it for what stands.";

// How much of the turn the inlined texts may take: the brief's files first,
// the passages after them, a text past the budget named by its path alone.
struct Budget {
    left: u64,
}

// What the turn reads of the scaffold, owned so the SDK's `Scaffold` borrows
// it: the brief and the passages under one budget, the guarded files, and
// the design types laid.
pub struct Prepared {
    brief: String,
    passages: String,
    files: Vec<Guarded>,
    types: Vec<String>,
}

impl Prepared {
    pub fn as_target(&self) -> target::Scaffold<'_> {
        target::Scaffold {
            brief: &self.brief,
            passages: &self.passages,
            files: &self.files,
            types: &self.types,
        }
    }
}

impl Scaffold {
    // The brief and, apart from it, the passages the slice's shapes select
    // from the references, so the SDK renders the brief as the slice's own
    // text and the passages after it as no text of the slice; the guarded
    // files and the laid design types beside them.
    pub fn prepare(&self, docs: &[Doc]) -> Prepared {
        let mut budget = Budget { left: INLINE_BYTES };
        let brief = self.brief(&mut budget);
        let passages = self.passages(docs, &mut budget);
        Prepared {
            brief,
            passages,
            files: self.guarded(),
            types: self.types(),
        }
    }

    fn brief(&self, budget: &mut Budget) -> String {
        let mut text = String::new();
        for stem in &self.plan.stems {
            stem.brief_files(budget, &mut text);
            stem.brief_markers(&mut text);
        }
        if !self.plan.start.is_empty() {
            let _ = write!(
                text,
                "The `start` stem ({}): the seed and the host are `start`, so nothing was \
                 scaffolded for it; cover what they satisfy and leave the rest out, as the \
                 guardrails' table says.",
                self.plan.start.join(", ")
            );
            text.push_str("\n\n");
        }
        for relaid in &self.plan.relaid {
            let _ = writeln!(
                text,
                "{} (`{}`), stated under `start`, is laid under `crates/{}/`, the stem its route's \
                 resource names, since a stated route is its own stem's and never `start`'s.\n",
                relaid.id,
                relaid.route,
                crate_dir(&relaid.stem)
            );
        }
        let _ = writeln!(
            text,
            "The check: once your edits are in, run `{}` in `$WORKSPACE` and read what it \
             prints; repair what it reports through `edit_files` and run it again until it \
             passes. Where its last step alone fails because no nightly toolchain is installed, \
             the steps before it passed and the check is done.\n",
            self.check()
        );
        text.trim_end().to_owned()
    }

    // The one shell command the turn checks the tree with: the slice's
    // packages under clippy and their tests, then the guest check and the
    // formatter over the workspace.
    fn check(&self) -> String {
        let packages: Vec<String> =
            self.plan.stems.iter().map(|stem| format!("-p {}", stem.package)).collect();
        let packages = packages.join(" ");
        format!(
            "cargo clippy {packages} --all-targets -- -D warnings && cargo test {packages} && \
             {CHECK}"
        )
    }

    // The passages the laid shapes, the roles, and the idioms select, each
    // quoted whole under its source and heading while the budget holds;
    // empty where none is selected or none fits.
    fn passages(&self, docs: &[Doc], budget: &mut Budget) -> String {
        let mut laid = BTreeSet::new();
        for stem in &self.plan.stems {
            if !stem.handlers.is_empty() {
                laid.insert(Laid::Routes);
            }
            if stem.handlers.iter().any(|handler| handler.route_fn.is_some()) {
                laid.insert(Laid::Encoded);
            }
            if !stem.consumers.is_empty() {
                laid.insert(Laid::Consumers);
            }
            if stem.tests.is_some() {
                laid.insert(Laid::Tests);
            }
            if !stem.types.is_empty() {
                laid.insert(Laid::Types);
            }
        }
        let needs = Needs {
            laid,
            roles: self.plan.roles.clone(),
            idioms: self.plan.idioms.clone(),
        };
        let mut quoted = String::new();
        for passage in reference::passages(docs, &needs) {
            let body = if passage.code {
                render::fenced("rust", &passage.text)
            } else {
                format!("{}\n", passage.text)
            };
            let entry =
                format!("From `{}`, under \"{}\":\n\n{body}\n", passage.source, passage.heading);
            if !budget.fits(&entry) {
                break;
            }
            quoted.push_str(&entry);
        }
        if quoted.is_empty() {
            return String::new();
        }
        format!(
            "What the references spell for these shapes, quoted; `read_doc` answers the rest of \
             each document:\n\n{}",
            quoted.trim_end()
        )
    }
}

impl Stem {
    fn brief_files(&self, budget: &mut Budget, text: &mut String) {
        let _ = writeln!(text, "Stem `{}`, part `{}`:\n", self.name, self.part);
        let _ = writeln!(
            text,
            "- `{}`: {}{}. One `<crate>.workspace = true` line at the end of `[dependencies]` \
             for each crate your code uses beyond the {} it holds; a crate the root manifest \
             does not pin carries its version instead.",
            self.manifest.path,
            self.manifest.origin(),
            self.brief_sql_dependency(),
            if self.sql { "seven" } else { "six" }
        );
        let mut entry_lines = vec![format!("`pub mod {};`", self.part)];
        if !self.handlers.is_empty() {
            entry_lines.push(format!("`{}::routes(),`", self.part));
        }
        if !self.consumers.is_empty() {
            entry_lines.push(format!("`{}::topics,`", self.part));
        }
        let _ = writeln!(
            text,
            "- `{}`: {}, {} {} its lists.",
            self.entry.path,
            self.entry.origin(),
            listed(&entry_lines),
            if self.entry.held { "added to" } else { "in" }
        );
        let _ = write!(
            text,
            "- `{}`: {}. {}",
            self.module.path,
            self.module.origin(),
            if self.module.held {
                HELD
            } else {
                "Fill its markers, listed below, through `edit_files`."
            }
        );
        self.brief_text(Kind::Module, budget, text);
        match &self.tests {
            Some(tests) if tests.held => {
                let _ = write!(text, "- `{}`: held. {HELD}", tests.path);
                self.brief_text(Kind::Tests, budget, text);
            }
            Some(tests) => {
                let _ = write!(
                    text,
                    "- `{}`: scaffolded, the prelude and one ignored test per scenario, each \
                     sending the request or the delivery its requirement carries. Fill each \
                     test through `edit_files`, replacing the block listed below.",
                    tests.path
                );
                self.brief_text(Kind::Tests, budget, text);
            }
            None => {
                text.push_str("- No scenario states a test, so no test file was scaffolded.\n");
            }
        }
        self.brief_lines(text);
    }

    // The clause after the manifest's origin naming the SQL crate laid among
    // its dependencies and what for, or nothing for a slice that neither
    // probes nor reaches a SQL store.
    fn brief_sql_dependency(&self) -> String {
        if !self.sql {
            return String::new();
        }
        let line = render::dependency_line(layout::PROBE_DEPENDENCY);
        let how = match self.lines.iter().find(|held| held.path == self.manifest.path) {
            None => "among",
            Some(laid) if laid.placement == Placement::Held => "already held among",
            Some(_) => "added at the end of",
        };
        let what = if self.probing {
            "the `Row` the probe's tests arrange the store's answer with"
        } else {
            "the SQL store the slice reaches"
        };
        format!(", `{line}` {how} its `[dependencies]` for {what}")
    }

    // The stem's lines in the root lists and the topic table, each spelled,
    // and the ones no anchor took.
    fn brief_lines(&self, text: &mut String) {
        let root: Vec<&Line> =
            self.lines.iter().filter(|line| !line.path.starts_with("crates/")).collect();
        let added = root.iter().any(|line| line.placement == Placement::Add);
        let spelled: Vec<String> = root
            .iter()
            .filter(|line| line.path != TOPICS)
            .map(|line| format!("`{}` in `{}`", line.text.trim(), line.path))
            .collect();
        let _ = writeln!(
            text,
            "- `Cargo.toml`, `{}`, `{}`: the stem's lines {}: {}.",
            layout::ROUTES,
            layout::MESSAGING,
            if added { "added" } else { "already held" },
            spelled.join(", ")
        );
        for consumer in &self.consumers {
            let _ = writeln!(
                text,
                "- `{TOPICS}`: the constant `{}` for the topic `{}` {}: `pub const {}: &str = \
                 \"{}\";`.",
                consumer.constant,
                consumer.topic,
                if consumer.declared {
                    "already declared"
                } else {
                    "added at the end of the table"
                },
                consumer.constant,
                escape(&consumer.topic)
            );
        }
        for line in self.lines.iter().filter(|line| line.placement == Placement::Unanchored) {
            let _ = writeln!(
                text,
                "- `{}` holds no anchor for `{}`, so the line was not added: add it through \
                 `edit_files` where the layout's table says.",
                line.path,
                line.text.trim()
            );
        }
        text.push('\n');
    }

    // The file's text under the budget, fenced after its line; its path
    // alone past it.
    fn brief_text(&self, kind: Kind, budget: &mut Budget, text: &mut String) {
        let Some(held) = self.text(kind) else {
            text.push('\n');
            return;
        };
        let fenced = render::fenced("rust", &held);
        if budget.fits(&fenced) {
            let _ = write!(text, " Its text:\n\n{fenced}\n");
        } else {
            text.push_str(" Read it in the tree.\n");
        }
    }

    // The design types laid from their signatures, finished as they stand,
    // and the fields among them that carry another type as a value.
    fn brief_types(&self, text: &mut String) {
        let laid: Vec<String> = self
            .types
            .iter()
            .filter(|design_type| !design_type.body.is_marker())
            .map(|design_type| format!("`{}` (`{}`)", design_type.name, design_type.key))
            .collect();
        if laid.is_empty() {
            return;
        }
        let _ = writeln!(
            text,
            "The design types {} in `{}` are scaffolded as their signatures spell, each field \
             and variant documented and the wire's names kept through `serde(rename)`; they are \
             finished, so change a field only where a scenario's wire shape or the check \
             requires it.",
            listed(&laid),
            self.module.path
        );
        let carried: Vec<String> = self
            .types
            .iter()
            .flat_map(|design_type| {
                design_type.body.carried().map(move |(field, stands_for)| {
                    format!("`{}.{field}` (`{stands_for}`)", design_type.name)
                })
            })
            .collect();
        match carried.as_slice() {
            [] => {}
            [one] => {
                let _ = writeln!(
                    text,
                    "The field {one} is `serde_json::Value` because the type it stands for is \
                     declared by another slice or by no design type: leave it as laid, and read \
                     the value's fields where the behaviour needs them."
                );
            }
            several => {
                let _ = writeln!(
                    text,
                    "The fields {} are `serde_json::Value` because the type each stands for is \
                     declared by another slice or by no design type: leave them as laid, and read \
                     the value's fields where the behaviour needs them.",
                    listed(several)
                );
            }
        }
        text.push('\n');
    }

    fn brief_markers(&self, text: &mut String) {
        self.brief_types(text);
        if self.module.held || self.tests.as_ref().is_some_and(|tests| tests.held) {
            text.push_str(
                "The markers below are the ones a fresh scaffold writes; a held file holds \
                 whatever stands in it.\n\n",
            );
        }
        text.push_str(
            "Markers, in fill order, each one `edit_files` edit whose `find` is the key quoted \
             beside it, alone, and whose `replace` is the whole text that takes the quoted \
             block's place, its lines indented as the block's are:\n\n",
        );
        let mut count = 0;
        for design_type in self.types.iter().filter(|design_type| design_type.body.is_marker()) {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `pub {} {{}}` in `{}`: the design type `{}`, whose signature the scaffold \
                 could not lay, its key as quoted. Replace it with the {} the signature in its \
                 doc spells, every field documented; drop `Eq` from its derives where a field is \
                 a float.\n",
                design_type.declaration(),
                self.module.path,
                design_type.key,
                if design_type.body.is_enum() { "enum" } else { "struct" }
            );
        }
        for handler in &self.handlers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. The handler `{}` of `{}` in `{}`, key `{}`: fill `{}` and `{}` with the \
                 wire's fields, then replace this block with the body. {}\n\n{}",
                handler.name,
                handler.route,
                self.module.path,
                handler.marker,
                handler.input,
                handler.output,
                handler.status_hint(),
                render::fenced("rust", &render::stub(&handler.marker))
            );
        }
        for consumer in &self.consumers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. The consumer `{}` of the topic `{}` in `{}`, key `{}`, registered in the \
                 part's `topics(..)` under `topic::{}`: fill `{}` with the delivery's fields, \
                 then replace this block with the body.\n\n{}",
                consumer.name,
                consumer.topic,
                self.module.path,
                consumer.marker,
                consumer.constant,
                consumer.input,
                render::fenced("rust", &render::stub(&consumer.marker))
            );
        }
        for function in &self.functions {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. The function `{}` in `{}`, key `{}`, since the requirement states no \
                 route and no message: replace this block with the function whole, the \
                 signature the behaviour needs over its body.\n\n{}",
                function.name,
                self.module.path,
                function.marker,
                render::fenced("rust", &function.block())
            );
        }
        if let Some(tests) = &self.tests {
            for test in &self.scenarios {
                count += 1;
                let _ = writeln!(
                    text,
                    "{count}. The test `{}` in `{}`, key `{}`: replace this block with the filled \
                     test, its `#[ignore = ..]` line gone: the arrangement over `provider()`, \
                     the act as laid, and the assertions the scenario states.\n\n{}",
                    test.name,
                    tests.path,
                    test.marker,
                    render::fenced("rust", &test.block(self))
                );
            }
        }
        if count == 0 {
            text.push_str("None: the slice states no requirement under this stem.\n\n");
        }
    }
}

impl Handler {
    // How the handler's `Ok` reaches the wire, and where another status is
    // set.
    fn status_hint(&self) -> String {
        self.route_fn.as_ref().map_or_else(
            || {
                format!(
                    "Its `Ok` is answered `200` as JSON; for another status, declare a route \
                     function over `handle_with(MethodFilter::{}, {}, decode, |output: {}| \
                     (StatusCode::NO_CONTENT, Json(output)).into_response())` as the handlers \
                     reference shows and name it in `routes()` in place of `{}({})`.",
                    self.route.method,
                    self.name,
                    self.output,
                    trigger::method_fn(&self.route.method),
                    self.name
                )
            },
            |route_fn| {
                format!(
                    "Its `Ok` is answered as JSON under the status the encoder in `{route_fn}` \
                     names, `StatusCode::OK`, which is this slice's to decide there: a `201` \
                     for a creation, or one read off the output, `503` where a probe found its \
                     store down; an error status, a `404`, is the error the errors passage \
                     maps to it."
                )
            },
        )
    }
}

impl Budget {
    // Whether `text` fits in what is left, taking it where it does.
    const fn fits(&mut self, text: &str) -> bool {
        let size = text.len() as u64;
        if size > self.left {
            return false;
        }
        self.left -= size;
        true
    }
}

// `a`, `a and b`, or `a, b, and c`.
fn listed(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}
