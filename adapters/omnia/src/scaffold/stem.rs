//! One stem's share of a slice: its files, held or to scaffold, the items its part declares, and its lines in the lists.

use std::collections::BTreeSet;
use std::path::Path;

use emery_sdk::target::{DesignType, Marker, Requirement, Scenario, Slice};
use emery_sdk::{Error, NoFollow, Route, Standing, bad_request, server_error, standing};

use super::gate::{Gated, Gating};
use super::lists::{Line, Lists, Place, Topics};
use super::names::{Names, flat, number, pascal, screaming, snake};
use super::reference::Gate;
use super::render::{self, dependency_line, topic_constant};
use super::settings::Settings;
use super::trigger;
use super::types::{self, DesignKey};
use crate::layout::{
    self, CRATE_LIST_ANCHOR, ENTRY, HANDLER_LIST_ANCHOR, TOPIC_DEPENDENCY, TOPICS, crate_dir,
    package,
};
use crate::syntax;

pub struct Stem {
    pub name: String,
    pub dir: String,
    pub package: String,
    pub part: String,
    pub doc: Vec<String>,
    // The slice's brief whole, where a setting the part reads may be
    // spelled beside the requirements.
    brief: String,
    // Whether the slice is a later serial slice (`index > 1`) of its part,
    // extending a held module and test file in place.
    serial: bool,
    pub manifest: File,
    pub entry: File,
    pub module: File,
    pub tests: Option<File>,
    pub paths: Vec<(String, String)>,
    pub settings: Option<Settings>,
    // The gate the stem's handlers under its prefix call, where the design
    // spells one and a handler falls under it.
    pub gating: Option<Gating>,
    pub handlers: Vec<Handler>,
    pub consumers: Vec<Consumer>,
    pub functions: Vec<Function>,
    pub types: Vec<Type>,
    // The data files of the tables among the types, each read by a fn of
    // the module.
    pub data: Vec<Data>,
    pub scenarios: Vec<Test>,
    pub lines: Vec<Line>,
    // Registrations in a held module's `handlers()` whose chain could not
    // be located by syntax, left for the turn to wire.
    pub unanchored_registrations: Vec<String>,
    // The top-level function names already declared in the held module and
    // test file before this slice's scaffold extends them.
    held_fns: BTreeSet<String>,
}

#[derive(Clone, Copy)]
pub enum Kind {
    Manifest,
    Entry,
    Module,
    Tests,
}

impl Kind {
    pub const ALL: [Self; 4] = [Self::Manifest, Self::Entry, Self::Module, Self::Tests];
}

pub struct File {
    pub path: String,
    pub held: bool,
    pub extended: bool,
    // What the tree holds at the path, where it holds a file.
    text: String,
}

pub struct Handler {
    pub name: String,
    // The type the body is read into, where the method carries one.
    pub input: Option<String>,
    pub output: String,
    pub constant: String,
    pub route: Route,
    pub params: Vec<Param>,
    // Whether a scenario states a status other than `200`, so the handler
    // answers its output under a status of its own.
    pub status: bool,
    // Whether the handler takes the request's headers and calls the gate
    // before anything else, its route falling under the gate's prefix.
    pub gated: bool,
    pub stated: Vec<Stated>,
    pub marker: String,
}

pub struct Consumer {
    pub name: String,
    pub input: String,
    pub constant: String,
    pub topic: String,
    pub declared: bool,
    pub stated: Vec<Stated>,
    pub marker: String,
}

pub struct Param {
    pub field: String,
}

pub struct Stated {
    pub id: String,
    pub subject: String,
    pub statement: String,
}

pub struct Function {
    pub name: String,
    pub stated: Stated,
    pub marker: String,
}

pub struct Type {
    pub name: String,
    pub key: String,
    pub signature: String,
    pub body: types::Body,
}

// A table's data file: its path from the tree's root and its rows as JSON.
pub struct Data {
    pub path: String,
    pub text: String,
    // The design key of the table the file holds, and the fn reading it.
    pub key: String,
    pub reader: String,
}

pub struct Test {
    pub name: String,
    pub id: String,
    pub subject: String,
    pub scenario: Scenario,
    pub ignore: String,
    pub marker: String,
    pub act: Act,
}

// What a scenario's test acts through: the route or the topic its
// requirement carries, or the provider alone for a function.
#[derive(Clone)]
pub enum Act {
    Route(Route),
    Topic(String),
    Call,
}

// The namespaces a part's items are named in, one per kind of item, the
// module's imports and its functions reserved.
struct Namespaces {
    values: Names,
    types: Names,
    constants: Names,
    tests: Names,
}

impl Default for Namespaces {
    fn default() -> Self {
        Self {
            values: Names::taken(&[
                "handlers", "get", "post", "put", "patch", "delete", "provider", "router", "call",
                "send", "deliver", "load",
            ]),
            types: Names::taken(&[
                "Capabilities",
                "Client",
                "Context",
                "DecodeError",
                "Delivery",
                "Deserialize",
                "Handlers",
                "IntoResponse",
                "Json",
                "MapConfig",
                "Path",
                "Provider",
                "Query",
                "Result",
                "Router",
                "Serialize",
                "Settings",
                "StatusCode",
                "Value",
            ]),
            constants: Names::default(),
            tests: Names::taken(&["provider", "router", "call", "send", "deliver"]),
        }
    }
}

impl Stem {
    pub fn new(name: &str, slice: &Slice) -> Self {
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
            brief: slice.brief.join("\n"),
            serial: slice.index > 1,
            manifest: File::at(format!("{root}/Cargo.toml")),
            entry: File::at(format!("{root}/src/lib.rs")),
            module: File::at(format!("{root}/src/{part}.rs")),
            tests: None,
            part,
            doc,
            paths: Vec::new(),
            settings: None,
            gating: None,
            handlers: Vec::new(),
            consumers: Vec::new(),
            functions: Vec::new(),
            types: Vec::new(),
            data: Vec::new(),
            scenarios: Vec::new(),
            lines: Vec::new(),
            unanchored_registrations: Vec::new(),
            held_fns: BTreeSet::new(),
        }
    }

    // Names every item the part declares, the design types first so one
    // keeps its name over a handler's wrapper, and lays each type once
    // every one is named, so a field naming another the slice owns reads
    // its reserved name; the settings last, once every constant the keys
    // are told from is named.
    pub fn plan(
        &mut self, owned: &[&DesignType], requirements: &[&Requirement], design: &types::Design,
        topics: &mut Topics,
    ) {
        let mut namespaces = Namespaces::default();
        let held_paths = if self.serial
            && self.module.held
            && let Ok(parsed) = syntax::parse(&self.module.text)
        {
            let declared = syntax::Declared::of(&parsed);
            self.held_fns.extend(declared.fns.iter().cloned());
            namespaces.values.extend(declared.fns);
            namespaces.types.extend(declared.types);
            namespaces.constants.extend(declared.constants);
            declared.paths
        } else {
            Vec::new()
        };
        if self.serial
            && let Some(tests) = self.tests.as_ref().filter(|file| file.held)
            && let Ok(parsed) = syntax::parse(&tests.text)
        {
            let declared = syntax::Declared::of(&parsed);
            self.held_fns.extend(declared.fns.iter().cloned());
            namespaces.tests.extend(declared.fns);
        }

        let parsed: Vec<types::Parsed> =
            owned.iter().map(|design_type| types::read(&design_type.signature)).collect();
        let names: Vec<String> = owned
            .iter()
            .zip(&parsed)
            .map(|(design_type, item)| {
                let key = DesignKey::parse(&design_type.key);
                let declared = item.name.as_deref().unwrap_or(key.name);
                if let types::Shape::Table(table) = &item.shape
                    && table.laid_over(&parsed)
                {
                    let suffix = key.stem.map_or_else(|| "2".to_owned(), snake);
                    return namespaces.values.reserve(&snake(declared), &format!("_{suffix}"));
                }
                let suffix = key.stem.map_or_else(|| "2".to_owned(), pascal);
                namespaces.types.reserve(&pascal(declared), &suffix)
            })
            .collect();
        let bodies = types::lay(&parsed, &names, design, &mut namespaces.values);
        for ((design_type, name), body) in owned.iter().zip(names).zip(bodies) {
            if let types::Body::Table { file, json, .. } = &body {
                self.data.push(Data {
                    path: format!("crates/{}/{file}", self.dir),
                    text: json.clone(),
                    key: design_type.key.clone(),
                    reader: name.clone(),
                });
            }
            self.types.push(Type {
                name,
                key: design_type.key.clone(),
                signature: design_type.signature.clone(),
                body,
            });
        }

        // the routed requirements grouped by route, the consumers by topic,
        // the rest a function each
        let mut routed: Vec<(&Route, Vec<&Requirement>)> = Vec::new();
        let mut consumed: Vec<(&str, Vec<&Requirement>)> = Vec::new();
        for requirement in requirements {
            let suffix = format!("_{}", number(&requirement.id));
            let act = if let Some(route) = trigger::route(requirement) {
                group(&mut routed, route, requirement);
                Act::Route(route.clone())
            } else if let Some(topic) = trigger::message(requirement) {
                group(&mut consumed, topic, requirement);
                Act::Topic(topic.to_owned())
            } else {
                let tail = tail(&requirement.subject);
                let base = if tail.is_empty() { snake(&self.name) } else { snake(tail) };
                self.functions.push(Function {
                    name: namespaces.values.reserve(&base, &suffix),
                    marker: format!("{} {}: the behaviour", requirement.id, requirement.subject),
                    stated: Stated::of(requirement),
                });
                Act::Call
            };
            for scenario in &requirement.scenarios {
                let name = flat(&scenario.name);
                self.scenarios.push(Test {
                    name: namespaces.tests.reserve(&snake(&name), &suffix),
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
            self.handler(route, &group, &held_paths, &mut namespaces);
        }
        for (topic, group) in consumed {
            self.consumer(topic, &group, &mut namespaces, topics);
        }
        self.settings(requirements, &mut namespaces, topics);
    }

    // One handler for the requirements served at `route`, over the path's
    // one constant; its input type where the method carries a body, and
    // its own status where a scenario states one.
    fn handler(
        &mut self, route: &Route, group: &[&Requirement], held_paths: &[(String, String)],
        namespaces: &mut Namespaces,
    ) {
        let suffix = number(&group[0].id);
        let base = handler_base(group, &route.to_string());
        let name = namespaces.values.reserve(&base, &format!("_{suffix}"));
        let title = pascal(&name);
        let constant = if let Some((constant, _)) =
            self.paths.iter().chain(held_paths).find(|(_, path)| *path == route.path)
        {
            constant.clone()
        } else {
            let constant =
                namespaces.constants.reserve(&screaming(&route.path), &format!("_{suffix}"));
            self.paths.push((constant.clone(), route.path.clone()));
            constant
        };
        let input = (trigger::carries_body(&route.method) && states_body(group))
            .then(|| namespaces.types.reserve(&format!("{title}Input"), &suffix));
        let stated = stated_of(group);
        self.handlers.push(Handler {
            input,
            output: namespaces.types.reserve(&format!("{title}Output"), &suffix),
            name,
            constant,
            params: route.params().map(|wire| Param { field: snake(wire) }).collect(),
            route: route.clone(),
            status: states_status(group),
            gated: false,
            marker: format!("{}: the handler", named(&stated)),
            stated,
        });
    }

    // Whether a handler of the stem falls under the gate's prefix.
    pub fn under(&self, gate: &Gate) -> bool {
        self.handlers.iter().any(|handler| gate.under(&handler.route.path))
    }

    // Resolves where the stem's handlers under the prefix reach the gate,
    // and marks each as calling it where the laid code can; a stem with no
    // handler under the prefix reaches none.
    pub fn gate(&mut self, gated: &Gated<'_>) {
        if !self.under(gated.gate) {
            return;
        }
        let held = self.held_fn(Gate::FN);
        let gating = Gating::resolve(gated, &self.dir, &self.part, &self.module.path, held);
        if let Some(gating) = &gating
            && gating.calls()
        {
            for handler in &mut self.handlers {
                handler.gated = gating.gate.under(&handler.route.path);
            }
        }
        self.gating = gating;
    }

    // Whether the request a test sends at `route` carries the gate's
    // header: what the handler laid for the route takes.
    pub fn gates(&self, route: &Route) -> bool {
        self.handlers.iter().any(|handler| handler.route == *route && handler.gated)
    }

    // The gate the stem lays in its module, where it lays one.
    pub fn laid_gate(&self) -> Option<&Gate> {
        self.gating.as_ref().filter(|gating| gating.lays()).map(|gating| &gating.gate)
    }

    // The keys the stem's tests seed: the part's settings and, where its
    // handlers call the gate, the gate's.
    pub fn seeded(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.settings.iter().flat_map(Settings::keys).collect();
        if let Some(gating) = self.gating.as_ref().filter(|gating| gating.calls()) {
            keys.extend(gating.gate.settings.iter().flat_map(Settings::keys));
        }
        keys
    }

    // Whether the request a test sends at `route` carries a body: what the
    // handler laid for the route takes, else what its method carries.
    pub fn sends_body(&self, route: &Route) -> bool {
        self.handlers
            .iter()
            .find(|handler| handler.route == *route)
            .map_or_else(|| trigger::carries_body(&route.method), |handler| handler.input.is_some())
    }

    // One consumer for the requirements that consume `topic`, under the
    // table's constant for it.
    fn consumer(
        &mut self, topic: &str, group: &[&Requirement], namespaces: &mut Namespaces,
        topics: &mut Topics,
    ) {
        let suffix = number(&group[0].id);
        let base = format!("on_{}", handler_base(group, topic));
        let name = namespaces.values.reserve(&base, &format!("_{suffix}"));
        let (constant, declared) = topics.constant(topic);
        let stated = stated_of(group);
        self.consumers.push(Consumer {
            input: namespaces.types.reserve(&format!("{}Input", pascal(&name)), &suffix),
            name,
            constant,
            topic: topic.to_owned(),
            declared,
            marker: format!("{}: the consumer", named(&stated)),
            stated,
        });
    }

    // The settings struct where the requirements, their scenarios, or the
    // brief spell a key: a part with a handler or a consumer to load it.
    fn settings(
        &mut self, requirements: &[&Requirement], namespaces: &mut Namespaces, topics: &Topics,
    ) {
        if self.handlers.is_empty() && self.consumers.is_empty() {
            return;
        }
        let texts = requirements
            .iter()
            .flat_map(|requirement| {
                requirement.statement.iter().map(String::as_str).chain(
                    requirement.scenarios.iter().flat_map(|scenario| {
                        std::iter::once(scenario.name.as_str())
                            .chain(scenario.given.iter().map(String::as_str))
                            .chain([scenario.when.as_str(), scenario.then.as_str()])
                            .chain(scenario.and.iter().map(String::as_str))
                    }),
                )
            })
            .chain([self.brief.as_str()]);
        let settings = Settings::spelled(format!("{}Settings", pascal(&self.part)), texts, |key| {
            topics.is_named(key) || namespaces.constants.contains(key)
        });
        if let Some(mut settings) = settings {
            settings.name = namespaces.types.reserve(&settings.name, "2");
            self.settings = Some(settings);
        }
    }

    pub fn read(&mut self, root: &Path, requirements: &[&Requirement]) -> Result<(), Error> {
        if requirements.iter().any(|requirement| !requirement.scenarios.is_empty()) {
            self.tests = Some(File::at(format!("crates/{}/tests/{}.rs", self.dir, self.part)));
        }
        let files = [&mut self.manifest, &mut self.entry, &mut self.module];
        for file in files.into_iter().chain(self.tests.as_mut()) {
            file.read(root)?;
        }
        Ok(())
    }

    // Extends the held module and test file in place for a later serial
    // slice of a part (`index > 1`): appends the new constants, functions,
    // handlers, consumers, and tests, and registers new routes and topics in
    // the held `handlers()` chain where `syn` locates it.
    pub fn extend(&mut self) {
        if !self.serial {
            return;
        }
        if self.module.held {
            let (next, unanchored) = self.extend_module(&self.module.text);
            self.module.extend(next);
            self.unanchored_registrations = unanchored;
        }
        if let Some(tests) = self.tests.as_ref().filter(|file| file.held) {
            let next = self.extend_tests(&tests.text);
            if let Some(tests) = &mut self.tests {
                tests.extend(next);
            }
        }
    }

    // Decides each of the stem's lines against its list, the held entry and
    // manifest read once with the stem's files.
    pub fn place_lines(&mut self, root: &Path, lists: &mut Lists) -> Result<(), Error> {
        for file in [&self.entry, &self.manifest].into_iter().filter(|file| file.held) {
            lists.cache(&file.path, &file.text);
        }
        let mut lines = self.lines();
        for line in &mut lines {
            line.place(root, lists)?;
        }
        self.lines = lines;
        Ok(())
    }

    pub const fn file(&self, kind: Kind) -> Option<&File> {
        match kind {
            Kind::Manifest => Some(&self.manifest),
            Kind::Entry => Some(&self.entry),
            Kind::Module => Some(&self.module),
            Kind::Tests => self.tests.as_ref(),
        }
    }

    // The file's text as the tree holds it after the scaffold: what was
    // written, or what was held.
    pub fn text(&self, kind: Kind) -> Option<String> {
        let file = self.file(kind)?;
        Some(if file.held { file.text.clone() } else { self.render(kind) })
    }

    // What a whole write of the file must keep: a module its requirement
    // ids and the items the scaffold named, a test file its ids and its
    // tests, and a manifest or an entry every line it holds, so a list is
    // added to and never cut.
    pub fn keeps(&self, kind: Kind) -> Vec<String> {
        match kind {
            Kind::Module => self.module_tokens(),
            Kind::Tests => self.test_tokens(),
            Kind::Manifest | Kind::Entry => lines_of(&self.text(kind).unwrap_or_default()),
        }
    }

    // The markers the file holds under the keys the brief quotes, each
    // standing for the block the scaffold laid; a manifest or an entry
    // holds none.
    pub fn markers(&self, kind: Kind) -> Vec<Marker> {
        match kind {
            Kind::Module => self.module_markers(),
            Kind::Tests => self.test_markers(),
            Kind::Manifest | Kind::Entry => Vec::new(),
        }
    }

    // Whether the part registers anything: a handler or a consumer gives it
    // a `handlers()`, and the stem's entry a line for it.
    pub const fn registers(&self) -> bool {
        !self.handlers.is_empty() || !self.consumers.is_empty()
    }

    // Each requirement's id, and each item the scaffold named as its
    // declaration spells it, `fn read`, `struct ReadOutput`, `const
    // ORDERS_ID`, so the entry's calls and the tests' names still resolve;
    // a held part that was not extended is an earlier slice's alone, so
    // nothing of it is the scaffold's to hold.
    fn module_tokens(&self) -> Vec<String> {
        if !self.module.laid() {
            return Vec::new();
        }
        let mut tokens = Vec::new();
        for handler in &self.handlers {
            tokens.extend(handler.stated.iter().map(|stated| stated.id.clone()));
            tokens.push(format!("fn {}", handler.name));
            if let Some(input) = &handler.input {
                tokens.push(format!("struct {input}"));
            }
            tokens.push(format!("struct {}", handler.output));
            tokens.push(format!("const {}", handler.constant));
        }
        for consumer in &self.consumers {
            tokens.extend(consumer.stated.iter().map(|stated| stated.id.clone()));
            tokens.push(format!("fn {}", consumer.name));
            tokens.push(format!("struct {}", consumer.input));
        }
        for function in &self.functions {
            tokens.push(function.stated.id.clone());
            tokens.push(format!("fn {}", function.name));
        }
        tokens.extend(self.types.iter().map(Type::declaration));
        if let Some(settings) = &self.settings {
            tokens.push(format!("struct {}", settings.name));
        }
        if let Some(gate) = self.laid_gate() {
            tokens.push(format!("fn {}", Gate::FN));
            tokens.push(format!("enum {}", Gate::ERROR));
            if let Some(settings) = &gate.settings {
                tokens.push(format!("struct {}", settings.name));
            }
        }
        if self.registers() {
            tokens.push("fn handlers".to_owned());
        }
        tokens
    }

    // Each scenario's requirement id and its test's name, which the check
    // looks the scenario up by.
    fn test_tokens(&self) -> Vec<String> {
        if !self.tests.as_ref().is_some_and(File::laid) {
            return Vec::new();
        }
        self.scenarios
            .iter()
            .flat_map(|test| [test.id.clone(), format!("fn {}", test.name)])
            .collect()
    }

    // A design type's key for its empty marker, a handler's or a consumer's
    // marker for its body, a function's for its signature and body; a held
    // part that was not extended is an earlier slice's alone, so none.
    fn module_markers(&self) -> Vec<Marker> {
        if !self.module.laid() {
            return Vec::new();
        }
        let mut markers: Vec<Marker> = self
            .types
            .iter()
            .filter(|design_type| design_type.body.is_marker())
            .map(|design_type| {
                Marker::new(&design_type.key, format!("pub {} {{}}", design_type.declaration()))
            })
            .collect();
        if let Some(gate) = self.laid_gate() {
            markers.push(Marker::new(gate.marker(), render::gate_stub(gate).trim_end()));
        }
        for handler in &self.handlers {
            markers.push(Marker::new(&handler.marker, handler.stub().trim_end()));
        }
        for consumer in &self.consumers {
            markers.push(Marker::new(&consumer.marker, consumer.stub().trim_end()));
        }
        for function in &self.functions {
            markers.push(Marker::new(&function.marker, function.block()));
        }
        markers
    }

    // Each scenario's test block under its `#[ignore]` text and under its
    // marker text, so either names the block.
    fn test_markers(&self) -> Vec<Marker> {
        if !self.tests.as_ref().is_some_and(File::laid) {
            return Vec::new();
        }
        self.scenarios
            .iter()
            .flat_map(|test| {
                let block = test.block(self);
                [Marker::new(&test.ignore, block.clone()), Marker::new(&test.marker, block)]
            })
            .collect()
    }

    // The stem's lines in the root lists, the constant of each topic the
    // slice declares in the table, where the tree holds its manifest and
    // the part names a topic, the table's crate at the end of the
    // manifest, and, where the tree holds its entry, the part's lines in
    // the entry's.
    fn lines(&self) -> Vec<Line> {
        let ident = self.package.replace('-', "_");
        let mut lines = vec![
            Line::new(
                "Cargo.toml",
                format!("{} = {{ path = \"crates/{}\" }}", self.package, self.dir),
                Place::After(CRATE_LIST_ANCHOR),
            ),
            Line::new(ENTRY, format!("{ident}::handlers()"), Place::After(HANDLER_LIST_ANCHOR)),
        ];
        for consumer in self.consumers.iter().filter(|consumer| !consumer.declared) {
            lines.push(Line::new(
                TOPICS,
                format!("\n{}", topic_constant(&consumer.constant, &consumer.topic)),
                Place::FileEnd,
            ));
        }
        if self.manifest.held {
            for dependency in self.dependencies() {
                lines.push(Line::new(&self.manifest.path, dependency, Place::FileEnd));
            }
        }
        if self.entry.held {
            lines.push(Line::new(
                &self.entry.path,
                format!("pub mod {};", self.part),
                Place::PubModList,
            ));
            if self.registers() {
                lines.push(Line::new(
                    &self.entry.path,
                    format!("{}::handlers()", self.part),
                    Place::After(HANDLER_LIST_ANCHOR),
                ));
            }
        }
        lines
    }

    pub fn held_fn(&self, name: &str) -> bool {
        self.held_fns.contains(name)
    }

    // The lines of the stem's `[dependencies]` beyond the fixed four: the
    // topic table's crate where a part names a topic, the error crates
    // where the stem lays the gate, and the gate's crate where it imports
    // the gate from another.
    pub fn dependencies(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.consumers.is_empty() {
            lines.push(dependency_line(TOPIC_DEPENDENCY));
        }
        if let Some(gating) = &self.gating {
            if gating.lays() {
                lines.extend(GATE_DEPENDENCIES.iter().map(|name| dependency_line(name)));
            }
            if let Some(dependency) = gating.dependency() {
                lines.push(dependency.line());
            }
        }
        lines
    }
}

// The crates the laid gate's error type uses beyond the fixed four, each
// pinned by the root manifest.
const GATE_DEPENDENCIES: [&str; 2] = ["anyhow", "thiserror"];

impl File {
    const fn at(path: String) -> Self {
        Self {
            path,
            held: false,
            extended: false,
            text: String::new(),
        }
    }

    // Reads what stands at the path, following no link on the way: a link
    // or a directory there, or on the way, is refused before any write.
    fn read(&mut self, root: &Path) -> Result<(), Error> {
        self.held = match standing(root, &self.path) {
            Ok(Standing::File) => {
                self.text = layout::read(&root.join(&self.path))?;
                true
            }
            Ok(Standing::Absent) => false,
            Err(NoFollow::Io { path, source }) => {
                return Err(server_error!("reading `{path}`: {source}"));
            }
            Err(err) => {
                return Err(bad_request!(
                    "`{}` {err}, where the slice's scaffold writes a file",
                    self.path
                ));
            }
        };
        Ok(())
    }

    // Where the file came from, as the brief names it.
    pub const fn origin(&self) -> &'static str {
        if self.extended {
            "held, extended"
        } else if self.held {
            "held"
        } else {
            "scaffolded"
        }
    }

    // Whether the scaffold laid this slice's items into the file: a fresh
    // file it created, or a held one it extended in place.
    pub const fn laid(&self) -> bool {
        !self.held || self.extended
    }

    fn extend(&mut self, next: String) {
        if next != self.text {
            self.text = next;
            self.extended = true;
        }
    }
}

impl Handler {
    // The names the body borrows as laid, the context first, then each
    // path parameter and the input, so the lints stay quiet.
    pub fn borrowed(&self) -> Vec<String> {
        let mut names = vec!["context".to_owned()];
        names.extend(self.params.iter().map(|param| param.field.clone()));
        names.extend(self.input.as_ref().map(|_| "input".to_owned()));
        names
    }

    // The body as the file holds it: the borrow and the marker, the one
    // block the turn replaces.
    pub fn stub(&self) -> String {
        render::stub(&self.borrowed(), &self.marker)
    }
}

impl Consumer {
    pub fn stub(&self) -> String {
        render::stub(&["context".to_owned(), "input".to_owned()], &self.marker)
    }
}

impl Type {
    // `struct Order` or `enum Status`, as the marker declares it, or `fn
    // zones` for a table's reader.
    pub fn declaration(&self) -> String {
        let keyword = if self.body.is_table() {
            "fn"
        } else if self.body.is_enum() {
            "enum"
        } else {
            "struct"
        };
        format!("{keyword} {}", self.name)
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

// Every non-blank line of `text`, trimmed, as the tokens a whole write of a
// list or a fixed file must keep.
pub fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|line| !line.is_empty()).map(ToOwned::to_owned).collect()
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

// The words a requirement or a scenario spells a request body with.
const BODY_WORDS: &[&str] = &["body", "bodies", "payload", "payloads"];

// Whether the group's requirements state a request body: a body word in a
// statement or a scenario line, so a `POST` whose text states none takes
// no `Json` extractor, which axum would refuse a bodiless request through.
fn states_body(group: &[&Requirement]) -> bool {
    group
        .iter()
        .flat_map(|requirement| {
            requirement.statement.iter().chain(requirement.scenarios.iter().flat_map(|scenario| {
                std::iter::once(&scenario.name)
                    .chain(&scenario.given)
                    .chain(std::iter::once(&scenario.when))
                    .chain(std::iter::once(&scenario.then))
                    .chain(&scenario.and)
            }))
        })
        .flat_map(|line| line.split(|ch: char| !ch.is_ascii_alphanumeric()))
        .any(|word| BODY_WORDS.iter().any(|body| body.eq_ignore_ascii_case(word)))
}

fn shared_lead<'a>(tails: &[Vec<&'a str>]) -> Vec<&'a str> {
    let Some(first) = tails.first() else { return Vec::new() };
    let shared = tails[1..].iter().fold(first.len(), |shared, tail| {
        shared.min(first.iter().zip(tail).take_while(|(a, b)| a == b).count())
    });
    first[..shared].to_vec()
}
