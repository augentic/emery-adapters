//! Lays a slice's skeleton before its turn: the stem's crate, the part, its tests, and its lines in every list.

mod names;
mod render;
mod route;

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::Path;

use emery_sdk::target::{DesignType, Requirement, Scenario, Slice};
use emery_sdk::{Error, bad_request, server_error};

use self::names::{Names, escape, flat, number, pascal, screaming, snake};
use self::render::Place;
use self::route::Route;
use crate::tree;

pub struct Scaffold {
    stems: Vec<Stem>,
    start: Vec<String>,
    existing: BTreeSet<String>,
    lines: Vec<Line>,
}

// What a check reads of one scenario the scaffold laid a test for.
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

impl Scaffold {
    pub fn plan(slice: &Slice, root: &Path) -> Result<Self, Error> {
        let existing: BTreeSet<String> = tree::crates(root)?.into_iter().collect();

        // the stems the slice builds, `start` set aside
        let mut stems: Vec<Stem> = Vec::new();
        let mut start = Vec::new();
        for requirement in &slice.requirements {
            let name = requirement.stem();
            if tree::is_start_stem(name) {
                start.push(requirement.id.clone());
            } else if !stems.iter().any(|stem| stem.name == name) {
                stems.push(Stem::new(name, slice));
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

        for (stem, types) in stems.iter_mut().zip(&owned) {
            stem.plan(slice, types);
            stem.read(root)?;
        }
        let mut lines: Vec<Line> = stems.iter().flat_map(Stem::lines).collect();
        for line in &mut lines {
            line.read(root)?;
        }
        Ok(Self {
            stems,
            start,
            existing,
            lines,
        })
    }

    pub fn lay(&self, root: &Path) -> Result<Vec<String>, Error> {
        let mut laid = Vec::new();
        for stem in &self.stems {
            for kind in [Kind::Manifest, Kind::Entry, Kind::Module, Kind::Tests] {
                if let Some(file) = stem.file(kind).filter(|file| !file.held) {
                    create(root, &file.path, &stem.render(kind))?;
                    laid.push(file.path.clone());
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
            laid.push(path.to_owned());
        }
        Ok(laid)
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
                "The `start` stem ({}): the seed and the host are `start`, so nothing was laid for \
                 it; cover what they satisfy and leave the rest out, as the guardrails' table \
                 says.\n",
                self.start.join(", ")
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
            functions: Vec::new(),
            types: Vec::new(),
            scenarios: Vec::new(),
        }
    }

    // Names every item the part declares, the design types first so one
    // keeps its name over a handler's wrapper.
    fn plan(&mut self, slice: &Slice, owned: &[&DesignType]) {
        let mut values = Names::reserving(&["routes", "get", "post", "put", "patch", "delete"]);
        let mut types =
            Names::reserving(&["Capabilities", "Client", "Context", "Deserialize", "Serialize"]);
        let mut constants = Names::default();
        let mut tests = Names::default();

        for design_type in owned {
            let key = design_type.key.as_str();
            let last = key.rsplit('.').next().unwrap_or(key);
            let first = key.split('.').next().unwrap_or(key);
            self.types.push(Type {
                name: types.claim(&pascal(last), &pascal(first)),
                key: key.to_owned(),
                signature: design_type.signature.clone(),
            });
        }

        // the routed requirements grouped by route, the rest a function each
        let mut routed: Vec<(Route, Vec<&Requirement>)> = Vec::new();
        for requirement in slice.requirements.iter().filter(|r| r.stem() == self.name) {
            let suffix = format!("_{}", number(&requirement.id));
            if let Some(route) = route::of(requirement) {
                match routed.iter_mut().find(|(known, _)| *known == route) {
                    Some((_, group)) => group.push(requirement),
                    None => routed.push((route, vec![requirement])),
                }
            } else {
                let tail = tail(&requirement.subject);
                let base = if tail.is_empty() { snake(&self.name) } else { snake(tail) };
                self.functions.push(Function {
                    name: values.claim(&base, &suffix),
                    marker: format!("{} {}: the behaviour", requirement.id, requirement.subject),
                    stated: Stated::of(requirement),
                });
            }
            for scenario in &requirement.scenarios {
                let name = flat(&scenario.name);
                self.scenarios.push(Test {
                    name: tests.claim(&snake(&name), &suffix),
                    id: requirement.id.clone(),
                    subject: requirement.subject.clone(),
                    scenario: scenario.clone(),
                    ignore: format!("{} {name}: unfilled", requirement.id),
                    marker: format!("{} {name}: arrange, act, assert", requirement.id),
                });
            }
        }
        for (route, group) in routed {
            let suffix = number(&group[0].id);
            let name = values.claim(&handler_base(&group, &route), &format!("_{suffix}"));
            let title = pascal(&name);
            let constant = if let Some((constant, _)) =
                self.paths.iter().find(|(_, path)| *path == route.path)
            {
                constant.clone()
            } else {
                let constant = constants.claim(&screaming(&route.path), &format!("_{suffix}"));
                self.paths.push((constant.clone(), route.path.clone()));
                constant
            };
            let stated: Vec<Stated> =
                group.iter().map(|requirement| Stated::of(requirement)).collect();
            let named: Vec<String> =
                stated.iter().map(|stated| format!("{} {}", stated.id, stated.subject)).collect();
            self.handlers.push(Handler {
                input: types.claim(&format!("{title}Input"), &suffix),
                output: types.claim(&format!("{title}Output"), &suffix),
                name,
                constant,
                params: route::params(&route.path)
                    .into_iter()
                    .map(|wire| Param {
                        field: snake(&wire),
                        wire,
                    })
                    .collect(),
                route,
                stated,
                marker: format!("{}: the handler", named.join(", ")),
            });
        }

        if !self.scenarios.is_empty() {
            self.tests = Some(File::at(format!("crates/{}/tests/{}.rs", self.name, self.part)));
        }
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

    // The stem's lines in the root lists and, where the tree holds its
    // entry, the part's lines in the entry's.
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
        let _ = writeln!(
            text,
            "- `{}`: {}, {} {} its lists.",
            self.entry.path,
            self.entry.standing(),
            entry_lines.join(" and "),
            if self.entry.held { "added to" } else { "in" }
        );
        let _ = writeln!(
            text,
            "- `{}`: {}. {}",
            self.module.path,
            self.module.standing(),
            if self.module.held { HELD } else { "Fill its markers, listed below." }
        );
        match &self.tests {
            Some(tests) if tests.held => {
                let _ = writeln!(text, "- `{}`: held. {HELD}", tests.path);
            }
            Some(tests) => {
                let _ = writeln!(
                    text,
                    "- `{}`: laid, one ignored test per scenario. Fill each and remove its \
                     `#[ignore = ..]` line.",
                    tests.path
                );
            }
            None => text.push_str("- No scenario states a test, so no test file was laid.\n"),
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
                "The markers below are the ones a fresh scaffold lays; a held file holds whatever \
                 stands in it.\n\n",
            );
        }
        text.push_str("Markers, in fill order:\n\n");
        let mut count = 0;
        for design_type in &self.types {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `pub struct {} {{}}` in `{}`: the design type `{}`. Lay its fields from \
                 the signature in its doc.",
                design_type.name, self.module.path, design_type.key
            );
        }
        for handler in &self.handlers {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `todo!(\"{}\")` in `{}`: the handler `{}` of `{} {}`. Fill `{}` and \
                 `{}` with the wire's fields, write the body, and drop the `let _ = (input, \
                 context);` line.",
                escape(&handler.marker),
                self.module.path,
                handler.name,
                handler.route.method,
                handler.route.path,
                handler.input,
                handler.output
            );
        }
        for function in &self.functions {
            count += 1;
            let _ = writeln!(
                text,
                "{count}. `todo!(\"{}\")` in `{}`: the function `{}`, since the requirement \
                 states no route. Give it the signature the behaviour needs and write the body.",
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
    "The tree already held it, so nothing was laid over it; read it for what stands.";

impl File {
    const fn at(path: String) -> Self {
        Self { path, held: false }
    }

    fn read(&mut self, root: &Path) -> Result<(), Error> {
        self.held = match fs::symlink_metadata(root.join(&self.path)) {
            Ok(meta) if meta.is_file() => true,
            Ok(_) => {
                return Err(bad_request!(
                    "a directory or a link stands at `{}`, where the slice's scaffold lays a file",
                    self.path
                ));
            }
            Err(err) if err.kind() == ErrorKind::NotFound => false,
            Err(err) => return Err(server_error!("reading `{}`: {err}", self.path)),
        };
        Ok(())
    }

    const fn standing(&self) -> &'static str {
        if self.held { "held" } else { "laid" }
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
                if text.lines().any(|held| held.trim() == self.text.trim()) {
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

// The subject tail the route's requirements share, else the first's, else
// the route itself, as a function name.
fn handler_base(group: &[&Requirement], route: &Route) -> String {
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
        return snake(&format!("{} {}", route.method, route.path));
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
