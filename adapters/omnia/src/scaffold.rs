//! Scaffolds a slice's skeleton before its turn: the stem's crate, the part, its tests, and its lines in every list.

mod names;
mod render;
mod route;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

use emery_sdk::target::{DesignType, Requirement, Scenario, Slice};
use emery_sdk::{Error, Route, bad_request, server_error};

use self::names::{Names, escape, flat, number, pascal, screaming, snake};
use self::render::Place;
use crate::tree;

const TOPIC_TABLE: &str = "crates/common/src/topic.rs";

pub struct Scaffold {
    stems: Vec<Stem>,
    start: Vec<String>,
    relaid: Vec<Relaid>,
    existing: BTreeSet<String>,
    lines: Vec<Line>,
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
}

struct Test {
    name: String,
    id: String,
    subject: String,
    scenario: Scenario,
    ignore: String,
    marker: String,
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

impl Default for Claims {
    fn default() -> Self {
        Self {
            values: Names::reserving(&[
                "routes", "topics", "get", "post", "put", "patch", "delete",
            ]),
            types: Names::reserving(&[
                "Capabilities",
                "Client",
                "Context",
                "Deserialize",
                "Serialize",
            ]),
            constants: Names::default(),
            tests: Names::default(),
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
        })
    }

    pub fn write(&self, root: &Path) -> Result<Vec<String>, Error> {
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
        Ok(written)
    }

    // Every file the scaffold wrote or found, and every list it adds a
    // line to, sorted: what the turn changes through `edit_files` alone.
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

    pub fn brief(&self) -> String {
        let mut text = String::new();
        for stem in &self.stems {
            stem.brief_files(&self.lines, &mut text);
            stem.brief_markers(&mut text);
        }
        if !self.start.is_empty() {
            let _ = writeln!(
                text,
                "The `start` stem ({}): the seed and the host are `start`, so nothing was \
                 scaffolded for it; cover what they satisfy and leave the rest out, as the \
                 guardrails' table says.\n",
                self.start.join(", ")
            );
        }
        for relaid in &self.relaid {
            let _ = writeln!(
                text,
                "{} (`{}`), stated under `start`, is laid under `crates/{}/`, the stem its route's \
                 resource names, since a stated route is its own stem's and never `start`'s.\n",
                relaid.id, relaid.route, relaid.stem
            );
        }
        text.trim_end().to_owned()
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
        let dir = format!("crates/{name}");
        Self {
            name: name.to_owned(),
            package: package(name),
            manifest: File::at(format!("{dir}/Cargo.toml")),
            entry: File::at(format!("{dir}/src/lib.rs")),
            module: File::at(format!("{dir}/src/{part}.rs")),
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
                signature: design_type.signature.clone(),
            });
        }

        // the routed requirements grouped by route, the consumers by topic,
        // the rest a function each
        let mut routed: Vec<(&Route, Vec<&Requirement>)> = Vec::new();
        let mut consumed: Vec<(&str, Vec<&Requirement>)> = Vec::new();
        for requirement in requirements {
            let suffix = format!("_{}", number(&requirement.id));
            if let Some(route) = route::of(requirement) {
                group(&mut routed, route, requirement);
            } else if let Some(topic) = requirement.topic.as_deref().filter(|t| !t.is_empty()) {
                group(&mut consumed, topic, requirement);
            } else {
                let tail = tail(&requirement.subject);
                let base = if tail.is_empty() { snake(&self.name) } else { snake(tail) };
                self.functions.push(Function {
                    name: claims.values.claim(&base, &suffix),
                    marker: format!("{} {}: the behaviour", requirement.id, requirement.subject),
                    stated: Stated::of(requirement),
                });
            }
            for scenario in &requirement.scenarios {
                let name = flat(&scenario.name);
                self.scenarios.push(Test {
                    name: claims.tests.claim(&snake(&name), &suffix),
                    id: requirement.id.clone(),
                    subject: requirement.subject.clone(),
                    scenario: scenario.clone(),
                    ignore: format!("{} {name}: unfilled", requirement.id),
                    marker: format!("{} {name}: arrange, act, assert", requirement.id),
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
            self.tests = Some(File::at(format!("crates/{}/tests/{}.rs", self.name, self.part)));
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

    // The stem's lines in the root lists, the constant of each topic the
    // slice declares in the table, and, where the tree holds its entry, the
    // part's lines in the entry's.
    fn lines(&self) -> Vec<Line> {
        let ident = self.package.replace('-', "_");
        let mut lines = vec![
            Line::new(
                "Cargo.toml",
                format!("{} = {{ path = \"crates/{}\" }}", self.package, self.name),
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
                format!(
                    "\n/// The `{}` topic.\npub const {}: &str = \"{}\";",
                    consumer.topic, consumer.constant, consumer.topic
                ),
                Place::FileEnd,
            ));
        }
        if self.entry.held {
            lines.push(Line::new(
                &self.entry.path,
                format!("pub mod {};", self.part),
                Place::AfterLastPubMod,
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

    fn brief_files(&self, lines: &[Line], text: &mut String) {
        let standing = |own: &Line| {
            lines
                .iter()
                .find(|line| line.path == own.path && line.text == own.text)
                .map_or(Standing::Unanchored, |line| line.standing)
        };
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
        let _ = writeln!(
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
        match &self.tests {
            Some(tests) if tests.held => {
                let _ = writeln!(text, "- `{}`: held. {HELD}", tests.path);
            }
            Some(tests) => {
                let _ = writeln!(
                    text,
                    "- `{}`: scaffolded, one ignored test per scenario. Fill each through \
                     `edit_files` and remove its `#[ignore = ..]` line.",
                    tests.path
                );
            }
            None => {
                text.push_str("- No scenario states a test, so no test file was scaffolded.\n");
            }
        }
        let own = self.lines();
        let added = own
            .iter()
            .filter(|line| !line.path.starts_with("crates/"))
            .any(|line| standing(line) == Standing::Add);
        let _ = writeln!(
            text,
            "- `Cargo.toml`, `src/routes.rs`, `src/messaging.rs`: the stem's lines {}.",
            if added { "added" } else { "already held" }
        );
        for consumer in &self.consumers {
            let _ = writeln!(
                text,
                "- `{TOPIC_TABLE}`: the constant `{}` for the topic `{}` {}.",
                consumer.constant,
                consumer.topic,
                if consumer.declared {
                    "already declared"
                } else {
                    "added at the end of the table"
                }
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

    fn brief_markers(&self, text: &mut String) {
        if self.module.held || self.tests.as_ref().is_some_and(|tests| tests.held) {
            text.push_str(
                "The markers below are the ones a fresh scaffold writes; a held file holds \
                 whatever stands in it.\n\n",
            );
        }
        text.push_str("Markers, in fill order:\n\n");
        let mut count = 0;
        for design_type in &self.types {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `pub struct {} {{}}` in `{}`: the design type `{}`. Declare its fields \
                 from the signature in its doc.",
                design_type.name, self.module.path, design_type.key
            );
        }
        for handler in &self.handlers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `todo!(\"{}\")` in `{}`: the handler `{}` of `{}`. Fill `{}` and `{}` \
                 with the wire's fields, write the body, and drop the `let _ = (input, \
                 context);` line.",
                escape(&handler.marker),
                self.module.path,
                handler.name,
                handler.route,
                handler.input,
                handler.output
            );
        }
        for consumer in &self.consumers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `todo!(\"{}\")` in `{}`: the consumer `{}` of the topic `{}`, registered \
                 in the part's `topics(..)` under `topic::{}`. Fill `{}` with the delivery's \
                 fields, write the body, and drop the `let _ = (input, context);` line.",
                escape(&consumer.marker),
                self.module.path,
                consumer.name,
                consumer.topic,
                consumer.constant,
                consumer.input
            );
        }
        for function in &self.functions {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `todo!(\"{}\")` in `{}`: the function `{}`, since the requirement \
                 states no route and no topic. Give it the signature the behaviour needs and \
                 write the body.",
                escape(&function.marker),
                self.module.path,
                function.name
            );
        }
        if let Some(tests) = &self.tests {
            for test in &self.scenarios {
                count += 1;
                let _ = writeln!(
                    text,
                    "{count}. `todo!(\"{}\")` in `{}`: the test `{}`. Fill it and remove the \
                     `#[ignore = \"{}\"]` line above it.",
                    escape(&test.marker),
                    tests.path,
                    test.name,
                    escape(&test.ignore)
                );
            }
        }
        if count == 0 {
            text.push_str("None: the slice states no requirement under this stem.\n");
        }
        text.push('\n');
    }
}

const HELD: &str =
    "The tree already held it, so nothing was written over it; read it for what stands.";

impl File {
    const fn at(path: String) -> Self {
        Self { path, held: false }
    }

    fn read(&mut self, root: &Path) -> Result<(), Error> {
        self.held = match fs::symlink_metadata(root.join(&self.path)) {
            Ok(meta) if meta.is_file() => true,
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

impl Stated {
    fn of(requirement: &Requirement) -> Self {
        Self {
            id: requirement.id.clone(),
            subject: requirement.subject.clone(),
            statement: requirement.statement.first().map(|line| flat(line)).unwrap_or_default(),
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
        let declared: Vec<(String, String)> = text.lines().filter_map(constant_of).collect();
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

// The name and the topic of a `pub const NAME: &str = "topic";` line.
fn constant_of(line: &str) -> Option<(String, String)> {
    let rest = line.trim().strip_prefix("pub const ")?;
    let (name, rest) = rest.split_once(':')?;
    let (_, value) = rest.split_once('=')?;
    let topic = value.trim().strip_suffix(';')?.trim().strip_prefix('"')?.strip_suffix('"')?;
    Some((name.trim().to_owned(), topic.to_owned()))
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

// A stem named as a crate the root manifest pins, or as `common` or `guest`,
// takes the package name `<stem>-stem`, so the names never collide.
fn package(stem: &str) -> String {
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
