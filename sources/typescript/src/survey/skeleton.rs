//! What a parsed tree states on its own, copied rather than restated.
//!
//! Every `interface`, `type`, `enum`, and class the seams' modules export is
//! a `type` claim with its declaration verbatim — a class as its header and
//! member signatures, bodies elided — so the design's domain model is the
//! code's, not a paraphrase; a declaration a module keeps to itself is its
//! own business. Every value the code spells as its own — a
//! `process.env` read with the default beside it, a named constant, a
//! pattern — is a boundary the brief lists at its line, so a criterion cites
//! one or is none. Every package the modules import is listed with the names
//! bound to it, and every call the modules make through one is listed under
//! its callee, so a `call` is drawn from the code's own calls and its
//! `callee` spelled from the import. Every point where the code decides — a
//! guard, a switch, a throw, a catch, a timer — is listed at its lines, so a
//! requirement anchors where a behaviour starts rather than where a value is
//! wired; those points, the `return`s, the heads of the functions and
//! methods, the calls through packages, the boundaries, and the surfaces'
//! own lines are the seam's anchors, which the SDK holds every
//! `requirement` to. Every behaviour the tree's own tests state — a test's
//! nested titles, a feature's scenarios — is listed at its line, so what the
//! code confirms of them is claimed and what it does not hold is not
//! invented.

use std::collections::{BTreeMap, BTreeSet};

use emery_sdk::serde_json::{Map, Value};
use emery_sdk::{Claim, ClaimKind};

use super::parse::{
    BindingKind, Call, Imported, Init, Lines, MemberKind, Module, Scope, TypeKind, Use,
};
use super::push_unique;
use super::resolve::Target;
use super::surface::{Surface, Tree};

// The runtime's own functions that leave the process, listed as calls with
// no package when nothing in the module binds the name.
const GLOBALS: &[&str] = &["fetch"];

// How many sites a callee is listed at before the rest are counted.
const SITES: usize = 8;

// The calls a test framework groups cases under, and the ones that state a
// case; `context` and `specify` are mocha's spellings.
const SUITES: &[&str] = &["describe", "context", "suite"];
const CASES: &[&str] = &["it", "test", "specify"];

// The modifiers a suite or case still runs under; `skip`, `todo`, and
// `each` state nothing the run holds.
const RUNS: &[&str] = &["only", "concurrent", "sequential"];

// The Gherkin keywords that open a feature and a scenario.
const FEATURE: &str = "Feature:";
const SCENARIOS: &[&str] = &["Scenario Outline:", "Scenario Template:", "Scenario:", "Example:"];

/// A test file of the tree, read for what it states.
#[derive(Debug)]
pub struct Test {
    pub path: String,
    /// The modules of the tree it imports — for a feature file, the ones
    /// the step modules beside it import.
    pub imports: Vec<String>,
    pub statements: Vec<Statement>,
}

/// One behaviour a test states: a case's title under its suites' titles,
/// or a scenario's under its feature's, at the line that states it.
#[derive(Debug)]
pub struct Statement {
    pub text: String,
    pub line: u32,
}

/// The behaviours a parsed test module states: each `it` or `test` case
/// led by a string, under the `describe` suites whose bodies enclose it,
/// outermost first; a skipped, pending, or parameterised one states
/// nothing the run holds.
pub fn statements(module: &Module) -> Vec<Statement> {
    let runs = |call: &Call| {
        call.callee.head_call.is_none()
            && call
                .callee
                .links
                .iter()
                .all(|link| link.call.is_none() && RUNS.contains(&link.name.as_str()))
    };
    let mut suites: Vec<(&str, Lines)> = Vec::new();
    let mut cases: Vec<(&str, Lines)> = Vec::new();
    for call in module.calls.iter().filter(|call| runs(call)) {
        let Some(title) = call.literal() else { continue };
        let head = call.callee.head.as_str();
        if SUITES.contains(&head) {
            if let Some(body) = call.args.iter().find(|arg| arg.function) {
                suites.push((title, body.lines));
            }
        } else if CASES.contains(&head) {
            cases.push((title, call.lines));
        }
    }
    cases
        .into_iter()
        .map(|(title, lines)| {
            let mut titles: Vec<&str> =
                suites.iter().filter(|(_, body)| body.contains(lines)).map(|(t, _)| *t).collect();
            titles.push(title);
            Statement {
                text: titles.join(" › "),
                line: lines.start,
            }
        })
        .collect()
}

/// The behaviours a Gherkin feature file states: each scenario's title
/// under its feature's, at the line that opens it.
pub fn scenarios(text: &str) -> Vec<Statement> {
    let mut feature: Option<&str> = None;
    let mut statements = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if let Some(title) = line.strip_prefix(FEATURE) {
            feature = Some(title.trim());
            continue;
        }
        let Some(title) = SCENARIOS.iter().find_map(|keyword| line.strip_prefix(keyword)) else {
            continue;
        };
        let title = title.trim();
        statements.push(Statement {
            text: feature
                .map_or_else(|| title.to_owned(), |feature| format!("{feature} › {title}")),
            line: u32::try_from(index + 1).unwrap_or(u32::MAX),
        });
    }
    statements
}

/// The brief's decision points over `modules`: each guard, switch,
/// conditional, throw, catch, and timer at its lines with its text and the
/// function it runs in, under the rule of where a requirement anchors.
/// `None` when the modules decide nothing.
pub fn decisions<'m>(modules: impl IntoIterator<Item = &'m Module>) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for module in modules {
        for decision in &module.decisions {
            let within = match (&decision.class, &decision.function) {
                (Some(class), Some(function)) => format!(" in `{class}.{function}`"),
                (None, Some(function)) => format!(" in `{function}`"),
                (Some(class), None) => format!(" in `{class}`"),
                (None, None) => " at module level".to_owned(),
            };
            lines.push(format!(
                "- `{}#{}` — `{}`{within}",
                module.path,
                decision.lines.anchor(),
                decision.text
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Decision points in these modules, each at its lines: the guards, switches, conditionals, \
         throws, catches, and timers the code turns on. A `requirement` anchors where a behaviour \
         starts or its result is decided — at one of these, at a `return`, at the line that opens \
         the function or method whose whole body is the behaviour, at a listed call or a \
         package's construction, at a boundary or a line that applies a named constant, at a step \
         the function takes into the tree (a write, a delete, a publish, a lookup it awaits), at \
         the code a stated behaviour names, or at a surface's registration or handler lines; a \
         line that only wires or assigns — a value passed on, a field set, a value computed from \
         the ones in hand — is no requirement's anchor, and what `start` constructs with a \
         boundary's value is one requirement at that construction:\n\n{}",
        lines.join("\n")
    ))
}

/// The brief's stated behaviours over `tests`: each statement at its line.
/// `None` when they state none.
pub fn stated<'t>(tests: impl IntoIterator<Item = &'t Test>) -> Option<String> {
    let lines: Vec<String> = tests
        .into_iter()
        .flat_map(|test| {
            test.statements.iter().map(move |statement| {
                format!("- `{}#L{}` — {}", test.path, statement.line, statement.text)
            })
        })
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Behaviours the tree's own tests state, each at its line — a case's title under its \
         suites', a scenario's under its feature's. One the laid code confirms is a `requirement` \
         anchored at the code that exhibits it, named for that code, its statement the test's \
         made present tense; one the code does not hold is not invented. A test is read for what \
         it states and the values it asserts, and is no `requirement`'s or `criterion`'s \
         anchor:\n\n{}",
        lines.join("\n")
    ))
}

/// The brief's data files over `modules`: each `.json` they import, with the
/// modules importing it, in the order the modules are laid. `None` when they
/// import none.
pub fn data<'m>(modules: impl IntoIterator<Item = &'m Module>) -> Option<String> {
    // path → importing modules
    let mut files: Vec<(&str, Vec<&str>)> = Vec::new();
    for module in modules {
        for path in module.data() {
            match files.iter_mut().find(|(known, _)| *known == path) {
                Some((_, by)) => push_unique(by, module.path.as_str()),
                None => files.push((path, vec![module.path.as_str()])),
            }
        }
    }
    if files.is_empty() {
        return None;
    }
    let lines: Vec<String> = files
        .iter()
        .map(|(path, by)| {
            let by: Vec<String> = by.iter().map(|module| format!("`{module}`")).collect();
            format!("- `{path}` — imported by {}", by.join(", "))
        })
        .collect();
    Some(format!(
        "Data files these modules import, each with the modules reading it, laid after the \
         modules:\n\n{}",
        lines.join("\n")
    ))
}

/// What the resolver could not follow from `modules`: each relative or
/// aliased specifier naming nothing of the tree, with the module importing
/// it, and each module loaded by a computed name, at its lines — and, when
/// `widened`, that the modules after the closure are the rest of the tree,
/// laid so what those name is still within reach. `None` when every import
/// was followed.
pub fn unfollowed<'m>(
    modules: impl IntoIterator<Item = &'m Module>, widened: bool,
) -> Option<String> {
    let mut items: Vec<String> = Vec::new();
    for module in modules {
        let unresolved: Vec<String> =
            module.unresolved().into_iter().map(|s| format!("`{s}`")).collect();
        if !unresolved.is_empty() {
            items.push(format!(
                "{} from `{}` {} no module of the tree",
                unresolved.join(", "),
                module.path,
                if unresolved.len() == 1 { "names" } else { "name" }
            ));
        }
        if !module.dynamic.is_empty() {
            let at: Vec<String> = module.dynamic.iter().map(ToString::to_string).collect();
            items.push(format!(
                "`{}` loads a module by a computed name at {}",
                module.path,
                at.join(", ")
            ));
        }
    }
    if items.is_empty() {
        return None;
    }
    let consequence = if widened {
        " The modules after the closure are the rest of the tree, laid so what these name is \
         still within reach; read them for that alone."
    } else {
        ""
    };
    Some(format!(
        "The caller could not follow every import: {}. What these name is in none of the lists \
         above.{consequence}",
        items.join("; ")
    ))
}

/// The `type` claims of `modules`: one per exported declaration, its
/// declaration verbatim, anchored at its lines when `anchored` — a tree's
/// file, never an inline value.
pub fn types<'m>(modules: impl IntoIterator<Item = &'m Module>, anchored: bool) -> Vec<Claim> {
    let mut claims = Vec::new();
    for module in modules {
        let anchor = |lines: Lines| anchored.then(|| format!("{}#{}", module.path, lines.anchor()));
        for decl in module.types.iter().filter(|decl| decl.exported) {
            let what = match decl.kind {
                TypeKind::Interface => "interface",
                TypeKind::Alias => "type alias",
                TypeKind::Enum => "enum",
            };
            claims.push(claim(&decl.name, &decl.text, anchor(decl.lines), what));
        }
        for class in module.classes.iter().filter(|class| class.exported) {
            let mut signature = format!("{} {{", class.header);
            for member in &class.members {
                let terminator = match member.kind {
                    MemberKind::Field => ";",
                    MemberKind::Constructor
                    | MemberKind::Method
                    | MemberKind::Getter
                    | MemberKind::Setter => " { .. }",
                };
                signature.push_str("\n  ");
                signature.push_str(&member.signature);
                signature.push_str(terminator);
            }
            signature.push_str("\n}");
            claims.push(claim(&class.name, &signature, anchor(class.lines), "class"));
        }
    }
    claims
}

fn claim(name: &str, signature: &str, path: Option<String>, what: &str) -> Claim {
    Claim {
        kind: ClaimKind::Type,
        id: None,
        path,
        synopsis: Some(format!("exported {what}")),
        backing: None,
        extras: Map::from_iter([
            ("name".to_owned(), Value::String(name.to_owned())),
            ("signature".to_owned(), Value::String(signature.to_owned())),
        ]),
    }
}

// A local spelled as a constant is named for one: `FIVE_SEC_DELAY`, never
// `count` or `i`, so a function's working variables stay out of the list.
fn constant_name(name: &str) -> bool {
    name.len() > 1
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && name.chars().any(|c| c.is_ascii_uppercase())
}

// The bindings of `module` that spell a value of their own: each module-level
// binding, class field (named `Class.field`), and function local that is
// constant-named or an enumeration written out in place (`allowedMimeTypes =
// ['image/jpeg', 'image/png']`) whose initializer is a literal, a pattern, a
// `process.env` read, or a definition, with the initializer's head and the
// binding's lines.
fn spelled(module: &Module) -> impl Iterator<Item = (String, &str, Lines)> {
    module.bindings.iter().filter_map(|binding| {
        let (name, init, head) = match (&binding.scope, &binding.kind) {
            (
                Scope::Module,
                BindingKind::Value {
                    init,
                    head: Some(head),
                    ..
                },
            ) => (binding.name.clone(), *init, head),
            (
                Scope::Function(_),
                BindingKind::Value {
                    init,
                    head: Some(head),
                    ..
                },
            ) if constant_name(&binding.name)
                || (*init == Init::Literal && head.starts_with(['[', '{'])) =>
            {
                (binding.name.clone(), *init, head)
            }
            (
                Scope::Class(class),
                BindingKind::Field {
                    init,
                    head: Some(head),
                    ..
                },
            ) => (format!("{class}.{}", binding.name), *init, head),
            _ => return None,
        };
        init.is_value().then_some((name, head.as_str(), binding.lines))
    })
}

/// The lines a `requirement` of a seam over the modules at `files` of `tree`
/// may anchor at — where the code's behaviour starts or its result is
/// decided: every decision point, every `return` of a value, the head of
/// every function, method, getter, setter, and constructor — the line that
/// opens it, where a behaviour it computes whole is anchored — every call
/// made through a package or a global that leaves the process
/// (constructions, registrations, and lifecycle calls among them), every
/// boundary and `process.env` read, every line that applies one of the
/// tree's constant-named boundaries, every step a function takes into the
/// tree or on one of its class's members — a call for its effect alone or
/// one it awaits — and the registration or declaration lines of each of
/// `surfaces`; each in the claim `path` grammar, once.
pub fn anchors<'s>(
    tree: &Tree, files: &[String], surfaces: impl IntoIterator<Item = &'s Surface>,
) -> Vec<String> {
    let mut anchors: Vec<String> = Vec::new();
    let mut push =
        |path: &str, lines: Lines| push_unique(&mut anchors, format!("{path}#{}", lines.anchor()));
    let head = |lines: Lines| Lines {
        start: lines.start,
        end: lines.start,
    };
    // the constant-named boundaries of the whole tree: a line that applies
    // one, wherever it is spelled, is where a behaviour that depends on it
    // is decided
    let constants: BTreeSet<String> = tree
        .modules
        .values()
        .flat_map(spelled)
        .map(|(name, _, _)| name)
        .filter(|name| constant_name(name))
        .collect();
    for surface in surfaces {
        push(&surface.entry, surface.lines);
    }
    for module in files.iter().filter_map(|path| tree.modules.get(path)) {
        for line in module.referencing(|name| constants.contains(name)) {
            push(
                &module.path,
                Lines {
                    start: line,
                    end: line,
                },
            );
        }
        for call in module.calls.iter().filter(|call| step(tree, module, call)) {
            push(&module.path, call.lines);
        }
        for decision in &module.decisions {
            push(&module.path, decision.lines);
        }
        for lines in &module.returns {
            push(&module.path, *lines);
        }
        for binding in &module.bindings {
            if matches!(
                binding.kind,
                BindingKind::Function
                    | BindingKind::Value {
                        init: Init::Function,
                        ..
                    }
                    | BindingKind::Field {
                        init: Init::Function,
                        ..
                    }
            ) {
                push(&module.path, head(binding.lines));
            }
        }
        for member in module.classes.iter().flat_map(|class| &class.members) {
            if member.kind != MemberKind::Field {
                push(&module.path, head(member.lines));
            }
        }
        for call in &module.calls {
            if callee(tree, module, call).is_some() {
                push(&module.path, call.lines);
            }
        }
        for (_, _, lines) in spelled(module) {
            push(&module.path, lines);
        }
        for read in &module.env {
            push(&module.path, read.lines);
        }
    }
    anchors
}

// A step a function takes into a module of the tree or on one of its class's
// members: a call for its effect alone — an expression statement, its value
// discarded (`this.sessions.deleteByUserId(..)`): a write, a delete, a
// publish — or one it awaits (`await this.customers.get(id)`): a lookup, a
// fetch, where that step is claimed. Not a registration handed a handler, a
// lifecycle or mounting call, or a call at module level.
fn step(tree: &Tree, module: &Module, call: &Call) -> bool {
    if call.value == Use::Consumed || call.frames.is_empty() || call.structural() {
        return false;
    }
    if call.args.iter().any(|arg| tree.handler(module, arg, &call.frames)) {
        return false;
    }
    let head = call.callee.head.as_str();
    head == "this"
        || matches!(
            module.import(head).and_then(|import| import.target.as_ref()),
            Some(Target::Module(_))
        )
}

/// The brief's boundaries over `modules`: each module-level binding, class
/// field, and constant-named or enumerating function local whose initializer
/// spells a value of its own — a literal, a pattern, a `process.env` read, a
/// definition — and each `process.env` read no such binding holds, at its
/// lines as the code writes it, a value written over several lines collapsed
/// to one. `None` when they spell none.
pub fn boundaries<'m>(modules: impl IntoIterator<Item = &'m Module>) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for module in modules {
        let mut spoken: Vec<Lines> = Vec::new();
        for (name, head, at) in spelled(module) {
            spoken.push(at);
            let value = if at.end > at.start { collapsed(&module.text, at) } else { None };
            let value = value.as_deref().unwrap_or(head);
            lines.push(format!("- `{}#{}` — `{name} = {value}`", module.path, at.anchor()));
        }
        for read in &module.env {
            if spoken.iter().any(|lines| lines.contains(read.lines)) {
                continue;
            }
            spoken.push(read.lines);
            let text = module
                .text
                .lines()
                .nth(read.lines.start.saturating_sub(1) as usize)
                .unwrap_or_default()
                .trim();
            lines.push(format!(
                "- `{}#{}` — `process.env.{}` in `{text}`",
                module.path,
                read.lines.anchor(),
                read.key
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Boundaries the code spells as values of their own, each at its line. A `criterion` cites one of \
         these anchors and states its value; a comparison written into a branch is its requirement's \
         to state, and no criterion:\n\n{}",
        lines.join("\n")
    ))
}

// A value written over the lines `at` of `text`, collapsed to one line: the
// lines trimmed, comment lines dropped, joined by one space, what follows the
// first `=` kept, and cut at `COLLAPSED` characters. `None` when nothing is
// left.
fn collapsed(text: &str, at: Lines) -> Option<String> {
    const COLLAPSED: usize = 400;
    let start = at.start.saturating_sub(1) as usize;
    let count = (at.end.saturating_sub(at.start) + 1) as usize;
    let joined = text
        .lines()
        .skip(start)
        .take(count)
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('*'))
        .collect::<Vec<_>>()
        .join(" ");
    let value = joined.split_once(" = ").map_or(joined.as_str(), |(_, value)| value);
    let value = value.trim().trim_end_matches(';').trim_end();
    if value.is_empty() {
        return None;
    }
    Some(match value.char_indices().nth(COLLAPSED) {
        Some((cut, _)) => format!("{}…", value[..cut].trim_end()),
        None => value.to_owned(),
    })
}

/// The brief's packages over `modules`: each bare specifier they import, with
/// the names bound to it and the modules binding them. `None` when they
/// import none.
pub fn packages<'m>(modules: impl IntoIterator<Item = &'m Module>) -> Option<String> {
    // specifier → (bound names, importing modules)
    let mut packages: BTreeMap<&str, (Vec<String>, Vec<&str>)> = BTreeMap::new();
    for module in modules {
        for import in module.imports.iter().filter(|import| !import.type_only) {
            if !matches!(import.target, Some(Target::Package(_))) {
                continue;
            }
            let bound = match &import.imported {
                Imported::Default => format!("`{}` (default)", import.local),
                Imported::Namespace => format!("`{}` (namespace)", import.local),
                Imported::Named(name) if *name == import.local => format!("`{name}`"),
                Imported::Named(name) => format!("`{name}` as `{}`", import.local),
                Imported::Effect => continue,
            };
            let (names, paths) = packages.entry(import.specifier.as_str()).or_default();
            push_unique(names, bound);
            push_unique(paths, module.path.as_str());
        }
    }
    if packages.is_empty() {
        return None;
    }
    let lines: Vec<String> = packages
        .iter()
        .map(|(specifier, (bound, paths))| {
            let paths: Vec<String> = paths.iter().map(|path| format!("`{path}`")).collect();
            format!("- `{specifier}` — {} — in {}", bound.join(", "), paths.join(", "))
        })
        .collect();
    Some(format!(
        "Packages these modules import, with the names bound to them. What leaves the process goes \
         through one of these; a call into a module of the tree is followed, not claimed:\n\n{}",
        lines.join("\n")
    ))
}

/// The brief's calls over the modules at `files` of `tree`: each call made
/// through a package — its receiver traced to the import, or a global that
/// leaves the process — grouped by module and callee with the lines of its
/// sites, less what is structure rather than a call: a construction, a
/// lifecycle or mounting call, a registration or hook handed a handler, a
/// call within a module-level declaration or a class body outside any
/// function, a call another in its chain is made on, and a call handed
/// nothing. `None` when the modules make none.
pub fn calls(tree: &Tree, files: &[String]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for module in files.iter().filter_map(|path| tree.modules.get(path)) {
        let declared = |call: &Call| {
            module
                .bindings
                .iter()
                .any(|binding| binding.scope == Scope::Module && binding.lines.contains(call.lines))
        };
        // callee → the lines of its sites, in the order first seen
        let mut sites: Vec<(String, Vec<Lines>)> = Vec::new();
        for call in &module.calls {
            if call.is_new
                || call.structural()
                || call.inner
                || call.args.is_empty()
                || call.args.iter().any(|arg| tree.handler(module, arg, &call.frames))
                || (call.frames.is_empty() && (call.class.is_some() || declared(call)))
            {
                continue;
            }
            let Some(callee) = callee(tree, module, call) else { continue };
            match sites.iter_mut().find(|(known, _)| *known == callee) {
                Some((_, at)) => at.push(call.lines),
                None => sites.push((callee, vec![call.lines])),
            }
        }
        for (callee, at) in sites {
            let shown: Vec<String> = at.iter().take(SITES).map(|lines| lines.anchor()).collect();
            let more = at.len().saturating_sub(SITES);
            lines.push(format!(
                "- `{callee}` in `{}` at {}{}",
                module.path,
                shown.join(", "),
                if more == 0 { String::new() } else { format!(" +{more}") }
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Calls these modules make through a package, each callee with the lines of its sites. A \
         `call` is drawn from this list, its `callee` spelled as listed: a store, a broker, an HTTP \
         client, a cache, an identity provider, or a metric emission leaves the process; a \
         validator, a logger, or a framework helper does not, unless its emission is the surface's \
         contract. Every listed call that leaves the process has a requirement beside it:\n\n{}",
        lines.join("\n")
    ))
}

// The callee of a call through a package, as a `call` claim spells it: for
// a head imported from the package, `<package>:<members>` (`axios:post`,
// `@azure/identity:getAzureToken`), or `<package>:<Head>.<members>` when
// the head is itself called first (`pg:Client.query` for `new
// Client(..).query(..)`); for a head bound in the tree, `<package>:<type or
// bound name>.<members>` (`pg:Pool.query`, `pino:logger.info`); a global as
// itself (`fetch`).
fn callee(tree: &Tree, module: &Module, call: &Call) -> Option<String> {
    let head = call.callee.head.as_str();
    let links: Vec<&str> = call.callee.links.iter().map(|link| link.name.as_str()).collect();
    // a receiver in a field: the first link is the field
    let (bound, members) = match (head, links.as_slice()) {
        ("this", [field, members @ ..]) => (*field, members),
        ("this", []) => return None,
        (head, members) => (head, members),
    };
    let Some(receiver) = tree.receiver(module, call) else {
        let global = GLOBALS.contains(&head)
            && module.binding(head, &call.frames).is_none()
            && module.import(head).is_none();
        return global.then(|| {
            std::iter::once(head).chain(members.iter().copied()).collect::<Vec<_>>().join(".")
        });
    };
    let direct = head != "this" && module.package(head).is_some();
    let mut path: Vec<&str> = Vec::new();
    match &receiver.type_name {
        Some(type_name) => path.push(type_name),
        None if direct && call.callee.head_call.is_none() => {}
        None => path.push(bound),
    }
    path.extend_from_slice(members);
    if path.is_empty() {
        path.push(head);
    }
    Some(format!("{}:{}", receiver.package, path.join(".")))
}
