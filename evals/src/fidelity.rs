//! The host's own pass over the tree a build left: its layout, its toolchain's checks, its tests.

use std::collections::BTreeSet;
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::{fs, io};

use serde_json::Value;

use crate::build::{Build, Integrated, Tree};
use crate::grade::stem;
use crate::reference::{self, Reference};
use crate::stage::export_tip;

// A target adapter a `--build` run builds through, and what the host reads
// of the tree it leaves: the files its layout admits, the checks its
// toolchain runs, and how its tests are counted and named.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Target {
    #[default]
    Node,
    Omnia,
}

impl Target {
    const ALL: [Self; 2] = [Self::Node, Self::Omnia];

    pub fn parse(name: &str) -> Result<Self, String> {
        Self::ALL.into_iter().find(|target| target.name() == name).ok_or_else(|| {
            let names: Vec<&str> = Self::ALL.iter().map(|target| target.name()).collect();
            format!("--target: `{name}` is no target adapter; one of {}", names.join(", "))
        })
    }

    // The crate's name: the component's, and the `eval:` package's.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Omnia => "omnia",
        }
    }

    // The one file the checks may write into the tree without it counting.
    pub const fn lockfile(self) -> &'static str {
        match self {
            Self::Node => "package-lock.json",
            Self::Omnia => "Cargo.lock",
        }
    }

    // The directory the checks leave beside the tree, passed over when the
    // export is listed.
    const fn scratch(self) -> &'static str {
        match self {
            Self::Node => "node_modules",
            Self::Omnia => "target",
        }
    }

    // A configuration file the checks find by walking up from the tree,
    // written empty at the export's root for the checks when the tree holds
    // none, so the export, which sits beneath this repository, is read
    // under the tree's own configuration as the worker under the fence
    // reads it, and never under this repository's.
    pub const fn config_shim(self) -> Option<&'static str> {
        match self {
            Self::Node => None,
            Self::Omnia => Some("clippy.toml"),
        }
    }

    // The check whose exit stands for the tests, and whose output counts them.
    pub const fn runner(self) -> &'static str {
        match self {
            Self::Node => "node --test",
            Self::Omnia => "cargo test",
        }
    }

    // The root files the layout admits: for `omnia`, the template the adapter
    // scaffolds and the lockfile.
    const fn root_files(self) -> &'static [&'static str] {
        match self {
            Self::Node => &["package.json", "package-lock.json", "tsconfig.json", ".gitignore"],
            Self::Omnia => &[
                "Cargo.toml",
                "Cargo.lock",
                "src/lib.rs",
                "src/routes.rs",
                "src/messaging.rs",
                "tests/routes.rs",
                "tests/messaging.rs",
                "examples/runtime.rs",
                "crates/common/Cargo.toml",
                "crates/common/src/lib.rs",
                "crates/common/src/topic.rs",
                "crates/common/src/config.rs",
                "Makefile",
                "mise.toml",
                "rust-toolchain.toml",
                "rustfmt.toml",
                "taplo.toml",
                "clippy.toml",
                "deny.toml",
                ".gitignore",
                ".vscode/settings.json",
                "supply-chain/README.md",
                "supply-chain/config.toml",
                "supply-chain/audits.toml",
                ".github/dependabot.yml",
                ".github/workflows/audit.yaml",
                ".github/workflows/ci.yaml",
                ".github/workflows/patch.yaml",
                ".github/workflows/release.yaml",
                ".github/workflows/publish.yaml",
            ],
        }
    }

    // Whether a tracked path is one the layout admits: a root file, the entry
    // or the runner, a file under a stem's directory, or a stem's test file
    // or directory. The sealed specification under `spec/` is the engine's,
    // held in every tree a build lands in and written by no slice, so it is
    // never outside.
    pub fn within_layout(self, path: &str, stems: &BTreeSet<&str>) -> bool {
        if path.starts_with(SPECIFICATION) || self.root_files().contains(&path) {
            return true;
        }
        match self {
            Self::Node => {
                if let Some(rest) = path.strip_prefix("src/") {
                    if rest == "index.ts" || rest == "main.ts" {
                        return true;
                    }
                    return rest.split_once('/').is_some_and(|(dir, _)| stems.contains(dir));
                }
                if let Some(rest) = path.strip_prefix("test/") {
                    return rest.strip_suffix(".test.ts").is_some_and(|name| stems.contains(name))
                        || rest.split_once('/').is_some_and(|(dir, _)| stems.contains(dir));
                }
                false
            }
            Self::Omnia => path
                .strip_prefix("crates/")
                .and_then(|rest| rest.split_once('/'))
                .is_some_and(|(dir, _)| stems.contains(dir)),
        }
    }

    // The layout's checks run in `export`, each unrun once an earlier one
    // fails to leave what it needs.
    fn checks(self, export: &Path) -> Vec<HostCheck> {
        match self {
            Self::Node => {
                let install = host_check(export, "npm install", &["--no-audit", "--no-fund"]);
                let installed = install.passed();
                let compile = if installed {
                    host_check(export, "npx tsc", &["--noEmit"])
                } else {
                    unrun("npx tsc", "the install failed")
                };
                let tests = if installed {
                    host_check(
                        export,
                        "node --test",
                        &[
                            "--test-force-exit",
                            "--test-timeout=5000",
                            "--test-reporter=tap",
                            "test/**/*.test.ts",
                        ],
                    )
                } else {
                    unrun("node --test", "the install failed")
                };
                vec![install, compile, tests]
            }
            Self::Omnia => {
                let lint = host_check(
                    export,
                    "cargo clippy",
                    &["--workspace", "--all-targets", "--", "-D", "warnings"],
                );
                let linted = lint.passed();
                let tests = if linted {
                    host_check(export, "cargo test", &["--workspace"])
                } else {
                    unrun("cargo test", "the lint failed")
                };
                let guest = if tests.passed() {
                    host_check(export, "cargo build", &["--target", "wasm32-wasip2"])
                } else if linted {
                    unrun("cargo build", "the tests failed")
                } else {
                    unrun("cargo build", "the lint failed")
                };
                vec![lint, tests, guest]
            }
        }
    }

    // The tests the runner's output counts as passed and failed.
    fn test_counts(self, runner: &HostCheck) -> (usize, usize) {
        match self {
            Self::Node => tap_counts(&runner.note),
            Self::Omnia => cargo_test_counts(&runner.output),
        }
    }

    // Every test name the tree's test files declare, spelled as a scenario
    // is: a `test(` or `it(` string, or a `#[test]` fn's name with its
    // underscores as spaces.
    fn test_names(self, export: &Path) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        match self {
            Self::Node => test_names(&export.join("test"), &mut names)?,
            Self::Omnia => {
                rust_test_names(&export.join("tests"), &mut names)?;
                let crates = export.join("crates");
                if crates.is_dir() {
                    for entry in fs::read_dir(crates)? {
                        rust_test_names(&entry?.path().join("tests"), &mut names)?;
                    }
                }
            }
        }
        Ok(names)
    }

    // Every module the layout's lists do not reach, sorted: a handler module
    // `crates/<stem>/src/<part>.rs` whose stem's entry names neither its
    // `routes()` nor its `topics`, a stem crate `crates/<stem>/` neither root
    // list merges and no Rust file outside it names as `<stem>::`, a
    // constant of the topic table no stem's module names, or a stem's or a
    // part's `index.ts` the list above it does not re-export. A module's own
    // tests pass without the list, so the checks cannot see it.
    pub fn unreached(self, export: &Path) -> io::Result<Vec<String>> {
        let mut unreached = Vec::new();
        match self {
            Self::Node => {
                let src = export.join("src");
                let entry = fs::read_to_string(src.join("index.ts")).unwrap_or_default();
                for stem in directories(&src)? {
                    let stem_dir = src.join(&stem);
                    if !stem_dir.join("index.ts").is_file() {
                        continue;
                    }
                    if !reexported(&entry, &stem) {
                        unreached.push(format!("src/{stem}/index.ts"));
                    }
                    let list = fs::read_to_string(stem_dir.join("index.ts"))?;
                    for part in directories(&stem_dir)? {
                        if stem_dir.join(&part).join("index.ts").is_file()
                            && !reexported(&list, &part)
                        {
                            unreached.push(format!("src/{stem}/{part}/index.ts"));
                        }
                    }
                }
            }
            Self::Omnia => {
                let routes =
                    code_lines(&fs::read_to_string(export.join(ROUTE_LIST)).unwrap_or_default());
                let topics =
                    code_lines(&fs::read_to_string(export.join(TOPIC_LIST)).unwrap_or_default());
                let mut paths = Vec::new();
                files_under(export, export, "target", &mut paths)?;
                let mut sources = Vec::new();
                for path in paths
                    .iter()
                    .filter(|path| Path::new(path).extension().is_some_and(|e| e == "rs"))
                {
                    sources
                        .push((path.as_str(), code_lines(&fs::read_to_string(export.join(path))?)));
                }
                let mut modules = Vec::new();
                let crates = export.join("crates");
                for stem in directories(&crates)? {
                    if stem == "common" {
                        continue;
                    }
                    let src = crates.join(&stem).join("src");
                    if !src.is_dir() {
                        continue;
                    }
                    let name = stem.replace('-', "_");
                    let entry =
                        code_lines(&fs::read_to_string(src.join("lib.rs")).unwrap_or_default());

                    // a stem neither list merges and nothing outside it names
                    let own = format!("crates/{stem}/");
                    let merged = spells_word(&routes, &format!("{name}::router"))
                        || spells_word(&topics, &format!("{name}::topics"));
                    if !merged
                        && crates.join(&stem).join("Cargo.toml").is_file()
                        && !sources
                            .iter()
                            .any(|(path, text)| !path.starts_with(&own) && names_crate(text, &name))
                    {
                        unreached.push(own);
                    }

                    // a handler module the entry's arrays do not name
                    for entry_path in fs::read_dir(&src)? {
                        let path = entry_path?.path();
                        let Some(part) = path.file_stem().and_then(|part| part.to_str()) else {
                            continue;
                        };
                        if path.extension().is_none_or(|ext| ext != "rs") {
                            continue;
                        }
                        let text = code_lines(&fs::read_to_string(&path)?);
                        if part != "lib"
                            && text.contains("Context<")
                            && !spells_word(&entry, &format!("{part}::routes"))
                            && !spells_word(&entry, &format!("{part}::topics"))
                        {
                            unreached.push(format!("crates/{stem}/src/{part}.rs"));
                        }
                        modules.push(text);
                    }
                }

                let topic = fs::read_to_string(export.join(TOPIC_TABLE)).unwrap_or_default();
                for (name, _) in constants(&topic) {
                    if !modules.iter().any(|text| spells_word(text, &name)) {
                        unreached.push(format!("{TOPIC_TABLE}#{name}"));
                    }
                }
            }
        }
        unreached.sort();
        Ok(unreached)
    }

    // Every route the tree registers, in tree order: for `omnia`, each
    // `.route(<path>, <method>(<handler>))` of a part's `routes()`,
    // attributed to the part's module `crates/<stem>/src/<part>.rs`, the
    // path read from the constant the module declares or a literal, and the
    // method each route fn, or `MethodFilter`, in its second argument
    // spells; a `.route(..)` over a topic is none. The TypeScript layout
    // registers routes on an app a slice is handed, which no one file
    // spells, so it lists none.
    pub fn routes(self, export: &Path) -> Vec<(String, String, String)> {
        let mut routes = Vec::new();
        if self == Self::Node {
            return routes;
        }
        let crates = export.join("crates");
        let Ok(mut stems) = directories(&crates) else {
            return routes;
        };
        stems.sort();
        for stem in stems {
            if stem == "common" {
                continue;
            }
            let src = crates.join(&stem).join("src");
            let Ok(entries) = fs::read_dir(&src) else {
                continue;
            };
            let mut parts: Vec<PathBuf> =
                entries.filter_map(Result::ok).map(|e| e.path()).collect();
            parts.sort();
            for path in parts {
                let Some(part) = path.file_stem().and_then(|part| part.to_str()) else {
                    continue;
                };
                if part == "lib" || path.extension().is_none_or(|ext| ext != "rs") {
                    continue;
                }
                let text = fs::read_to_string(&path).unwrap_or_default();
                let module = format!("crates/{stem}/src/{part}.rs");
                for statement in registered(&text, &text) {
                    if statement.topic {
                        continue;
                    }
                    for method in statement.methods {
                        routes.push((module.clone(), method, statement.path.clone()));
                    }
                }
            }
        }
        routes
    }
}

// The root-level directory `emery apply` seals the three documents under, in
// every repository a build lands in.
const SPECIFICATION: &str = "spec/";

// The omnia layout's two root lists and the topic table of `common`.
const ROUTE_LIST: &str = "src/routes.rs";
const TOPIC_LIST: &str = "src/messaging.rs";
const TOPIC_TABLE: &str = "crates/common/src/topic.rs";

// Each `pub const <NAME>: &str = "<text>";` or `pub(crate) const ..` of a
// file, in order, with the text it spells.
fn constants(table: &str) -> Vec<(String, String)> {
    table
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let declaration = line
                .strip_prefix("pub const ")
                .or_else(|| line.strip_prefix("pub(crate) const "))?;
            let (name, rest) = declaration.split_once(':')?;
            let text = literal(rest.split_once('=')?.1.trim().strip_suffix(';')?.trim())?;
            Some((name.trim().to_owned(), text.to_owned()))
        })
        .collect()
}

fn code_lines(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| if line.trim_start().starts_with("//") { "\n" } else { line })
        .collect()
}

// Whether `text` reaches into the crate `name`: `name::` spelled at the
// head of a path, `use name::..` or `name::module::item`.
fn names_crate(text: &str, name: &str) -> bool {
    let path = format!("{name}::");
    let ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut from = 0;
    while let Some(at) = text[from..].find(&path) {
        let start = from + at;
        if text[..start].chars().next_back().is_none_or(|ch| !ident(ch) && ch != ':') {
            return true;
        }
        from = start + path.len();
    }
    false
}

fn spells_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let ident = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut from = 0;
    while let Some(at) = text[from..].find(word) {
        let start = from + at;
        let end = start + word.len();
        let before = text[..start].chars().next_back().is_none_or(|ch| !ident(ch));
        let after = text[end..].chars().next().is_none_or(|ch| !ident(ch));
        if before && after {
            return true;
        }
        from = end;
    }
    false
}

// One `.route(<path>, <method>(<handler>))` statement of a list: whether it
// registers a topic, its first argument qualified through `topic_for` or
// `topic`; the path, a literal or the table's constant the first argument
// names by its last segment, `?` where it is neither; and the methods the
// second argument spells, one per route fn (`get(..)`) or
// `MethodFilter::<M>`, those of the `handle_with(..)` a route function of
// the list's own text names where the argument calls one (`create_route()`),
// `?` where it spells none.
struct Registered {
    topic: bool,
    path: String,
    methods: Vec<String>,
}

// Each statement of a list, in order.
fn registered(list: &str, table: &str) -> Vec<Registered> {
    let constants = constants(table);
    let mut routes = Vec::new();
    let mut from = 0;
    while let Some(found) = list[from..].find(".route(") {
        let open = from + found + ".route(".len();
        let Some(close) = closing(&list[open..]).map(|at| open + at) else {
            break;
        };
        let call = &list[open..close];
        let (first, second) =
            top_level_comma(call).map_or((call, ""), |at| (&call[..at], &call[at + 1..]));
        let first = first.trim();
        let path = literal(first)
            .or_else(|| {
                let name = first.rsplit("::").next()?;
                constants
                    .iter()
                    .find(|(declared, _)| declared == name)
                    .map(|(_, text)| text.as_str())
            })
            .unwrap_or("?")
            .to_owned();
        let topic = first.contains("topic_for(") || first.contains("topic(");
        let mut methods = methods_in(second);
        if methods.is_empty() {
            methods = declared_methods(list, second);
        }
        if methods.is_empty() {
            methods.push("?".to_owned());
        }
        routes.push(Registered { topic, path, methods });
        from = close;
    }
    routes
}

// The offset of the `)` closing a call whose `(` is just before `text`,
// string literals skipped; none where the call is unclosed.
fn closing(text: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            _ if escaped => escaped = false,
            b'\\' if quoted => escaped = true,
            b'"' => quoted = !quoted,
            _ if quoted => {}
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

// The offset of the first `,` of `text` outside every bracket and string.
fn top_level_comma(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quoted = false;
    for (index, byte) in text.bytes().enumerate() {
        match byte {
            b'"' => quoted = !quoted,
            _ if quoted => {}
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

// The text of a plain string literal, `"…"`, with nothing around it.
fn literal(text: &str) -> Option<&str> {
    let inner = text.strip_prefix('"')?.strip_suffix('"')?;
    (!inner.contains('"')).then_some(inner)
}

// The methods a route expression spells, in order: each route fn called
// bare (`get(..)`, never `map.get(..)`) and each `MethodFilter::<M>`.
fn methods_in(expression: &str) -> Vec<String> {
    const VERBS: [&str; 7] = ["get", "post", "put", "patch", "delete", "head", "options"];
    let mut methods = Vec::new();
    let mut at = 0;
    while let Some(found) = expression[at..].find(|c: char| c.is_ascii_alphabetic() || c == '_') {
        let start = at + found;
        let end = expression[start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .map_or(expression.len(), |len| start + len);
        let word = &expression[start..end];
        let before = &expression[..start];
        let called = expression[end..].starts_with('(');
        if before.ends_with("MethodFilter::") && word.bytes().all(|b| b.is_ascii_uppercase()) {
            methods.push(word.to_owned());
        } else if VERBS.contains(&word) && called && !before.ends_with('.') {
            methods.push(word.to_ascii_uppercase());
        }
        at = end;
    }
    methods
}

// The methods a route expression spells through a route function of the
// module, `<name>()` or `<name>::<P>()`: those the `handle_with(..)` in the
// body of `fn <name>` names; none where the expression calls no function,
// the module declares none of that name, or its body names no filter.
fn declared_methods(module: &str, expression: &str) -> Vec<String> {
    let expression = expression.trim();
    let end = expression
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(expression.len());
    let name = &expression[..end];
    if name.is_empty() || !expression[end..].contains('(') {
        return Vec::new();
    }
    let declaration = format!("fn {name}");
    let mut from = 0;
    while let Some(at) = module[from..].find(&declaration) {
        let start = from + at;
        let after = module[start + declaration.len()..].chars().next();
        if matches!(after, Some('(' | '<')) {
            let body = &module[start..];
            let Some(call) = body.find("handle_with(") else {
                return Vec::new();
            };
            let open = call + "handle_with(".len();
            let close = closing(&body[open..]).map_or(body.len(), |at| open + at);
            return methods_in(&body[open..close]);
        }
        from = start + declaration.len();
    }
    Vec::new()
}

// The names of the directories under `dir`; none where `dir` is no directory.
fn directories(dir: &Path) -> io::Result<Vec<String>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && let Some(name) = entry.file_name().to_str()
        {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

// Whether a list re-exports the directory `name` beside it: an
// `export * as … from "./<name>/index.ts"` line, the specifier read
// whatever it names within the directory.
pub fn reexported(list: &str, name: &str) -> bool {
    list.lines().map(str::trim).filter(|line| line.starts_with("export * as")).any(|line| {
        let Some((_, from)) = line.split_once(" from ") else {
            return false;
        };
        let specifier = from.trim().trim_end_matches(';').trim_matches(|c| c == '"' || c == '\'');
        specifier == format!("./{name}") || specifier.starts_with(&format!("./{name}/"))
    })
}

// What the host's own checks say of the tree a build left: the layout's
// three checks run in a fresh export of the tip, the tests they ran, the
// scenarios of the merged slices a test is named by, the modules the
// layout's lists do not reach, the tracked files outside the layout, what
// the checks wrote besides the lockfile, and, for an `omnia` build of a
// case naming a reference guest, the tree measured against it and its
// fixtures driven through the guest.
pub struct Fidelity {
    // the file beside the run's log the pass is written to
    pub file: String,
    pub tree: String,
    pub target: Target,
    pub checks: Vec<HostCheck>,
    // the tests the runner reported passed and failed
    pub passed: usize,
    pub failed: usize,
    // the test names the tree's test files declare
    pub tests: usize,
    pub scenarios: Vec<Scenario>,
    pub unreached: Vec<String>,
    pub routes: Vec<Route>,
    pub outside: Vec<String>,
    pub wrote: Vec<String>,
    pub reference: Option<reference::Comparison>,
}

// One route a module of the tree mounts, as its source spells it; a probe
// is a route whose module's requirements state no method and path, so the
// surface is the layout's shape and not the specification's.
#[derive(Debug, PartialEq, Eq)]
pub struct Route {
    pub module: String,
    pub method: String,
    pub path: String,
    pub probe: bool,
}

// One command the host ran in the exported tree: its exit, or none where an
// earlier check's failure left it unrun, what it printed, and the tail of it.
pub struct HostCheck {
    pub name: &'static str,
    pub exit: Option<i32>,
    pub output: String,
    pub note: String,
}

impl HostCheck {
    fn passed(&self) -> bool {
        self.exit == Some(0)
    }

    // The one line of the note the table shows: the TAP counts for Node's
    // runner, the summed `test result` counts for cargo's, the last line
    // printed for the rest.
    pub fn summary(&self) -> String {
        let lines: Vec<&str> =
            self.note.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
        if self.name == "node --test" {
            let counts: Vec<&str> = lines
                .iter()
                .copied()
                .filter(|line| {
                    ["# tests", "# pass", "# fail"].iter().any(|key| line.starts_with(key))
                })
                .collect();
            if !counts.is_empty() {
                return counts.join(" · ");
            }
        }
        if self.name == "cargo test" && self.output.contains("test result:") {
            let (passed, failed) = cargo_test_counts(&self.output);
            return format!("{passed} passed · {failed} failed");
        }
        lines.last().copied().unwrap_or("").to_owned()
    }
}

// One acceptance scenario of a requirement a merged slice covered, and the
// test named by it, if any.
pub struct Scenario {
    pub stem: String,
    pub unknown: bool,
    pub named: Option<Named>,
}

// The scenarios of the requirements the merged slices covered, each with
// the test among `names` it is named by: an uncovered requirement has no
// test to name, by the adapter's own rule, so its scenarios are not counted
// against the ratio; where nothing merged, every requirement's are, so a
// build that landed no slice still shows what its tests would be named by.
pub fn scenarios_of(
    requirements: &[Value], integrated: &[Integrated], names: &[String],
) -> Vec<Scenario> {
    let covered: BTreeSet<&str> =
        integrated.iter().flat_map(|slice| slice.covered.iter().map(String::as_str)).collect();
    let mut scenarios = Vec::new();
    for requirement in requirements {
        let Some(id) = requirement["id"].as_str() else {
            continue;
        };
        if !integrated.is_empty() && !covered.contains(id) {
            continue;
        }
        let stem = requirement["subject"].as_str().and_then(stem).unwrap_or("").to_owned();
        let listed = requirement["scenarios"].as_array().map(Vec::as_slice).unwrap_or_default();
        for scenario in listed {
            scenarios.push(Scenario {
                stem: stem.clone(),
                unknown: scenario["then"].as_str() == Some("[unknown]"),
                named: named_by(scenario["name"].as_str().unwrap_or(""), names),
            });
        }
    }
    scenarios
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    Exact,
    Fuzzy,
}

impl Fidelity {
    pub fn read(
        project: &Path, tag: &str, tree: &Tree, build: &Build, spec: Option<&Value>,
        target: Target, reference: Option<&Reference>,
    ) -> io::Result<Self> {
        let export = project.join(format!("{tag}.fidelity"));
        let tracked = export_tip(tree, &export)?;

        // the layout's checks under the tree's own configuration, then the
        // tests the runner counted
        let shim = target.config_shim().map(|name| export.join(name)).filter(|path| !path.exists());
        if let Some(shim) = &shim {
            fs::write(shim, "")?;
        }
        let checks = target.checks(&export);
        if let Some(shim) = &shim {
            fs::remove_file(shim)?;
        }
        let (passed, failed) = checks
            .iter()
            .find(|check| check.name == target.runner())
            .map_or((0, 0), |runner| target.test_counts(runner));

        // the tests the tree declares, against the covered scenarios
        let names = target.test_names(&export)?;
        let integrated = build.integrated();
        let requirements = spec
            .and_then(|spec| spec["requirements"].as_array())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let scenarios = scenarios_of(requirements, &integrated, &names);

        // the modules the lists do not reach, the routes they mount, the
        // files outside the layout, and what the checks wrote
        let unreached = target.unreached(&export)?;
        let routes = routes(target.routes(&export), &integrated, requirements);
        let stems: BTreeSet<&str> =
            requirements.iter().filter_map(|r| r["subject"].as_str().and_then(stem)).collect();
        let outside =
            tracked.iter().filter(|path| !target.within_layout(path, &stems)).cloned().collect();
        let mut present = Vec::new();
        files_under(&export, &export, target.scratch(), &mut present)?;
        let wrote = present
            .into_iter()
            .filter(|path| path != target.lockfile() && !tracked.contains(path))
            .collect();

        // the reference guest the case names, measured against the tree and
        // its fixtures driven through the guest, for an omnia build alone
        let reference =
            reference.filter(|_| matches!(target, Target::Omnia)).and_then(|reference| {
                match reference.compare(&export) {
                    Ok(comparison) => Some(comparison),
                    Err(error) => {
                        eprintln!("eval: `{tag}` reference not compared: {error}");
                        None
                    }
                }
            });
        fs::remove_dir_all(&export)?;

        Ok(Self {
            file: format!("{tag}.fidelity.md"),
            tree: tree.to_string(),
            target,
            checks,
            passed,
            failed,
            tests: names.len(),
            scenarios,
            unreached,
            routes,
            outside,
            wrote,
            reference,
        })
    }

    fn probes(&self) -> usize {
        self.routes.iter().filter(|route| route.probe).count()
    }

    // Whether the host's three checks all passed, with every test.
    pub fn clean(&self) -> bool {
        self.checks.iter().all(HostCheck::passed) && self.failed == 0
    }

    fn named(&self) -> usize {
        self.scenarios.iter().filter(|scenario| scenario.named.is_some()).count()
    }

    // The `host checks` cell: `ok` with the tests' count, else each check
    // that failed or was left unrun.
    pub fn host_cell(&self) -> String {
        let tests = format!("{}/{}", self.passed, self.passed + self.failed);
        if self.clean() {
            return format!("ok {tests}");
        }
        let mut parts = Vec::new();
        for check in &self.checks {
            let runner = check.name == self.target.runner();
            match check.exit {
                Some(0) if !runner || self.failed == 0 => {}
                Some(_) if runner => parts.push(format!("tests {tests}")),
                Some(_) => parts.push(format!("{} ✗", check.name)),
                None => parts.push(format!("{} unrun", check.name)),
            }
        }
        parts.join(" · ")
    }

    // The `scenarios named` cell: the scenarios a test is named by, the
    // tracked files outside the layout, the modules the lists do not
    // reach, and the probe routes.
    pub fn scenarios_cell(&self) -> String {
        let total = self.scenarios.len();
        let percent = (self.named() * 100).checked_div(total).unwrap_or(0);
        format!(
            "{}/{total} ({percent}%) · {} outside · {} unreached · {} probes",
            self.named(),
            self.outside.len(),
            self.unreached.len(),
            self.probes()
        )
    }

    // The routes the tree mounts, per module, each probe marked.
    fn routes_section(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.routes.is_empty() {
            return writeln!(f, "\n## Routes\n\n- none: 0 routes mounted.");
        }
        let modules: BTreeSet<&str> =
            self.routes.iter().map(|route| route.module.as_str()).collect();
        writeln!(
            f,
            "\n## Routes\n\n- {} routes under {} modules; {} probes, a route whose module's \
             requirements state no method and path.",
            self.routes.len(),
            modules.len(),
            self.probes()
        )?;
        for module in modules {
            let of: Vec<String> = self
                .routes
                .iter()
                .filter(|route| route.module == module)
                .map(|route| {
                    let probe = if route.probe { " (probe)" } else { "" };
                    format!("`{} {}`{probe}", route.method, route.path)
                })
                .collect();
            writeln!(f, "  - `{module}`: {}", of.join(", "))?;
        }
        Ok(())
    }
}

// The routes a tree mounts, each marked a probe where the merged slice
// owning its module has no requirement stating a method and path.
pub fn routes(
    mounted: Vec<(String, String, String)>, slices: &[Integrated], requirements: &[Value],
) -> Vec<Route> {
    mounted
        .into_iter()
        .map(|(module, method, path)| {
            let probe = slice_of(slices, &module)
                .is_some_and(|slice| !states_route(&slice.requirements, requirements));
            Route {
                module,
                method,
                path,
                probe,
            }
        })
        .collect()
}

// The merged slice whose module `crates/<stem>/src/<part>.rs` is: the one
// named `<stem>-<part>`, or `<stem>` where the part is the stem itself, or
// `<part>` where the slice's name went in whole under another crate, as
// the bootstrap's under `start`; the module spells the name's hyphens as
// underscores.
fn slice_of<'a>(slices: &'a [Integrated], module: &str) -> Option<&'a Integrated> {
    let rest = module.strip_prefix("crates/")?;
    let (stem, rest) = rest.split_once("/src/")?;
    let part = rest.strip_suffix(".rs")?;
    let stem = stem.replace('-', "_");
    let name = if part == stem { stem } else { format!("{stem}_{part}") };
    let named = |wanted: &str| slices.iter().find(|slice| slice.name.replace('-', "_") == wanted);
    named(&name).or_else(|| named(part))
}

// Whether any of the requirements `ids` name, among `requirements`, states
// an HTTP surface: the typed `trigger` the engine lowers from the survey's
// contract where it is an `http` route, else a method in capitals followed
// by a path, `GET /orders/{id}`, in backticks or bare, in its body or in a
// scenario's given, when, then, or and, which an older revision spells it by.
fn states_route(ids: &[String], requirements: &[Value]) -> bool {
    const METHODS: [&str; 7] = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
    let texts = |value: &Value| -> Vec<String> {
        match value {
            Value::String(text) => vec![text.clone()],
            Value::Array(items) => {
                items.iter().filter_map(Value::as_str).map(str::to_owned).collect()
            }
            _ => Vec::new(),
        }
    };
    let named: Vec<&Value> = requirements
        .iter()
        .filter(|requirement| {
            requirement["id"].as_str().is_some_and(|id| ids.iter().any(|x| x == id))
        })
        .collect();
    if named.iter().any(|requirement| requirement["trigger"]["http"]["method"].is_string()) {
        return true;
    }
    named
        .into_iter()
        .flat_map(|requirement| {
            let mut lines = texts(&requirement["body"]);
            for scenario in requirement["scenarios"].as_array().into_iter().flatten() {
                for clause in ["given", "when", "then", "and"] {
                    lines.extend(texts(&scenario[clause]));
                }
            }
            lines
        })
        .any(|line| {
            let words: Vec<&str> = line
                .split(|c: char| c.is_whitespace() || c == '`')
                .filter(|w| !w.is_empty())
                .collect();
            words.windows(2).any(|pair| METHODS.contains(&pair[0]) && pair[1].starts_with('/'))
        })
}

impl Display for Fidelity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "# Fidelity\n\nTree: {}.\n\n## The host's checks\n", self.tree)?;
        writeln!(f, "| check | exit | note |\n| --- | --- | --- |")?;
        for check in &self.checks {
            let exit = check.exit.map_or_else(|| "unrun".to_owned(), |code| code.to_string());
            writeln!(f, "| `{}` | {exit} | {} |", check.name, check.summary())?;
        }
        writeln!(
            f,
            "\n## Tests\n\n- {} test names in the tree; `{}` reports {} passed, {} failed.",
            self.tests,
            self.target.runner(),
            self.passed,
            self.failed
        )?;

        // the scenarios, by stem
        let total = self.scenarios.len();
        let exact = self.scenarios.iter().filter(|s| s.named == Some(Named::Exact)).count();
        let unknown = self.scenarios.iter().filter(|s| s.unknown).count();
        writeln!(
            f,
            "\n## Scenarios named by a test\n\n- {}/{total} scenarios of the requirements the \
             merged slices covered have a test named by them: {exact} by the scenario's name \
             exactly, {} by a test name containing it or contained in it; an uncovered \
             requirement's scenarios are not counted.\n- {unknown} scenarios have `then \
             [unknown]`.\n",
            self.named(),
            self.named() - exact,
        )?;
        writeln!(
            f,
            "| stem | scenarios | exact | fuzzy | then [unknown] |\n| --- | --- | --- | --- | --- |"
        )?;
        let stems: BTreeSet<&str> = self.scenarios.iter().map(|s| s.stem.as_str()).collect();
        for stem in stems {
            let of: Vec<&Scenario> = self.scenarios.iter().filter(|s| s.stem == stem).collect();
            writeln!(
                f,
                "| `{stem}` | {} | {} | {} | {} |",
                of.len(),
                of.iter().filter(|s| s.named == Some(Named::Exact)).count(),
                of.iter().filter(|s| s.named == Some(Named::Fuzzy)).count(),
                of.iter().filter(|s| s.unknown).count(),
            )?;
        }

        // the files
        listed(f, "## Modules the lists do not reach", "modules no list names", &self.unreached)?;
        self.routes_section(f)?;
        listed(f, "## Files outside the layout", "tracked files outside it", &self.outside)?;
        listed(f, "## What the checks wrote", "paths besides the lockfile", &self.wrote)?;

        // the reference
        if let Some(comparison) = &self.reference {
            write!(f, "{comparison}")?;
        }
        Ok(())
    }
}

// A section counting `paths` and listing them, `none` where there are none.
fn listed(f: &mut fmt::Formatter<'_>, heading: &str, what: &str, paths: &[String]) -> fmt::Result {
    if paths.is_empty() {
        return writeln!(f, "\n{heading}\n\n- none: 0 {what}.");
    }
    writeln!(f, "\n{heading}\n\n- {} {what}:", paths.len())?;
    for path in paths {
        writeln!(f, "  - `{path}`")?;
    }
    Ok(())
}

// One command of the host's toolchain run in `at`, its stdout and stderr
// read together for the note.
#[expect(clippy::disallowed_types, reason = "the eval runs the host's toolchain over the tree")]
pub fn host_check(at: &Path, command: &'static str, args: &[&str]) -> HostCheck {
    let mut words = command.split(' ');
    let program = words.next().unwrap_or(command);
    let output = std::process::Command::new(program)
        .args(words)
        .args(args)
        .current_dir(at)
        .env_remove("CURSOR_API_KEY")
        .output();
    match output {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            let mut tail: Vec<&str> =
                text.lines().filter(|line| !line.trim().is_empty()).rev().take(12).collect();
            tail.reverse();
            HostCheck {
                name: command,
                exit: output.status.code(),
                note: tail.join("\n"),
                output: text,
            }
        }
        Err(error) => HostCheck {
            name: command,
            exit: None,
            output: String::new(),
            note: format!("not run: {error}"),
        },
    }
}

pub fn unrun(command: &'static str, why: &str) -> HostCheck {
    HostCheck {
        name: command,
        exit: None,
        output: String::new(),
        note: format!("not run: {why}"),
    }
}

// The `# pass N` and `# fail N` lines of a TAP summary.
pub fn tap_counts(note: &str) -> (usize, usize) {
    let count = |key: &str| {
        note.lines()
            .rev()
            .filter_map(|line| line.trim().strip_prefix(key))
            .find_map(|rest| rest.trim().parse::<usize>().ok())
            .unwrap_or(0)
    };
    (count("# pass"), count("# fail"))
}

// The `N passed` and `M failed` of every `test result:` line cargo's runner
// prints, one per test binary, summed.
pub fn cargo_test_counts(output: &str) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    for line in output.lines().filter_map(|line| line.trim().strip_prefix("test result:")) {
        for field in line.split(';') {
            let words: Vec<&str> = field.split_whitespace().collect();
            if let [.., count, kind] = words.as_slice()
                && let Ok(count) = count.parse::<usize>()
            {
                match *kind {
                    "passed" => passed += count,
                    "failed" => failed += count,
                    _ => {}
                }
            }
        }
    }
    (passed, failed)
}

// Every `#[test]` or `#[tokio::test]` fn under `dir`, read from the files'
// text and spelled as a scenario, its underscores as spaces.
fn rust_test_names(dir: &Path, names: &mut Vec<String>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            rust_test_names(&path, names)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            let text = fs::read_to_string(&path)?;
            names.extend(declared_rust_tests(&text));
        }
    }
    Ok(())
}

pub fn declared_rust_tests(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line != "#[test]" && !line.starts_with("#[tokio::test") {
            continue;
        }
        // the fn head follows, past any further attribute
        let head = lines.by_ref().find(|line| !line.starts_with("#["));
        let Some(rest) = head.and_then(|head| head.split_once("fn ")) else {
            continue;
        };
        let name: String =
            rest.1.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !name.is_empty() {
            names.push(name.replace('_', " "));
        }
    }
    names
}

// Every `test("…")` and `it("…")` name under `dir`, read from the files'
// text; a name over a template or an expression is passed over.
fn test_names(dir: &Path, names: &mut Vec<String>) -> io::Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            test_names(&path, names)?;
        } else if path.extension().is_some_and(|ext| ext == "ts") {
            let text = fs::read_to_string(&path)?;
            names.extend(declared_tests(&text));
        }
    }
    Ok(())
}

pub fn declared_tests(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for opener in ["test(", "it("] {
        for (at, _) in text.match_indices(opener) {
            let led = text[..at].chars().next_back();
            if led.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '.') {
                continue;
            }
            let rest = text[at + opener.len()..].trim_start();
            let Some(quote) = rest.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
                continue;
            };
            if let Some(end) = rest[1..].find(quote) {
                names.push(rest[1..=end].to_owned());
            }
        }
    }
    names
}

// The test named by a scenario: one spelled exactly as it is, else one that
// contains it or that it contains, both read as words — without case, every
// run of non-alphanumeric characters one space, so `Per-probe budget` names
// `per_probe_budget`; a short name is never contained, since a word is not a
// scenario.
pub fn named_by(scenario: &str, tests: &[String]) -> Option<Named> {
    let scenario = words_of(scenario);
    if scenario.is_empty() {
        return None;
    }
    let lowered: Vec<String> = tests.iter().map(|test| words_of(test)).collect();
    if lowered.contains(&scenario) {
        return Some(Named::Exact);
    }
    let fuzzy = lowered.iter().any(|test| {
        (test.len() >= 12 && scenario.contains(test.as_str()))
            || (scenario.len() >= 12 && test.contains(scenario.as_str()))
    });
    fuzzy.then_some(Named::Fuzzy)
}

fn words_of(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

// Every file beneath `dir` as a `/`-separated path relative to `root`, the
// checks' `scratch` directory and `.git` passed over.
pub fn files_under(
    root: &Path, dir: &Path, scratch: &str, found: &mut Vec<String>,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == scratch || name == ".git" {
            continue;
        }
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            files_under(root, &path, scratch, found)?;
        } else if let Ok(relative) = path.strip_prefix(root) {
            let parts: Vec<String> = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            found.push(parts.join("/"));
        }
    }
    Ok(())
}
