//! Scaffolds a slice's skeleton before its turn: the stem's crate, the part, its tests, and its lines in every list.

mod brief;
mod lists;
mod names;
mod reference;
mod render;
mod trigger;
mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::target::{DesignType, Guarded, Marker, Requirement, Scenario, Slice};
use emery_sdk::{
    Error, NoFollow, Route, Standing, bad_request, push_unique, server_error, standing, unique,
    write_whole,
};

use self::lists::{Line, Lists, Place, Placement, Topics};
use self::names::{Names, flat, number, pascal, screaming, snake};
use self::reference::{Idiom, Role};
use self::render::{Call, Expr};
use self::types::DesignKey;
use crate::layout::{self, TOPICS, crate_dir, package};

// A slice's skeleton as read against the tree and not yet written: each
// stem's files, held or to scaffold, and each line placed in its list.
pub struct Plan {
    stems: Vec<Stem>,
    start: Vec<String>,
    relaid: Vec<Relaid>,
    existing: BTreeSet<String>,
    lists: Lists,
    roles: BTreeSet<Role>,
    idioms: BTreeSet<Idiom>,
}

// A slice's skeleton as written, which the turn fills and the check holds
// the tree to.
pub struct Scaffold {
    plan: Plan,
    written: Vec<String>,
    listing: BTreeSet<String>,
}

// What a check reads of one scenario the scaffold wrote a test for.
pub struct Expected<'a> {
    pub path: &'a str,
    pub id: &'a str,
    pub scenario: &'a str,
    pub test: &'a str,
}

// One stem's share of the slice before its files are planned.
struct StemPlan<'s> {
    stem: Stem,
    requirements: Vec<&'s Requirement>,
    owned: Vec<&'s DesignType>,
}

struct Stem {
    name: String,
    dir: String,
    package: String,
    part: String,
    doc: Vec<String>,
    // Whether the slice's words select the probe idiom, so a `GET` handler's
    // route function is pre-rendered for the probe's status.
    probing: bool,
    // Whether the manifest names the SQL crate: the slice probes, whose
    // tests arrange the store's rows, or reaches a SQL store, whose
    // `TableStore` the part queries through.
    sql: bool,
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
    lines: Vec<Line>,
}

#[derive(Clone, Copy)]
enum Kind {
    Manifest,
    Entry,
    Module,
    Tests,
}

impl Kind {
    const ALL: [Self; 4] = [Self::Manifest, Self::Entry, Self::Module, Self::Tests];
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
    // handler, beside one whose scenarios state a status other than `200`,
    // and beside a probe's `GET`, so another status is one token's edit.
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
    body: types::Body,
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

impl Scaffold {
    pub fn plan(slice: &Slice, root: &Path) -> Result<Plan, Error> {
        let existing: BTreeSet<String> = layout::crates(root)?.into_iter().collect();
        let mut topics = Topics::read(root)?;

        // each requirement to its stem: `start` set aside, unless a typed
        // route names the resource whose crate serves it
        let mut plans: Vec<StemPlan> = Vec::new();
        let mut start = Vec::new();
        let mut relaid = Vec::new();
        for requirement in &slice.requirements {
            let own = requirement.stem();
            let name = if layout::is_start_stem(own) {
                let resource = trigger::route(requirement)
                    .map(|route| (route, trigger::resource(route)))
                    .and_then(|(route, stem)| Some((route, stem?)))
                    .filter(|(_, stem)| !layout::is_start_stem(stem));
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
            match plans.iter_mut().find(|plan| plan.stem.name == name) {
                Some(plan) => plan.requirements.push(requirement),
                None => plans.push(StemPlan {
                    stem: Stem::new(&name, slice),
                    requirements: vec![requirement],
                    owned: Vec::new(),
                }),
            }
        }

        // each owned type to the stem its key leads with, else the first
        for design_type in &slice.types {
            let stem = DesignKey::parse(&design_type.key).stem;
            let at = plans
                .iter()
                .position(|plan| Some(plan.stem.name.as_str()) == stem)
                .or_else(|| (!plans.is_empty()).then_some(0));
            if let Some(at) = at {
                plans[at].owned.push(design_type);
            }
        }

        let design_keys = types::design_keys(&slice.design);
        let idioms = reference::idioms(slice);
        let roles = reference::roles(slice);
        let probing = idioms.contains(&Idiom::Probe);
        let sql = probing || roles.contains(&Role::Table);
        let mut lists = Lists::default();
        let mut stems = Vec::new();
        for plan in plans {
            let mut stem = plan.stem;
            stem.probing = probing;
            stem.sql = sql;
            stem.plan(&plan.owned, &plan.requirements, &design_keys, &mut topics);
            stem.read(root)?;
            stem.place_lines(root, &mut lists)?;
            stems.push(stem);
        }
        Ok(Plan {
            stems,
            start,
            relaid,
            existing,
            lists,
            roles,
            idioms,
        })
    }

    // Every file the scaffold wrote and every list it added a line to.
    pub fn written(&self) -> &[String] {
        &self.written
    }

    // Every file the scaffold wrote or found and every list it holds a line
    // in, each with what a whole write of it must keep, sorted by path: a
    // module its requirement ids and the items the scaffold named, a test
    // file its ids and its tests, and a manifest, an entry, or a list every
    // line it holds, so a list is added to and never cut; and, for a module
    // or a test file the scaffold wrote, each marker it laid under the key
    // the brief quotes, so the turn fills a marker by naming it.
    pub fn guarded(&self) -> Vec<Guarded> {
        let mut keeps: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut markers: BTreeMap<&str, Vec<Marker>> = BTreeMap::new();
        for stem in &self.plan.stems {
            for kind in Kind::ALL {
                let Some(file) = stem.file(kind) else { continue };
                let tokens = match kind {
                    Kind::Module => stem.module_tokens(),
                    Kind::Tests => stem.test_tokens(),
                    Kind::Manifest | Kind::Entry => lines_of(&stem.text(kind).unwrap_or_default()),
                };
                keeps.entry(&file.path).or_default().extend(tokens);
                let keyed = match kind {
                    Kind::Module => stem.module_markers(),
                    Kind::Tests => stem.test_markers(),
                    Kind::Manifest | Kind::Entry => Vec::new(),
                };
                markers.entry(&file.path).or_default().extend(keyed);
            }
            for line in stem.lines.iter().filter(|line| line.placement != Placement::Unanchored) {
                let tokens =
                    self.plan.lists.held(&line.path).map_or_else(|| lines_of(&line.text), lines_of);
                keeps.entry(&line.path).or_default().extend(tokens);
            }
        }
        keeps
            .into_iter()
            .map(|(path, tokens)| {
                Guarded::new(path, unique(tokens))
                    .with_markers(markers.remove(path).unwrap_or_default())
            })
            .collect()
    }

    // The design keys of the types laid into the modules the scaffold wrote,
    // a marker among them, since its signature stands fenced in its doc; a
    // held module holds whatever an earlier slice left, so its types are
    // none.
    pub fn types(&self) -> Vec<String> {
        self.plan
            .stems
            .iter()
            .filter(|stem| !stem.module.held)
            .flat_map(|stem| stem.types.iter().map(|design_type| design_type.key.clone()))
            .collect()
    }

    // The files of the tree once the skeleton was written, which the check
    // holds the tree to: a file beyond them outside the slice's crates is a
    // stray.
    pub const fn listing(&self) -> &BTreeSet<String> {
        &self.listing
    }

    pub fn stems(&self) -> impl Iterator<Item = &str> {
        self.plan.stems.iter().map(|stem| stem.name.as_str())
    }

    pub const fn existing(&self) -> &BTreeSet<String> {
        &self.plan.existing
    }

    pub fn expected(&self) -> impl Iterator<Item = Expected<'_>> {
        self.plan
            .stems
            .iter()
            .filter_map(|stem| stem.tests.as_ref().map(|file| (stem, file)))
            .flat_map(|(stem, file)| {
                stem.scenarios.iter().map(move |test| Expected {
                    path: &file.path,
                    id: &test.id,
                    scenario: &test.scenario.name,
                    test: &test.name,
                })
            })
    }
}

impl Plan {
    // A slice of `start` requirements alone, none carrying a route, lays no
    // stem: nothing is scaffolded and no turn is opened for it.
    pub const fn is_empty(&self) -> bool {
        self.stems.is_empty()
    }

    // Writes each stem's files the tree lacks and adds each placed line to
    // its list, every list written once with its lines in order.
    pub fn write(mut self, root: &Path) -> Result<Scaffold, Error> {
        let mut written = Vec::new();
        for stem in &self.stems {
            for kind in Kind::ALL {
                if let Some(file) = stem.file(kind).filter(|file| !file.held) {
                    create(root, &file.path, &stem.render(kind))?;
                    written.push(file.path.clone());
                }
            }
        }

        let mut touched: Vec<&str> = Vec::new();
        for line in self.stems.iter().flat_map(|stem| &stem.lines) {
            if line.placement == Placement::Add && self.lists.add(line) {
                push_unique(&mut touched, &line.path);
            }
        }
        for path in touched {
            let Some(text) = self.lists.held(path) else { continue };
            write_whole(root, path, Standing::File, text.as_bytes())
                .map_err(|err| server_error!("writing `{path}`: {err}"))?;
            written.push(path.to_owned());
        }
        let listing = layout::listing(root)?;
        Ok(Scaffold {
            plan: self,
            written,
            listing,
        })
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
            probing: false,
            sql: false,
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
    fn plan(
        &mut self, owned: &[&DesignType], requirements: &[&Requirement], design_keys: &[String],
        topics: &mut Topics,
    ) {
        let probing = self.probing;
        let mut namespaces = Namespaces::default();
        let reads: Vec<types::Read> =
            owned.iter().map(|design_type| types::read(&design_type.signature)).collect();
        let names: Vec<String> = owned
            .iter()
            .zip(&reads)
            .map(|(design_type, read)| {
                let key = DesignKey::parse(&design_type.key);
                let declared = read.name.as_deref().unwrap_or(key.name);
                let suffix = key.stem.map_or_else(|| "2".to_owned(), pascal);
                namespaces.types.reserve(&pascal(declared), &suffix)
            })
            .collect();
        let bodies = types::lay(&reads, &names, design_keys, &mut namespaces.values);
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
            self.handler(route, &group, &mut namespaces, probing);
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
    fn handler(
        &mut self, route: &Route, group: &[&Requirement], namespaces: &mut Namespaces,
        probing: bool,
    ) {
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
        let encoded =
            route.method == "POST" || states_status(group) || (probing && route.method == "GET");
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

    fn read(&mut self, root: &Path) -> Result<(), Error> {
        let files = [&mut self.manifest, &mut self.entry, &mut self.module];
        for file in files.into_iter().chain(self.tests.as_mut()) {
            file.read(root)?;
        }
        Ok(())
    }

    // Decides each of the stem's lines against its list, the held entry and
    // manifest read once with the stem's files.
    fn place_lines(&mut self, root: &Path, lists: &mut Lists) -> Result<(), Error> {
        for file in [&self.entry, &self.manifest].into_iter().filter(|file| file.held) {
            lists.hold(&file.path, &file.text);
        }
        let mut lines = self.lines();
        for line in &mut lines {
            line.place(root, lists)?;
        }
        self.lines = lines;
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

    // What a whole write of the part must keep: each requirement's id, and
    // each item the scaffold named as its declaration spells it, `fn read`,
    // `struct ReadInput`, `const ORDERS_ID`, so the entry's calls and the
    // tests' names still resolve; a held part is an earlier slice's, so
    // nothing of it is the scaffold's to hold.
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

    // What a whole write of the tests must keep: each scenario's
    // requirement id and its test's name, which the check looks the
    // scenario up by.
    fn test_tokens(&self) -> Vec<String> {
        if self.tests.as_ref().is_none_or(|file| file.held) {
            return Vec::new();
        }
        self.scenarios
            .iter()
            .flat_map(|test| [test.id.clone(), format!("fn {}", test.name)])
            .collect()
    }

    // The part's markers under the keys the brief quotes, each standing for
    // the block the scaffold laid as the brief quotes it: a design type's
    // key for its empty marker, a handler's or a consumer's marker for its
    // body, a function's for its signature and body; a held part is an
    // earlier slice's, so none.
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
            markers.push(Marker::new(&function.marker, render::function_block(function)));
        }
        markers
    }

    // The tests' markers: each scenario's test block under its `#[ignore]`
    // text and under its marker text, so either names the block.
    fn test_markers(&self) -> Vec<Marker> {
        if self.tests.as_ref().is_none_or(|file| file.held) {
            return Vec::new();
        }
        self.scenarios
            .iter()
            .flat_map(|test| {
                let block = render::test_block(self, test);
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
                format!("\n{}", render::topic_constant(&consumer.constant, &consumer.topic)),
                Place::FileEnd,
            ));
        }
        if self.manifest.held && self.sql {
            lines.push(Line::new(
                &self.manifest.path,
                render::dependency_line(layout::PROBE_DEPENDENCY),
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
    const fn origin(&self) -> &'static str {
        if self.held { "held" } else { "scaffolded" }
    }
}

impl Handler {
    // The `.route(..)` call that registers the handler: its route function
    // where one was scaffolded, else the SDK's method fn over it.
    fn registration(&self) -> Call {
        let service = self.route_fn.as_ref().map_or_else(
            || Call::new(trigger::method_fn(&self.route.method), [self.name.as_str()]),
            |route_fn| Call::new(route_fn.as_str(), Vec::<Expr>::new()),
        );
        Call::new(".route", [Expr::from(self.constant.as_str()), service.into()])
    }
}

impl Type {
    // `struct Order` or `enum Status`, as the marker declares it.
    fn declaration(&self) -> String {
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

// Every non-blank line of `text`, trimmed, as the tokens a whole write of a
// list or a fixed file must keep.
fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::trim).filter(|line| !line.is_empty()).map(ToOwned::to_owned).collect()
}

// Creates the file the plan found absent, its directories made one at a
// time and no link followed.
fn create(root: &Path, path: &str, text: &str) -> Result<(), Error> {
    write_whole(root, path, Standing::Absent, text.as_bytes())
        .map_err(|err| server_error!("creating `{path}`: {err}"))
}
