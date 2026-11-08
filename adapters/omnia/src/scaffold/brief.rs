//! Prepares what a build turn reads of the scaffold: the brief — each stem's
//! files and lines, the design types laid, the markers to fill, the check —
//! and, apart from it, the reference passages the slice's shapes need, with
//! the guarded files and the laid design types beside them.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use emery_sdk::target::{self, Guarded};
use emery_sdk::{Doc, INLINE_BYTES};

use super::gate::{self, Gating};
use super::lists::{Line, Placement};
use super::names::escape;
use super::reference::{self, Gate, Laid, Needs, Wire};
use super::stem::{Consumer, Handler, Kind, Stem};
use super::{Scaffold, registration, render, trigger};
use crate::layout::{ENTRY, STEM_DEPENDENCIES, TOPIC_DEPENDENCY, TOPICS, crate_dir};

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
            self.brief_data(stem, &mut text);
            stem.brief_markers(&mut text, self.plan.wire);
        }
        if let Some(wire) = self.plan.wire {
            let _ = writeln!(
                text,
                "The design says the inbound stream's payload is {wire}: a consumer this slice \
                 registers goes through `.topic_with(topic::<NAME>, <handler>, decode)` over a \
                 `fn decode(delivery: &Delivery) -> std::result::Result<<Input>, DecodeError>` of \
                 the part that reads the payload through the format's `serde` reader{reader}, \
                 since `.topic(..)` decodes JSON and refuses every delivery of another format.\n",
                wire = wire.name(),
                reader = wire.reader()
            );
        }
        self.brief_inbound(&mut text);
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
        self.brief_rules(&mut text);
        self.brief_gate(&mut text);
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

    // Each table's data module the scaffold wrote or found for the stem,
    // and the fn reading its constant, so the rows are read and never
    // typed again.
    fn brief_data(&self, stem: &Stem, text: &mut String) {
        for data in stem.data.iter().filter(|_| stem.module.laid()) {
            let origin = if self.written.contains(&data.path) { "scaffolded" } else { "held" };
            let _ = writeln!(
                text,
                "- `{}`: {origin}, every row of the design type `{}` as the wire spells it, one \
                 JSON constant read into `Vec<_>` by `{}()` in `{}`, which is finished. Read \
                 the rows through it where the behaviour needs them, and never type a row again \
                 in code.\n",
                data.path, data.key, data.reader, stem.module.path
            );
        }
    }

    // The inbound stream the design lists for the part, at the part's last
    // slice, since no later slice of the part registers anything: the stems
    // the bullet holds to a registration and the recipe, and, for a stem
    // whose crate already registers a topic, that the stream is consumed
    // there. A stem whose scaffold lays a consumer registers through it and
    // is told nothing.
    fn brief_inbound(&self, text: &mut String) {
        let Some(inbound) = &self.plan.inbound else { return };
        if !self.plan.last {
            return;
        }
        let mut open = Vec::new();
        let mut consumed = Vec::new();
        for stem in inbound.held(self.plan.stems.iter().map(|stem| stem.name.as_str())) {
            if self.consumes(stem) {
                continue;
            }
            if self.plan.consumed.contains(stem) {
                consumed.push(stem);
            } else {
                open.push(stem);
            }
        }
        if !open.is_empty() {
            let crates: Vec<String> =
                open.iter().map(|stem| format!("`crates/{}/`", crate_dir(stem))).collect();
            let _ = writeln!(
                text,
                "The design lists an inbound stream for this part (\"{lead}\"), and this slice \
                 is the part's last, so the stream is consumed by a registration in {crates} now \
                 or never: the host runs a stem through its `handlers()` alone, and a stem \
                 registering no topic is never run. The trigger is the stream's deliveries \
                 setting off the requirements the bullet cites, never a route in its place, a \
                 `start` crate, or a requirement of the registration's own; {registration}, the \
                 constant spelling the topic the bullet names for the stream.\n",
                lead = inbound.lead,
                crates = crates.join(" or "),
                registration = registration(self.plan.wire)
            );
        }
        for stem in consumed {
            let _ = writeln!(
                text,
                "The inbound stream the design lists for this part (\"{lead}\") is consumed \
                 already by a registration in `crates/{dir}/src/`: register no second one, and \
                 reach its deliveries through the consumer that stands.\n",
                lead = inbound.lead,
                dir = crate_dir(stem)
            );
        }
    }

    // The rules every route obeys, where the slice lays a handler and the
    // design or the part's plan entry speaks of them: that they are this
    // part's since no `start` crate applies them, the design's paragraphs on
    // them, and the functions other stems export for them.
    fn brief_rules(&self, text: &mut String) {
        let lays_handlers = self.plan.stems.iter().any(|stem| !stem.handlers.is_empty());
        if !lays_handlers || self.plan.rules.is_empty() {
            return;
        }
        text.push_str(
            "The rules every route obeys are this part's to apply: there is no `start` crate, so \
             a gate on the request or a mapping of failures to wire codes that the plan or the \
             design says `start` applies to this part's routes is written here. A gate is one \
             `pub fn` of the first stem that needs it, called at the top of each handler it \
             guards; a later stem depends on that crate (`<stem> = { path = \"../<stem>\" }` under \
             its `[dependencies]`) and calls it rather than writing its own. An error answers \
             the wire code the design's mapping spells, `Error::bad_request(\"<code>\", ..)` \
             over the macro, which spells the default; a status the host's `Error` lacks (`401`) \
             is the part's own error type, as the handlers reference shows under \"A status of \
             its own\". A refusal the specification codes is checked by the handler over \
             optional fields, since axum's own rejection answers `422`, or a plain `400`, for a \
             field that is missing or malformed.\n\n",
        );
        if !self.plan.rules.paragraphs.is_empty() {
            text.push_str("The design says of them:\n\n");
            for paragraph in &self.plan.rules.paragraphs {
                for line in paragraph.lines() {
                    let _ = writeln!(text, "> {line}");
                }
                text.push('\n');
            }
        }
        if !self.plan.exports.is_empty() {
            text.push_str(
                "Exported by the stems already in the tree, to call rather than write again:\n\n",
            );
            for export in &self.plan.exports {
                let _ = write!(
                    text,
                    "- `{}` (`{}:{}`): `{}`",
                    export.path, export.file, export.line, export.signature
                );
                if let Some(doc) = &export.doc {
                    let _ = write!(text, " — {doc}");
                }
                text.push('\n');
            }
            text.push('\n');
        }
    }

    // The gate the design's rules spell, where they spell one: for each stem
    // with a handler under its prefix, where the gate stands and that the
    // handlers call it as laid; for a slice with no route under it, that
    // none applies.
    fn brief_gate(&self, text: &mut String) {
        let Some(gate) = &self.plan.gate else { return };
        let mut gated = false;
        for stem in &self.plan.stems {
            let Some(gating) = &stem.gating else { continue };
            gated = true;
            let prefix = escape(&gate.prefix);
            let calling = format!(
                "every handler of this slice under `{prefix}` takes `headers: HeaderMap` after \
                 the context, calls `{}(&context, &headers).await?` before anything else, and \
                 answers `std::result::Result<_, {}>`, as laid; its tests send `AUTHORIZATION` \
                 from `let authorization = \"<authorization>\";`, which you fill with a \
                 credential the seeded settings accept",
                Gate::FN,
                Gate::ERROR
            );
            match &gating.source {
                gate::Source::Laid => {
                    let settings = gate.settings.as_ref().map_or_else(String::new, |settings| {
                        let keys: Vec<String> =
                            settings.keys().map(|key| format!("`{key}`")).collect();
                        format!(
                            " and `{}` over {}, which `provider()` seeds in every test file \
                             under `{prefix}`",
                            settings.name,
                            listed(&keys)
                        )
                    });
                    let _ = writeln!(
                        text,
                        "The gate under `{prefix}` the design's rules spell is laid in `{}`: `pub \
                         async fn {}<P: Config>(context: &Context<P>, headers: &HeaderMap) -> \
                         std::result::Result<(), {error}>` beside `{error}` (its `Unauthorized` \
                         answering `401 {{\"error\": \"{}\"}}`, its `Host` passing the host's \
                         error through){settings}. Fill the gate's marker, listed under the \
                         stem, with the rule as the design states it, `Err({error}::Unauthorized)` \
                         where the credential is missing or unknown, and {calling}. Write no \
                         second gate: a later stem imports this one.\n",
                        stem.module.path,
                        Gate::FN,
                        escape(&gate.code),
                        error = Gate::ERROR,
                    );
                }
                gate::Source::Held => {
                    let _ = writeln!(
                        text,
                        "The gate under `{prefix}` stands in `{}` from an earlier slice, and \
                         {calling}. Write no second gate.\n",
                        gating.file
                    );
                }
                gate::Source::Imported { path, dependency } => {
                    let dependency = dependency.as_ref().map_or_else(String::new, |dependency| {
                        format!(", `{}` under its `[dependencies]`", dependency.line())
                    });
                    let _ = writeln!(
                        text,
                        "The gate under `{prefix}` is `{path}::{}` in `{}`, imported into `{}`{dependency}, and \
                         {calling}. Write no second gate.\n",
                        Gate::FN,
                        gating.file,
                        stem.module.path,
                    );
                }
                gate::Source::Named { path } => {
                    let _ = writeln!(
                        text,
                        "The tree already holds a function taking the request's headers, \
                         `{path}` in `{}`, outside the laid shape, so no gate was laid and the \
                         handlers of this slice under `{prefix}` take no `HeaderMap` yet: give \
                         each `headers: HeaderMap` after the context and call it before \
                         anything else, send `AUTHORIZATION` from each test under `{prefix}`, \
                         and write no second gate.\n",
                        gating.file
                    );
                }
            }
        }
        if !gated && self.plan.stems.iter().any(|stem| !stem.handlers.is_empty()) {
            let _ = writeln!(
                text,
                "The gate under `{}` the design's rules spell guards none of this slice's \
                 routes, so none is laid here and no handler of this slice calls one.\n",
                escape(&gate.prefix)
            );
        }
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
            if stem.handlers.iter().any(|handler| handler.status) {
                laid.insert(Laid::Status);
            }
            if !stem.consumers.is_empty() {
                laid.insert(Laid::Consumers);
            }
            if stem.settings.is_some() {
                laid.insert(Laid::Settings);
            }
            if stem.gating.as_ref().is_some_and(Gating::calls) {
                laid.insert(Laid::Gate);
            }
            if stem.tests.is_some() {
                laid.insert(Laid::Tests);
            }
            if !stem.types.is_empty() {
                laid.insert(Laid::Types);
            }
            if !stem.data.is_empty() {
                laid.insert(Laid::Data);
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
            self.brief_dependencies(),
            self.brief_dependency_count()
        );
        let mut entry_lines: Vec<String> =
            self.modules().iter().map(|module| format!("`pub mod {module};`")).collect();
        if self.registers() {
            entry_lines.push(format!("`{}::handlers()`", self.part));
        }
        let _ = writeln!(
            text,
            "- `{}`: {}, {} {} its {}.",
            self.entry.path,
            self.entry.origin(),
            listed(&entry_lines),
            if self.entry.held { "added to" } else { "in" },
            if self.registers() { "lists" } else { "list" }
        );
        let _ = write!(
            text,
            "- `{}`: {}. {}",
            self.module.path,
            self.module.origin(),
            if self.module.extended {
                "Extended with this slice's markers, listed below; fill them through `edit_files` \
                 and keep what the earlier slice wrote."
            } else if self.module.held {
                HELD
            } else {
                "Fill its markers, listed below, through `edit_files`."
            }
        );
        self.brief_text(Kind::Module, budget, text);
        match &self.tests {
            Some(tests) if tests.extended => {
                let _ = write!(
                    text,
                    "- `{}`: held, extended with one ignored test per scenario of this slice. \
                     Fill each new test through `edit_files`, replacing the block listed below, \
                     and keep the earlier tests passing.",
                    tests.path
                );
                self.brief_text(Kind::Tests, budget, text);
            }
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

    // The clause after the manifest's origin naming the topic table's crate
    // laid among its dependencies and what for, or nothing for a part that
    // names no topic.
    // The stem's `[dependencies]` lines beyond the fixed four, each with how
    // the manifest came to hold it and what for.
    fn brief_dependencies(&self) -> String {
        let lines = self.dependencies();
        if lines.is_empty() {
            return String::new();
        }
        let spelled: Vec<String> = lines
            .iter()
            .map(|line| {
                let placed = self
                    .lines
                    .iter()
                    .find(|held| held.path == self.manifest.path && held.text == *line);
                let how = match placed {
                    None => "among",
                    Some(laid) if laid.placement == Placement::Held => "already held among",
                    Some(_) => "added at the end of",
                };
                let purpose = if *line == render::dependency_line(TOPIC_DEPENDENCY) {
                    "the topic table's constants"
                } else if line.contains("path = ") {
                    "the gate it imports"
                } else {
                    "the gate's error type"
                };
                format!("`{line}` {how} its `[dependencies]` for {purpose}")
            })
            .collect();
        format!(", {}", listed(&spelled))
    }

    // How many crates the manifest names as scaffolded, in words.
    fn brief_dependency_count(&self) -> &'static str {
        match STEM_DEPENDENCIES.len() + self.dependencies().len() {
            4 => "four",
            5 => "five",
            6 => "six",
            7 => "seven",
            _ => "several",
        }
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
            "- `Cargo.toml`, `{ENTRY}`: the stem's lines {}: {}.",
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
        for registration in &self.unanchored_registrations {
            let _ = writeln!(
                text,
                "- {registration} holds no chain anchor, so the registration was not added: \
                 register the handler in the router chain through `edit_files`."
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
            .filter(|design_type| !design_type.body.is_marker() && !design_type.body.is_table())
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

    // The settings struct laid from the keys the slice spells, what to load
    // it with, and where its values are seeded.
    fn brief_settings(&self, text: &mut String) {
        let Some(settings) = &self.settings else { return };
        let keys: Vec<String> = settings.keys().map(|key| format!("`{key}`")).collect();
        let _ = writeln!(
            text,
            "The settings `{}` in `{}` read {}, as the slice spells them: load them with \
             `{}::load(context.config()).await?` where a body needs one, narrow each field from \
             `String` to the type its value parses as, drop a field the slice spells for \
             something other than a setting, and give each key its value in place of \
             `\"<value>\"` in the tests' `provider()`.\n",
            settings.name,
            self.module.path,
            listed(&keys),
            settings.name
        );
    }

    fn brief_markers(&self, text: &mut String, wire: Option<Wire>) {
        self.brief_types(text);
        self.brief_settings(text);
        if !self.module.laid() || self.tests.as_ref().is_some_and(|tests| !tests.laid()) {
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
        if let Some(gate) = self.laid_gate() {
            count += 1;
            self.brief_gate_marker(gate, count, text);
        }
        for handler in &self.handlers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. The handler `{}` of `{}` in `{}`, key `{}`: fill {} with the wire's \
                 fields, then replace this block with the body. {}{}{} Narrow `P: Capabilities` \
                 to the traits the body reaches through the context, `P: StateStore + Config` \
                 for `context.state()` and `context.config()`.\n\n{}",
                handler.name,
                handler.route,
                self.module.path,
                handler.marker,
                handler.filled(),
                handler.status_hint(),
                handler.params_hint(),
                handler.body_hint(),
                render::fenced("rust", &handler.stub())
            );
        }
        for consumer in &self.consumers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. The consumer `{}` of the topic `{}` in `{}`, key `{}`, registered in the \
                 part's `handlers()` under `topic::{}`: fill `{}` with the delivery's fields, \
                 then replace this block with the body, narrowing `P: Capabilities` to the \
                 traits the body reaches. {}\n\n{}",
                consumer.name,
                consumer.topic,
                self.module.path,
                consumer.marker,
                consumer.constant,
                consumer.input,
                consumer.decoder_hint(wire),
                render::fenced("rust", &consumer.stub())
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

    // The gate's entry among the markers: its body to fill, its signature
    // to keep.
    fn brief_gate_marker(&self, gate: &Gate, count: usize, text: &mut String) {
        let loaded = gate.settings.as_ref().map_or_else(
            || "the context's config".to_owned(),
            |settings| format!("`{}::load(context.config()).await?`", settings.name),
        );
        let _ = writeln!(
            text,
            "{count}. The gate `{}` in `{}`, key `{}`: replace this block with the body, the rule \
             as the design states it over {loaded} and the request's headers, `Ok(())` for an \
             accepted credential and `Err({}::Unauthorized)` for a missing or unknown one; keep \
             the signature, which every handler under `{}` calls.\n\n{}",
            Gate::FN,
            self.module.path,
            gate.marker(),
            Gate::ERROR,
            escape(&gate.prefix),
            render::fenced("rust", &render::gate_stub(gate))
        );
    }
}

impl Consumer {
    // The clause of the consumer's entry on its decoder: the format the
    // design names with the reader for it, else the conditional.
    fn decoder_hint(&self, wire: Option<Wire>) -> String {
        wire.map_or_else(
            || {
                format!(
                    "Where the payload is not JSON, register it through `.topic_with(topic::{}, \
                     {}, decode)` instead, over a `fn decode(delivery: &Delivery) -> \
                     std::result::Result<{}, DecodeError>` of the part.",
                    self.constant, self.name, self.input
                )
            },
            |wire| {
                format!(
                    "The design says the payload is {}, so register it through \
                     `.topic_with(topic::{}, {}, decode)` instead, over a `fn decode(delivery: \
                     &Delivery) -> std::result::Result<{}, DecodeError>` of the part{}.",
                    wire.name(),
                    self.constant,
                    self.name,
                    self.input,
                    wire.reader()
                )
            },
        )
    }
}

impl Handler {
    // Where the method carries a body the requirements state none of, that
    // no input was laid and where one goes if a scenario's wire needs it.
    fn body_hint(&self) -> String {
        if self.input.is_some() || !trigger::carries_body(&self.route.method) {
            return String::new();
        }
        format!(
            " No input struct was laid, since no requirement or scenario of `{}` states a body, \
             and its tests send none: a bodiless `{}` takes no `Json` extractor, which would \
             refuse the request with `415`; where a scenario's wire does carry a body, add a \
             `#[derive(Debug, Deserialize)]` input struct and the `Json(input): Json<..>` \
             extractor, and send it from the test with `CONTENT_TYPE` set.",
            self.route, self.route.method
        )
    }

    // The types the turn fills with the wire's fields: the input where the
    // method carries a body, and the output.
    fn filled(&self) -> String {
        self.input.as_ref().map_or_else(
            || format!("`{}`", self.output),
            |input| format!("`{input}` and `{}`", self.output),
        )
    }

    // How the handler's `Ok` reaches the wire, and where another status is
    // set.
    fn status_hint(&self) -> String {
        if self.status {
            format!(
                "Its `Ok` is a `(StatusCode, Json<{}>)` pair, since a scenario states a status: \
                 answer the status the outcome earns, `StatusCode::CREATED` for a creation, \
                 `StatusCode::SERVICE_UNAVAILABLE` where a probe found its store down; an error \
                 status, a `404`, is the error the errors passage maps to it.",
                self.output
            )
        } else {
            format!(
                "Its `Ok` is answered `200` as JSON; for another status, change the return type \
                 to `Result<(StatusCode, Json<{}>)>` and answer the pair, as the handlers \
                 reference shows.",
                self.output
            )
        }
    }

    // How the path's parameters reach the body, where the route has any.
    fn params_hint(&self) -> String {
        if self.params.is_empty() {
            return String::new();
        }
        " Each path parameter is extracted as a `String`; where the design keys it as an \
         integer, extract it typed, `Path<i64>`, and let axum answer `400` for a malformed one."
            .to_owned()
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
