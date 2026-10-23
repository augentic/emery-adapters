//! One stem's share of a slice: its files, held or to scaffold, the items its part declares, and its lines in the lists.

use std::path::Path;

use emery_sdk::target::{DesignType, Marker, Requirement, Scenario, Slice};
use emery_sdk::{Error, NoFollow, Route, Standing, bad_request, server_error, standing};

use super::lists::{Line, Lists, Place, Topics};
use super::names::{Names, flat, number, pascal, screaming, snake};
use super::render::{self, dependency_line, topic_constant};
use super::trigger;
use super::types::{self, DesignKey};
use crate::layout::{self, TOPICS, crate_dir, package};

pub(super) struct Stem {
    pub(super) name: String,
    pub(super) dir: String,
    pub(super) package: String,
    pub(super) part: String,
    pub(super) doc: Vec<String>,
    // Whether the slice's words select the probe idiom, so a `GET` handler's
    // route function is pre-rendered for the probe's status.
    pub(super) probing: bool,
    // Whether the manifest names the SQL crate: the slice probes, whose
    // tests arrange the store's rows, or reaches a SQL store, whose
    // `TableStore` the part queries through.
    pub(super) sql: bool,
    pub(super) manifest: File,
    pub(super) entry: File,
    pub(super) module: File,
    pub(super) tests: Option<File>,
    pub(super) paths: Vec<(String, String)>,
    pub(super) handlers: Vec<Handler>,
    pub(super) consumers: Vec<Consumer>,
    pub(super) functions: Vec<Function>,
    pub(super) types: Vec<Type>,
    pub(super) scenarios: Vec<Test>,
    pub(super) lines: Vec<Line>,
}

#[derive(Clone, Copy)]
pub(super) enum Kind {
    Manifest,
    Entry,
    Module,
    Tests,
}

impl Kind {
    pub(super) const ALL: [Self; 4] = [Self::Manifest, Self::Entry, Self::Module, Self::Tests];
}

pub(super) struct File {
    pub(super) path: String,
    pub(super) held: bool,
    // What the tree holds at the path, where it holds a file.
    text: String,
}

pub(super) struct Handler {
    pub(super) name: String,
    pub(super) input: String,
    pub(super) output: String,
    pub(super) constant: String,
    pub(super) route: Route,
    pub(super) params: Vec<Param>,
    pub(super) stated: Vec<Stated>,
    pub(super) marker: String,
    // The route function over `handle_with` scaffolded beside a `POST`
    // handler, beside one whose scenarios state a status other than `200`,
    // and beside a probe's `GET`, so another status is one token's edit; its
    // decoder reads what the SDK's route fn for the method reads, the body
    // for `POST`, the query for `GET` and `DELETE`.
    pub(super) route_fn: Option<String>,
}

pub(super) struct Consumer {
    pub(super) name: String,
    pub(super) input: String,
    pub(super) constant: String,
    pub(super) topic: String,
    pub(super) declared: bool,
    pub(super) stated: Vec<Stated>,
    pub(super) marker: String,
}

pub(super) struct Param {
    pub(super) wire: String,
    pub(super) field: String,
}

pub(super) struct Stated {
    pub(super) id: String,
    pub(super) subject: String,
    pub(super) statement: String,
}

pub(super) struct Function {
    pub(super) name: String,
    pub(super) stated: Stated,
    pub(super) marker: String,
}

pub(super) struct Type {
    pub(super) name: String,
    pub(super) key: String,
    pub(super) signature: String,
    pub(super) body: types::Body,
}

pub(super) struct Test {
    pub(super) name: String,
    pub(super) id: String,
    pub(super) subject: String,
    pub(super) scenario: Scenario,
    pub(super) ignore: String,
    pub(super) marker: String,
    pub(super) act: Act,
}

// What a scenario's test acts through: the route or the topic its
// requirement carries, or the provider alone for a function.
#[derive(Clone)]
pub(super) enum Act {
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
                "routes",
                "topics",
                "get",
                "post",
                "put",
                "patch",
                "delete",
                "handle_with",
                "decode_json",
                "decode_query",
                "unescape",
            ]),
            types: Names::taken(&[
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
            tests: Names::taken(&["provider", "send", "deliver"]),
        }
    }
}

impl Stem {
    pub(super) fn new(name: &str, slice: &Slice, probing: bool, sql: bool) -> Self {
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
            probing,
            sql,
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
            lines: Vec::new(),
        }
    }

    // Names every item the part declares, the design types first so one
    // keeps its name over a handler's wrapper, and lays each type once
    // every one is named, so a field naming another the slice owns reads
    // its reserved name.
    pub(super) fn plan(
        &mut self, owned: &[&DesignType], requirements: &[&Requirement], design_keys: &[String],
        topics: &mut Topics,
    ) {
        let mut namespaces = Namespaces::default();
        let parsed: Vec<types::Parsed> =
            owned.iter().map(|design_type| types::read(&design_type.signature)).collect();
        let names: Vec<String> = owned
            .iter()
            .zip(&parsed)
            .map(|(design_type, parsed)| {
                let key = DesignKey::parse(&design_type.key);
                let declared = parsed.name.as_deref().unwrap_or(key.name);
                let suffix = key.stem.map_or_else(|| "2".to_owned(), pascal);
                namespaces.types.reserve(&pascal(declared), &suffix)
            })
            .collect();
        let bodies = types::lay(&parsed, &names, design_keys, &mut namespaces.values);
        for ((design_type, name), body) in owned.iter().zip(names).zip(bodies) {
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
            self.handler(route, &group, &mut namespaces);
        }
        for (topic, group) in consumed {
            self.consumer(topic, &group, &mut namespaces, topics);
        }

        if !self.scenarios.is_empty() {
            self.tests = Some(File::at(format!("crates/{}/tests/{}.rs", self.dir, self.part)));
        }
    }

    // One handler for the requirements served at `route`, over the path's
    // one constant; its route function where the method is `POST`, a
    // scenario states a status, or the slice probes and the method is
    // `GET`, since a probe's status follows the outcome.
    fn handler(&mut self, route: &Route, group: &[&Requirement], namespaces: &mut Namespaces) {
        let suffix = number(&group[0].id);
        let base = handler_base(group, &route.to_string());
        let name = namespaces.values.reserve(&base, &format!("_{suffix}"));
        let title = pascal(&name);
        let constant =
            if let Some((constant, _)) = self.paths.iter().find(|(_, path)| *path == route.path) {
                constant.clone()
            } else {
                let constant =
                    namespaces.constants.reserve(&screaming(&route.path), &format!("_{suffix}"));
                self.paths.push((constant.clone(), route.path.clone()));
                constant
            };
        let encoded = route.method == "POST"
            || states_status(group)
            || (self.probing && route.method == "GET");
        let route_fn = encoded
            .then(|| namespaces.values.reserve(&format!("{name}_route"), &format!("_{suffix}")));
        let stated = stated_of(group);
        self.handlers.push(Handler {
            input: namespaces.types.reserve(&format!("{title}Input"), &suffix),
            output: namespaces.types.reserve(&format!("{title}Output"), &suffix),
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

    pub(super) fn read(&mut self, root: &Path) -> Result<(), Error> {
        let files = [&mut self.manifest, &mut self.entry, &mut self.module];
        for file in files.into_iter().chain(self.tests.as_mut()) {
            file.read(root)?;
        }
        Ok(())
    }

    // Decides each of the stem's lines against its list, the held entry and
    // manifest read once with the stem's files.
    pub(super) fn place_lines(&mut self, root: &Path, lists: &mut Lists) -> Result<(), Error> {
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

    pub(super) const fn file(&self, kind: Kind) -> Option<&File> {
        match kind {
            Kind::Manifest => Some(&self.manifest),
            Kind::Entry => Some(&self.entry),
            Kind::Module => Some(&self.module),
            Kind::Tests => self.tests.as_ref(),
        }
    }

    // The file's text as the tree holds it after the scaffold: what was
    // written, or what was held.
    pub(super) fn text(&self, kind: Kind) -> Option<String> {
        let file = self.file(kind)?;
        Some(if file.held { file.text.clone() } else { self.render(kind) })
    }

    // What a whole write of the file must keep: a module its requirement
    // ids and the items the scaffold named, a test file its ids and its
    // tests, and a manifest or an entry every line it holds, so a list is
    // added to and never cut.
    pub(super) fn keeps(&self, kind: Kind) -> Vec<String> {
        match kind {
            Kind::Module => self.module_tokens(),
            Kind::Tests => self.test_tokens(),
            Kind::Manifest | Kind::Entry => lines_of(&self.text(kind).unwrap_or_default()),
        }
    }

    // The markers the file holds under the keys the brief quotes, each
    // standing for the block the scaffold laid; a manifest or an entry
    // holds none.
    pub(super) fn markers(&self, kind: Kind) -> Vec<Marker> {
        match kind {
            Kind::Module => self.module_markers(),
            Kind::Tests => self.test_markers(),
            Kind::Manifest | Kind::Entry => Vec::new(),
        }
    }

    // Each requirement's id, and each item the scaffold named as its
    // declaration spells it, `fn read`, `struct ReadInput`, `const
    // ORDERS_ID`, so the entry's calls and the tests' names still resolve;
    // a held part is an earlier slice's, so nothing of it is the
    // scaffold's to hold.
    fn module_tokens(&self) -> Vec<String> {
        if self.module.held {
            return Vec::new();
        }
        let mut tokens = Vec::new();
        for handler in &self.handlers {
            tokens.extend(handler.stated.iter().map(|stated| stated.id.clone()));
            tokens.push(format!("fn {}", handler.name));
            tokens.push(format!("struct {}", handler.input));
            tokens.push(format!("struct {}", handler.output));
            tokens.push(format!("const {}", handler.constant));
            if let Some(route_fn) = &handler.route_fn {
                tokens.push(format!("fn {route_fn}"));
            }
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
        if !self.handlers.is_empty() {
            tokens.push("fn routes".to_owned());
        }
        if !self.consumers.is_empty() {
            tokens.push("fn topics".to_owned());
        }
        tokens
    }

    // Each scenario's requirement id and its test's name, which the check
    // looks the scenario up by.
    fn test_tokens(&self) -> Vec<String> {
        if self.tests.as_ref().is_none_or(|file| file.held) {
            return Vec::new();
        }
        self.scenarios
            .iter()
            .flat_map(|test| [test.id.clone(), format!("fn {}", test.name)])
            .collect()
    }

    // A design type's key for its empty marker, a handler's or a consumer's
    // marker for its body, a function's for its signature and body; a held
    // part is an earlier slice's, so none.
    fn module_markers(&self) -> Vec<Marker> {
        if self.module.held {
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
        for marker in self
            .handlers
            .iter()
            .map(|handler| &handler.marker)
            .chain(self.consumers.iter().map(|consumer| &consumer.marker))
        {
            markers.push(Marker::new(marker, render::stub(marker).trim_end()));
        }
        for function in &self.functions {
            markers.push(Marker::new(&function.marker, function.block()));
        }
        markers
    }

    // Each scenario's test block under its `#[ignore]` text and under its
    // marker text, so either names the block.
    fn test_markers(&self) -> Vec<Marker> {
        if self.tests.as_ref().is_none_or(|file| file.held) {
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
    // the slice probes or reaches a SQL store, the SQL crate at the end of
    // the manifest, and, where the tree holds its entry, the part's lines
    // in the entry's.
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
                layout::ROUTES,
                format!("    let router = router.merge({ident}::router());"),
                Place::Before(layout::ROUTE_LIST_ANCHOR),
            ),
            Line::new(
                layout::MESSAGING,
                format!("    let router = {ident}::topics(router, &env);"),
                Place::Before(layout::TOPIC_LIST_ANCHOR),
            ),
        ];
        for consumer in self.consumers.iter().filter(|consumer| !consumer.declared) {
            lines.push(Line::new(
                TOPICS,
                format!("\n{}", topic_constant(&consumer.constant, &consumer.topic)),
                Place::FileEnd,
            ));
        }
        if self.manifest.held && self.sql {
            lines.push(Line::new(
                &self.manifest.path,
                dependency_line(layout::PROBE_DEPENDENCY),
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
}

impl File {
    const fn at(path: String) -> Self {
        Self {
            path,
            held: false,
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
    pub(super) const fn origin(&self) -> &'static str {
        if self.held { "held" } else { "scaffolded" }
    }
}

impl Type {
    // `struct Order` or `enum Status`, as the marker declares it.
    pub(super) fn declaration(&self) -> String {
        let keyword = if self.body.is_enum() { "enum" } else { "struct" };
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
pub(super) fn lines_of(text: &str) -> Vec<String> {
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

fn shared_lead<'a>(tails: &[Vec<&'a str>]) -> Vec<&'a str> {
    let Some(first) = tails.first() else { return Vec::new() };
    let shared = tails[1..].iter().fold(first.len(), |shared, tail| {
        shared.min(first.iter().zip(tail).take_while(|(a, b)| a == b).count())
    });
    first[..shared].to_vec()
}
