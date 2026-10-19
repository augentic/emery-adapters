//! Scaffolds a slice's skeleton before its turn: the stem's crate, the part, its tests, and its lines in every list.

mod names;
mod reference;
mod render;
mod route;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

use emery_sdk::target::{DesignType, Requirement, Scenario, Slice};
use emery_sdk::{Doc, Error, INLINE_BYTES, Route, bad_request, server_error};

use self::names::{Names, escape, flat, number, pascal, screaming, snake};
use self::reference::{Needs, Role, Shape};
use self::render::{Call, Expr, Place};
use crate::tree;

const TOPIC_TABLE: &str = "crates/common/src/topic.rs";
const CHECK: &str = "cargo check --target wasm32-wasip2 && cargo +nightly fmt --all -- --check";

pub struct Scaffold {
    stems: Vec<Stem>,
    start: Vec<String>,
    relaid: Vec<Relaid>,
    existing: BTreeSet<String>,
    lines: Vec<Line>,
    listing: BTreeSet<String>,
    roles: BTreeSet<Role>,
}

// What a check reads of one scenario the scaffold wrote a test for.
pub struct Expected<'a> {
    pub path: &'a str,
    pub id: &'a str,
    pub scenario: &'a str,
    pub test: &'a str,
}

struct Stem {
    name: String,
    dir: String,
    package: String,
    part: String,
    doc: Vec<String>,
    manifest: File,
    entry: File,
    module: File,
    tests: Option<File>,
    paths: Vec<(String, String)>,
    handlers: Vec<Handler>,
    consumers: Vec<Consumer>,
    functions: Vec<Function>,
    types: Vec<Type>,
    scenarios: Vec<Test>,
}

#[derive(Clone, Copy)]
enum Kind {
    Manifest,
    Entry,
    Module,
    Tests,
}

struct File {
    path: String,
    held: bool,
    // What the tree holds at the path, where it holds a file.
    text: String,
}

struct Handler {
    name: String,
    input: String,
    output: String,
    constant: String,
    route: Route,
    params: Vec<Param>,
    stated: Vec<Stated>,
    marker: String,
    // The route function over `handle_with` scaffolded beside a `POST`
    // handler, and beside one whose scenarios state a status other than
    // `200`, so another status is one token's edit.
    route_fn: Option<String>,
}

struct Consumer {
    name: String,
    input: String,
    constant: String,
    topic: String,
    declared: bool,
    stated: Vec<Stated>,
    marker: String,
}

struct Param {
    wire: String,
    field: String,
}

struct Stated {
    id: String,
    subject: String,
    statement: String,
}

struct Function {
    name: String,
    stated: Stated,
    marker: String,
}

struct Type {
    name: String,
    key: String,
    signature: String,
    is_enum: bool,
}

struct Test {
    name: String,
    id: String,
    subject: String,
    scenario: Scenario,
    ignore: String,
    marker: String,
    act: Act,
}

// What a scenario's test acts through: the route or the topic its
// requirement carries, or the provider alone for a function.
#[derive(Clone)]
enum Act {
    Route(Route),
    Topic(String),
    Call,
}

// A `start` requirement laid under the crate of its route's resource.
struct Relaid {
    id: String,
    route: String,
    stem: String,
}

struct Line {
    path: String,
    text: String,
    place: Place,
    standing: Standing,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Standing {
    Held,
    Add,
    Unanchored,
}

// The topic table as the tree holds it: each constant's name and topic,
// and the names claimed for the topics the slice declares.
struct Table {
    declared: Vec<(String, String)>,
    names: Names,
}

// The names a part's items claim, each kind against its own namespace,
// the module's imports and its functions reserved.
struct Claims {
    values: Names,
    types: Names,
    constants: Names,
    tests: Names,
}

// How much of the brief the inlined texts may take: the files first, the
// references after them, a text past the budget named by its path alone.
struct Budget {
    left: u64,
}

impl Default for Claims {
    fn default() -> Self {
        Self {
            values: Names::reserving(&[
                "routes",
                "topics",
                "get",
                "post",
                "put",
                "patch",
                "delete",
                "handle_with",
                "decode_json",
            ]),
            types: Names::reserving(&[
                "Capabilities",
                "Client",
                "Context",
                "DecodeError",
                "Deserialize",
                "DeserializeOwned",
                "IntoResponse",
                "Json",
                "MethodFilter",
                "MethodRouter",
                "RawRequest",
                "Serialize",
                "StatusCode",
            ]),
            constants: Names::default(),
            tests: Names::reserving(&["provider", "send", "deliver"]),
        }
    }
}

impl Scaffold {
    pub fn plan(slice: &Slice, root: &Path) -> Result<Self, Error> {
        let existing: BTreeSet<String> = tree::crates(root)?.into_iter().collect();
        let mut table = Table::read(root)?;

        // each requirement to its stem: `start` set aside, unless a typed
        // route names the resource whose crate serves it
        let mut stems: Vec<Stem> = Vec::new();
        let mut laid: Vec<Vec<&Requirement>> = Vec::new();
        let mut start = Vec::new();
        let mut relaid = Vec::new();
        for requirement in &slice.requirements {
            let own = requirement.stem();
            let name = if tree::is_start_stem(own) {
                let resource = route::of(requirement)
                    .map(|route| (route, route::resource(route)))
                    .and_then(|(route, stem)| Some((route, stem?)))
                    .filter(|(_, stem)| !tree::is_start_stem(stem));
                let Some((route, stem)) = resource else {
                    start.push(requirement.id.clone());
                    continue;
                };
                relaid.push(Relaid {
                    id: requirement.id.clone(),
                    route: route.to_string(),
                    stem: stem.clone(),
                });
                stem
            } else {
                own.to_owned()
            };
            if let Some(at) = stems.iter().position(|stem| stem.name == name) {
                laid[at].push(requirement);
            } else {
                stems.push(Stem::new(&name, slice));
                laid.push(vec![requirement]);
            }
        }

        // each owned type to the stem its key leads with, else the first
        let mut owned: Vec<Vec<&DesignType>> = vec![Vec::new(); stems.len()];
        for design_type in &slice.types {
            let first = design_type.key.split('.').next().unwrap_or(&design_type.key);
            let at = stems
                .iter()
                .position(|stem| stem.name == first)
                .or_else(|| (!stems.is_empty()).then_some(0));
            if let Some(at) = at {
                owned[at].push(design_type);
            }
        }

        for ((stem, types), requirements) in stems.iter_mut().zip(&owned).zip(&laid) {
            stem.plan(types, requirements, &mut table);
            stem.read(root)?;
        }
        let mut lines: Vec<Line> = Vec::new();
        for line in stems.iter().flat_map(Stem::lines) {
            if !lines.iter().any(|held| held.path == line.path && held.text == line.text) {
                lines.push(line);
            }
        }
        for line in &mut lines {
            line.read(root)?;
        }
        Ok(Self {
            stems,
            start,
            relaid,
            existing,
            lines,
            listing: BTreeSet::new(),
            roles: reference::roles(&slice.design),
        })
    }

    pub fn write(&mut self, root: &Path) -> Result<Vec<String>, Error> {
        let mut written = Vec::new();
        for stem in &self.stems {
            for kind in [Kind::Manifest, Kind::Entry, Kind::Module, Kind::Tests] {
                if let Some(file) = stem.file(kind).filter(|file| !file.held) {
                    create(root, &file.path, &stem.render(kind))?;
                    written.push(file.path.clone());
                }
            }
        }

        // every list file once, each of its lines in order
        let mut paths: Vec<&str> = Vec::new();
        for line in self.lines.iter().filter(|line| line.standing == Standing::Add) {
            if !paths.contains(&line.path.as_str()) {
                paths.push(&line.path);
            }
        }
        for path in paths {
            let file = root.join(path);
            let mut text = tree::read(&file)?;
            for line in &self.lines {
                if line.path == path
                    && line.standing == Standing::Add
                    && let Some(next) = render::insert(&text, &line.text, &line.place)
                {
                    text = next;
                }
            }
            fs::write(&file, text).map_err(|err| server_error!("writing `{path}`: {err}"))?;
            written.push(path.to_owned());
        }
        self.listing = tree::listing(root)?;
        Ok(written)
    }

    // A slice of `start` requirements alone, none carrying a route, lays no
    // stem: nothing is scaffolded and no turn is opened for it.
    pub const fn is_empty(&self) -> bool {
        self.stems.is_empty()
    }

    // Every file the scaffold wrote or found and every list it adds a line
    // to, sorted: what the turn changes through `edit_files` alone.
    pub fn paths(&self) -> Vec<String> {
        let mut paths: Vec<String> = self
            .stems
            .iter()
            .flat_map(|stem| {
                [Kind::Manifest, Kind::Entry, Kind::Module, Kind::Tests]
                    .into_iter()
                    .filter_map(|kind| stem.file(kind))
                    .map(|file| file.path.clone())
            })
            .chain(self.lines.iter().map(|line| line.path.clone()))
            .collect();
        paths.sort();
        paths.dedup();
        paths
    }

    // The files of the tree once the skeleton was written, which the check
    // holds the tree to: a file beyond them outside the slice's crates is a
    // stray.
    pub const fn listing(&self) -> &BTreeSet<String> {
        &self.listing
    }

    pub fn brief(&self, docs: &[Doc]) -> String {
        let mut budget = Budget { left: INLINE_BYTES };
        let mut text = String::new();
        for stem in &self.stems {
            stem.brief_files(&self.lines, &mut budget, &mut text);
            stem.brief_markers(&mut text);
        }
        if !self.start.is_empty() {
            let _ = write!(
                text,
                "The `start` stem ({}): the seed and the host are `start`, so nothing was \
                 scaffolded for it; cover what they satisfy and leave the rest out, as the \
                 guardrails' table says.",
                self.start.join(", ")
            );
            text.push_str("\n\n");
        }
        for relaid in &self.relaid {
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
        self.brief_references(docs, &mut budget, &mut text);
        text.trim_end().to_owned()
    }

    // The one shell command the turn checks the tree with: the slice's
    // packages under clippy and their tests, then the guest check and the
    // formatter over the workspace.
    pub fn check(&self) -> String {
        let packages: Vec<String> =
            self.stems.iter().map(|stem| format!("-p {}", stem.package)).collect();
        let packages = packages.join(" ");
        format!(
            "cargo clippy {packages} --all-targets -- -D warnings && cargo test {packages} && \
             {CHECK}"
        )
    }

    fn brief_references(&self, docs: &[Doc], budget: &mut Budget, text: &mut String) {
        let mut shapes = BTreeSet::new();
        for stem in &self.stems {
            if !stem.handlers.is_empty() {
                shapes.insert(Shape::Routes);
            }
            if stem.handlers.iter().any(|handler| handler.route_fn.is_some()) {
                shapes.insert(Shape::Encoded);
            }
            if !stem.consumers.is_empty() {
                shapes.insert(Shape::Consumers);
            }
            if stem.tests.is_some() {
                shapes.insert(Shape::Tests);
            }
            if !stem.types.is_empty() {
                shapes.insert(Shape::Types);
            }
        }
        let needs = Needs {
            shapes,
            roles: self.roles.clone(),
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
            return;
        }
        text.push_str(
            "What the references spell for these shapes, quoted; `read_doc` answers the rest of \
             each document:\n\n",
        );
        text.push_str(&quoted);
    }

    pub fn stems(&self) -> impl Iterator<Item = &str> {
        self.stems.iter().map(|stem| stem.name.as_str())
    }

    pub const fn existing(&self) -> &BTreeSet<String> {
        &self.existing
    }

    pub fn expected(&self) -> impl Iterator<Item = Expected<'_>> {
        self.stems.iter().filter_map(|stem| stem.tests.as_ref().map(|file| (stem, file))).flat_map(
            |(stem, file)| {
                stem.scenarios.iter().map(move |test| Expected {
                    path: &file.path,
                    id: &test.id,
                    scenario: &test.scenario.name,
                    test: &test.name,
                })
            },
        )
    }
}

impl Stem {
    fn new(name: &str, slice: &Slice) -> Self {
        let part = part_of(&slice.name, name);
        let doc = match slice.brief.first() {
            Some(paragraph) if !paragraph.trim().is_empty() => {
                paragraph.lines().map(|line| line.trim_end().to_owned()).collect()
            }
            _ => vec![format!("The `{part}` part of the `{name}` stem.")],
        };
        let dir = crate_dir(name);
        let root = format!("crates/{dir}");
        Self {
            name: name.to_owned(),
            dir: dir.to_owned(),
            package: package(name),
            manifest: File::at(format!("{root}/Cargo.toml")),
            entry: File::at(format!("{root}/src/lib.rs")),
            module: File::at(format!("{root}/src/{part}.rs")),
            tests: None,
            part,
            doc,
            paths: Vec::new(),
            handlers: Vec::new(),
            consumers: Vec::new(),
            functions: Vec::new(),
            types: Vec::new(),
            scenarios: Vec::new(),
        }
    }

    // Names every item the part declares, the design types first so one
    // keeps its name over a handler's wrapper.
    fn plan(&mut self, owned: &[&DesignType], requirements: &[&Requirement], table: &mut Table) {
        let mut claims = Claims::default();
        for design_type in owned {
            let key = design_type.key.as_str();
            let last = key.rsplit('.').next().unwrap_or(key);
            let first = key.split('.').next().unwrap_or(key);
            self.types.push(Type {
                name: claims.types.claim(&pascal(last), &pascal(first)),
                key: key.to_owned(),
                is_enum: is_enum(&design_type.signature),
                signature: design_type.signature.clone(),
            });
        }

        // the routed requirements grouped by route, the consumers by topic,
        // the rest a function each
        let mut routed: Vec<(&Route, Vec<&Requirement>)> = Vec::new();
        let mut consumed: Vec<(&str, Vec<&Requirement>)> = Vec::new();
        for requirement in requirements {
            let suffix = format!("_{}", number(&requirement.id));
            let act = if let Some(route) = route::of(requirement) {
                group(&mut routed, route, requirement);
                Act::Route(route.clone())
            } else if let Some(topic) = route::message(requirement) {
                group(&mut consumed, topic, requirement);
                Act::Topic(topic.to_owned())
            } else {
                let tail = tail(&requirement.subject);
                let base = if tail.is_empty() { snake(&self.name) } else { snake(tail) };
                self.functions.push(Function {
                    name: claims.values.claim(&base, &suffix),
                    marker: format!("{} {}: the behaviour", requirement.id, requirement.subject),
                    stated: Stated::of(requirement),
                });
                Act::Call
            };
            for scenario in &requirement.scenarios {
                let name = flat(&scenario.name);
                self.scenarios.push(Test {
                    name: claims.tests.claim(&snake(&name), &suffix),
                    id: requirement.id.clone(),
                    subject: requirement.subject.clone(),
                    scenario: scenario.clone(),
                    ignore: format!("{} {name}: unfilled", requirement.id),
                    marker: format!("{} {name}: arrange, act, assert", requirement.id),
                    act: act.clone(),
                });
            }
        }
        for (route, group) in routed {
            self.handler(route, &group, &mut claims);
        }
        for (topic, group) in consumed {
            self.consumer(topic, &group, &mut claims, table);
        }

        if !self.scenarios.is_empty() {
            self.tests = Some(File::at(format!("crates/{}/tests/{}.rs", self.dir, self.part)));
        }
    }

    // One handler for the requirements served at `route`, over the path's
    // one constant.
    fn handler(&mut self, route: &Route, group: &[&Requirement], claims: &mut Claims) {
        let suffix = number(&group[0].id);
        let base = handler_base(group, &route.to_string());
        let name = claims.values.claim(&base, &format!("_{suffix}"));
        let title = pascal(&name);
        let constant = if let Some((constant, _)) =
            self.paths.iter().find(|(_, path)| *path == route.path)
        {
            constant.clone()
        } else {
            let constant = claims.constants.claim(&screaming(&route.path), &format!("_{suffix}"));
            self.paths.push((constant.clone(), route.path.clone()));
            constant
        };
        let route_fn = (route.method == "POST" || states_status(group))
            .then(|| claims.values.claim(&format!("{name}_route"), &format!("_{suffix}")));
        let stated = stated_of(group);
        self.handlers.push(Handler {
            input: claims.types.claim(&format!("{title}Input"), &suffix),
            output: claims.types.claim(&format!("{title}Output"), &suffix),
            name,
            constant,
            params: route
                .params()
                .map(|wire| Param {
                    field: snake(wire),
                    wire: wire.to_owned(),
                })
                .collect(),
            route: route.clone(),
            marker: format!("{}: the handler", named(&stated)),
            stated,
            route_fn,
        });
    }

    // One consumer for the requirements that consume `topic`, under the
    // table's constant for it.
    fn consumer(
        &mut self, topic: &str, group: &[&Requirement], claims: &mut Claims, table: &mut Table,
    ) {
        let suffix = number(&group[0].id);
        let base = format!("on_{}", handler_base(group, topic));
        let name = claims.values.claim(&base, &format!("_{suffix}"));
        let (constant, declared) = table.constant(topic);
        let stated = stated_of(group);
        self.consumers.push(Consumer {
            input: claims.types.claim(&format!("{}Input", pascal(&name)), &suffix),
            name,
            constant,
            topic: topic.to_owned(),
            declared,
            marker: format!("{}: the consumer", named(&stated)),
            stated,
        });
    }

    fn read(&mut self, root: &Path) -> Result<(), Error> {
        let files = [&mut self.manifest, &mut self.entry, &mut self.module];
        for file in files.into_iter().chain(self.tests.as_mut()) {
            file.read(root)?;
        }
        Ok(())
    }

    const fn file(&self, kind: Kind) -> Option<&File> {
        match kind {
            Kind::Manifest => Some(&self.manifest),
            Kind::Entry => Some(&self.entry),
            Kind::Module => Some(&self.module),
            Kind::Tests => self.tests.as_ref(),
        }
    }

    fn render(&self, kind: Kind) -> String {
        match kind {
            Kind::Manifest => render::manifest(self),
            Kind::Entry => render::entry(self),
            Kind::Module => render::module(self),
            Kind::Tests => render::tests(self),
        }
    }

    // The file's text as the tree holds it after the scaffold: what was
    // written, or what was held.
    fn text(&self, kind: Kind) -> Option<String> {
        let file = self.file(kind)?;
        Some(if file.held { file.text.clone() } else { self.render(kind) })
    }

    // The stem's lines in the root lists, the constant of each topic the
    // slice declares in the table, and, where the tree holds its entry, the
    // part's lines in the entry's.
    fn lines(&self) -> Vec<Line> {
        let ident = self.package.replace('-', "_");
        let mut lines = vec![
            Line::new(
                "Cargo.toml",
                format!("{} = {{ path = \"crates/{}\" }}", self.package, self.dir),
                Place::PathList,
            ),
            Line::new("Cargo.toml", format!("{}.workspace = true", self.package), Place::FileEnd),
            Line::new(
                "src/routes.rs",
                format!("    let router = router.merge({ident}::router());"),
                Place::Before("router.with_state("),
            ),
            Line::new(
                "src/messaging.rs",
                format!("    let router = {ident}::topics(router, &env);"),
                Place::Before("tracing::debug!("),
            ),
        ];
        for consumer in self.consumers.iter().filter(|consumer| !consumer.declared) {
            lines.push(Line::new(
                TOPIC_TABLE,
                format!("\n{}", render::topic_constant(&consumer.constant, &consumer.topic)),
                Place::FileEnd,
            ));
        }
        if self.entry.held {
            lines.push(Line::new(
                &self.entry.path,
                format!("pub mod {};", self.part),
                Place::PubModList,
            ));
            if !self.handlers.is_empty() {
                lines.push(Line::new(
                    &self.entry.path,
                    format!("        {}::routes(),", self.part),
                    Place::RoutesArray,
                ));
            }
            if !self.consumers.is_empty() {
                lines.push(Line::new(
                    &self.entry.path,
                    format!("        {}::topics,", self.part),
                    Place::TopicsArray,
                ));
            }
        }
        lines
    }

    fn brief_files(&self, lines: &[Line], budget: &mut Budget, text: &mut String) {
        let _ = writeln!(text, "Stem `{}`, part `{}`:\n", self.name, self.part);
        let _ = writeln!(
            text,
            "- `{}`: {}. One `<crate>.workspace = true` line at the end of `[dependencies]` for \
             each crate your code uses beyond the six it holds; a crate the root manifest does \
             not pin carries its version instead.",
            self.manifest.path,
            self.manifest.standing()
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
            self.entry.standing(),
            listed(&entry_lines),
            if self.entry.held { "added to" } else { "in" }
        );
        let _ = write!(
            text,
            "- `{}`: {}. {}",
            self.module.path,
            self.module.standing(),
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
        self.brief_lines(lines, text);
    }

    // The stem's lines in the root lists and the topic table, each spelled,
    // and the ones no anchor took.
    fn brief_lines(&self, lines: &[Line], text: &mut String) {
        let standing = |own: &Line| {
            lines
                .iter()
                .find(|line| line.path == own.path && line.text == own.text)
                .map_or(Standing::Unanchored, |line| line.standing)
        };
        let own = self.lines();
        let root: Vec<&Line> =
            own.iter().filter(|line| !line.path.starts_with("crates/")).collect();
        let added = root.iter().any(|line| standing(line) == Standing::Add);
        let spelled: Vec<String> = root
            .iter()
            .filter(|line| line.path != TOPIC_TABLE)
            .map(|line| format!("`{}` in `{}`", line.text.trim(), line.path))
            .collect();
        let _ = writeln!(
            text,
            "- `Cargo.toml`, `src/routes.rs`, `src/messaging.rs`: the stem's lines {}: {}.",
            if added { "added" } else { "already held" },
            spelled.join(", ")
        );
        for consumer in &self.consumers {
            let _ = writeln!(
                text,
                "- `{TOPIC_TABLE}`: the constant `{}` for the topic `{}` {}: `pub const {}: &str = \
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
        for line in own.iter().filter(|line| standing(line) == Standing::Unanchored) {
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

    fn brief_markers(&self, text: &mut String) {
        if self.module.held || self.tests.as_ref().is_some_and(|tests| tests.held) {
            text.push_str(
                "The markers below are the ones a fresh scaffold writes; a held file holds \
                 whatever stands in it.\n\n",
            );
        }
        text.push_str(
            "Markers, in fill order, each one `edit_files` edit whose `find` is the block quoted \
             under it:\n\n",
        );
        let mut count = 0;
        for design_type in &self.types {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `pub {} {{}}` in `{}`: the design type `{}`. Replace it with the {} the \
                 signature in its doc spells, every field documented; drop `Eq` from its derives \
                 where a field is a float.\n",
                render::declaration(design_type),
                self.module.path,
                design_type.key,
                if design_type.is_enum { "enum" } else { "struct" }
            );
        }
        for handler in &self.handlers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. The handler `{}` of `{}` in `{}`: fill `{}` and `{}` with the wire's \
                 fields, then replace this block with the body. {}\n\n{}",
                handler.name,
                handler.route,
                self.module.path,
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
                "{count}. The consumer `{}` of the topic `{}` in `{}`, registered in the part's \
                 `topics(..)` under `topic::{}`: fill `{}` with the delivery's fields, then \
                 replace this block with the body.\n\n{}",
                consumer.name,
                consumer.topic,
                self.module.path,
                consumer.constant,
                consumer.input,
                render::fenced("rust", &render::stub(&consumer.marker))
            );
        }
        for function in &self.functions {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. The function `{}` in `{}`, since the requirement states no route and no \
                 message: give it the signature the behaviour needs and replace this block with \
                 the body.\n\n{}",
                function.name,
                self.module.path,
                render::fenced("rust", &render::marker_line(&function.marker))
            );
        }
        if let Some(tests) = &self.tests {
            for test in &self.scenarios {
                count += 1;
                let _ = writeln!(
                    text,
                    "{count}. The test `{}` in `{}`: replace this block with the filled test, its \
                     `#[ignore = ..]` line gone: the arrangement over `provider()`, the act as \
                     laid, and the assertions the scenario states.\n\n{}",
                    test.name,
                    tests.path,
                    render::fenced("rust", &render::test_block(self, test))
                );
            }
        }
        if count == 0 {
            text.push_str("None: the slice states no requirement under this stem.\n\n");
        }
    }
}

const HELD: &str =
    "The tree already held it, so nothing was written over it; read it for what stands.";

impl File {
    const fn at(path: String) -> Self {
        Self {
            path,
            held: false,
            text: String::new(),
        }
    }

    fn read(&mut self, root: &Path) -> Result<(), Error> {
        let file = root.join(&self.path);
        self.held = match fs::symlink_metadata(&file) {
            Ok(meta) if meta.is_file() => {
                self.text = tree::read(&file)?;
                true
            }
            Ok(_) => {
                return Err(bad_request!(
                    "a directory or a link stands at `{}`, where the slice's scaffold writes a file",
                    self.path
                ));
            }
            Err(err) if err.kind() == ErrorKind::NotFound => false,
            Err(err) => return Err(server_error!("reading `{}`: {err}", self.path)),
        };
        Ok(())
    }

    const fn standing(&self) -> &'static str {
        if self.held { "held" } else { "scaffolded" }
    }
}

impl Handler {
    // The `.route(..)` call that registers the handler: its route function
    // where one was scaffolded, else the SDK's method fn over it.
    fn registration(&self) -> Call {
        let service = self.route_fn.as_ref().map_or_else(
            || Call::new(route::function(&self.route.method), [self.name.as_str()]),
            |route_fn| Call::new(route_fn.as_str(), Vec::<Expr>::new()),
        );
        Call::new(".route", [Expr::from(self.constant.as_str()), service.into()])
    }

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
                    route::function(&self.route.method),
                    self.name
                )
            },
            |route_fn| {
                format!(
                    "Its `Ok` is answered as JSON under the status `{route_fn}` names, \
                     `StatusCode::OK`; for another status on an `Ok`, a `201` or a `503`, \
                     change it there, and answer an error status, a `404`, as the error the \
                     errors passage maps to it."
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

impl Stated {
    // A requirement the engine lowers with no statement is one its sources
    // conflict over, so its line says so rather than ending at the colon.
    const CONFLICT: &str = "[conflict] the sources disagree; the specification's notes under this id say what each \
         states";

    fn of(requirement: &Requirement) -> Self {
        let statement = requirement
            .statement
            .first()
            .map(|line| flat(line))
            .filter(|line| !line.is_empty())
            .unwrap_or_else(|| Self::CONFLICT.to_owned());
        Self {
            id: requirement.id.clone(),
            subject: requirement.subject.clone(),
            statement,
        }
    }
}

impl Table {
    fn read(root: &Path) -> Result<Self, Error> {
        let file = root.join(TOPIC_TABLE);
        let text = match fs::symlink_metadata(&file) {
            Ok(meta) if meta.is_file() => tree::read(&file)?,
            Ok(_) => return Err(bad_request!("`{TOPIC_TABLE}` is not a regular file")),
            Err(err) if err.kind() == ErrorKind::NotFound => String::new(),
            Err(err) => return Err(server_error!("reading `{TOPIC_TABLE}`: {err}")),
        };
        let declared = constants_of(&text);
        let taken: Vec<&str> = declared.iter().map(|(name, _)| name.as_str()).collect();
        Ok(Self {
            names: Names::reserving(&taken),
            declared,
        })
    }

    // The table's constant for `topic`, and whether the table declares it
    // already; a topic no constant spells is named for it, once.
    fn constant(&mut self, topic: &str) -> (String, bool) {
        if let Some((name, _)) = self.declared.iter().find(|(_, held)| held == topic) {
            return (name.clone(), true);
        }
        let name = self.names.claim(&screaming(topic), "_TOPIC");
        self.declared.push((name.clone(), topic.to_owned()));
        (name, false)
    }
}

// The name and the topic of every `pub const NAME: &str = "topic";` in the
// table, a value the formatter laid on the line beneath its `=` read with
// the declaration.
fn constants_of(text: &str) -> Vec<(String, String)> {
    let mut declared = Vec::new();
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("pub const ") else { continue };
        let Some((name, rest)) = rest.split_once(':') else { continue };
        let Some((_, value)) = rest.split_once('=') else { continue };
        let value = if value.trim().is_empty() { lines.next().unwrap_or_default() } else { value };
        let Some(topic) = value
            .trim()
            .strip_suffix(';')
            .and_then(|value| value.trim().strip_prefix('"'))
            .and_then(|value| value.strip_suffix('"'))
        else {
            continue;
        };
        declared.push((name.trim().to_owned(), topic.to_owned()));
    }
    declared
}

impl Line {
    fn new(path: &str, text: String, place: Place) -> Self {
        Self {
            path: path.to_owned(),
            text,
            place,
            standing: Standing::Unanchored,
        }
    }

    fn read(&mut self, root: &Path) -> Result<(), Error> {
        let file = root.join(&self.path);
        self.standing = match fs::symlink_metadata(&file) {
            Err(err) if err.kind() == ErrorKind::NotFound => Standing::Unanchored,
            Err(err) => return Err(server_error!("reading `{}`: {err}", self.path)),
            Ok(meta) if !meta.is_file() => {
                return Err(bad_request!(
                    "`{}` is not a regular file, so the slice's line cannot be added to it",
                    self.path
                ));
            }
            Ok(_) => {
                let text = tree::read(&file)?;
                if text.lines().any(|held| held.trim() == self.key()) {
                    Standing::Held
                } else if render::insert(&text, &self.text, &self.place).is_some() {
                    Standing::Add
                } else {
                    Standing::Unanchored
                }
            }
        };
        Ok(())
    }

    // The line that tells whether the file holds this one: the last of a
    // text of several, since a constant's doc line tells nothing.
    fn key(&self) -> &str {
        self.text.lines().last().unwrap_or(&self.text).trim()
    }
}

// Whether a design signature declares an enumeration: a Rust or TypeScript
// `enum`, a TypeScript union of string literals, or a Python class over an
// `Enum` base.
fn is_enum(signature: &str) -> bool {
    let Some(head) = signature.lines().map(str::trim).find(|line| !line.is_empty()) else {
        return false;
    };
    let mut head = head;
    for prefix in ["export ", "declare ", "pub(crate) ", "pub ", "const "] {
        head = head.strip_prefix(prefix).unwrap_or(head);
    }
    if head.starts_with("enum ") {
        return true;
    }
    if let Some(rest) = head.strip_prefix("class ")
        && let Some((_, bases)) = rest.split_once('(')
        && let Some((bases, _)) = bases.split_once(')')
    {
        return bases.rsplit(',').next().is_some_and(|base| base.trim().ends_with("Enum"));
    }
    if let Some(rest) = head.strip_prefix("type ")
        && let Some((_, union)) = rest.split_once('=')
    {
        let start = signature.find(union).unwrap_or(0);
        let union = signature[start..].split(';').next().unwrap_or_default();
        let literals: Vec<&str> =
            union.split('|').map(str::trim).filter(|part| !part.is_empty()).collect();
        return !literals.is_empty()
            && literals.iter().all(|literal| {
                literal.len() >= 2
                    && ((literal.starts_with('"') && literal.ends_with('"'))
                        || (literal.starts_with('\'') && literal.ends_with('\'')))
            });
    }
    false
}

// The slice's name less the stem and the hyphen after it where it leads
// with them, else the name whole, as a module name.
fn part_of(slice: &str, stem: &str) -> String {
    let rest = slice
        .strip_prefix(stem)
        .and_then(|rest| rest.strip_prefix('-'))
        .filter(|rest| !rest.is_empty())
        .unwrap_or(slice);
    snake(rest)
}

// The directory under `crates/` a stem's crate lives at: `common-stem` for
// `common`, since `crates/common/` is the shared crate, and `<stem>`
// otherwise.
pub fn crate_dir(stem: &str) -> &str {
    if stem == "common" { "common-stem" } else { stem }
}

// A stem named as a crate the root manifest pins, or as `common` or `guest`,
// takes the package name `<stem>-stem`, so the names never collide.
pub fn package(stem: &str) -> String {
    if stem == "common" || stem == "guest" || pinned(stem) {
        format!("{stem}-stem")
    } else {
        stem.to_owned()
    }
}

fn pinned(name: &str) -> bool {
    let Some(manifest) = omnia_template::entry("Cargo.toml") else { return false };
    let mut in_pins = false;
    for line in manifest.body.lines() {
        if let Some(header) = line.strip_prefix('[') {
            in_pins = header.starts_with("workspace.dependencies");
        } else if in_pins && line.split_once(" = ").is_some_and(|(key, _)| key.trim() == name) {
            return true;
        }
    }
    false
}

fn tail(subject: &str) -> &str {
    subject.split_once('.').map_or("", |(_, tail)| tail)
}

fn stated_of(group: &[&Requirement]) -> Vec<Stated> {
    group.iter().map(|requirement| Stated::of(requirement)).collect()
}

// `requirement` under `key`'s group, in first-occurrence order.
fn group<'r, K: PartialEq>(
    groups: &mut Vec<(K, Vec<&'r Requirement>)>, key: K, requirement: &'r Requirement,
) {
    if let Some((_, group)) = groups.iter_mut().find(|(known, _)| *known == key) {
        group.push(requirement);
    } else {
        groups.push((key, vec![requirement]));
    }
}

// `REQ-001 orders.read, REQ-002 orders.refuse`, as a marker names the
// requirements it stands for.
fn named(stated: &[Stated]) -> String {
    stated
        .iter()
        .map(|stated| format!("{} {}", stated.id, stated.subject))
        .collect::<Vec<_>>()
        .join(", ")
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

// The subject tail the group's requirements share, else the first's, else
// `fallback` (the route or the topic), as a function name.
fn handler_base(group: &[&Requirement], fallback: &str) -> String {
    let tails: Vec<Vec<&str>> = group
        .iter()
        .map(|requirement| {
            tail(&requirement.subject).split('.').filter(|segment| !segment.is_empty()).collect()
        })
        .collect();
    let shared = shared_lead(&tails);
    let segments =
        if shared.is_empty() { tails.first().cloned().unwrap_or_default() } else { shared };
    if segments.is_empty() {
        return snake(fallback);
    }
    snake(&segments.join(" "))
}

// Whether a scenario of the group's requirements states an HTTP status
// other than `200`: a word of three digits in the status range in an
// outcome line, `503` in `the status is 503 and the body says degraded`.
fn states_status(group: &[&Requirement]) -> bool {
    group
        .iter()
        .flat_map(|requirement| &requirement.scenarios)
        .flat_map(|scenario| std::iter::once(&scenario.then).chain(&scenario.and))
        .flat_map(|line| line.split(|ch: char| !ch.is_ascii_alphanumeric()))
        .any(|word| {
            word.len() == 3
                && word != "200"
                && word.starts_with(|ch| matches!(ch, '1'..='5'))
                && word.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn shared_lead<'a>(tails: &[Vec<&'a str>]) -> Vec<&'a str> {
    let Some(first) = tails.first() else { return Vec::new() };
    let shared = tails[1..].iter().fold(first.len(), |shared, tail| {
        shared.min(first.iter().zip(tail).take_while(|(a, b)| a == b).count())
    });
    first[..shared].to_vec()
}

fn create(root: &Path, path: &str, text: &str) -> Result<(), Error> {
    let file = root.join(path);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)
            .map_err(|err| server_error!("creating `{}`: {err}", dir.display()))?;
    }
    let mut created = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)
        .map_err(|err| server_error!("creating `{path}`: {err}"))?;
    created.write_all(text.as_bytes()).map_err(|err| server_error!("writing `{path}`: {err}"))
}
