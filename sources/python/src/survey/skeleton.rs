//! What a parsed tree states on its own, copied rather than restated.
//!
//! The briefs carry what the code spells, so the model draws from it rather
//! than paraphrase:
//!
//! - every exported class, type alias, and functional type as a `type`
//!   claim, its declaration verbatim with bodies elided
//! - every value the code spells as its own, a boundary at its line, so a
//!   criterion cites one or is none
//! - every package imported with the names bound to it, and every call made
//!   through one under its callee, so a `call` is drawn from the code's own
//!   calls
//! - every point where the code decides, so a requirement anchors where a
//!   behaviour starts rather than where a value is wired
//! - every behaviour the tree's own tests state, so what the code confirms
//!   is claimed and what it does not hold is not invented
//!
//! The decision points, the `return`s, the heads of functions and methods,
//! the calls through packages, the boundaries, and the surfaces' own lines
//! are the seam's anchors, which the SDK holds every `requirement` to. Each
//! section builder answers `None` where it has nothing to list.

use std::collections::{BTreeMap, BTreeSet};

use emery_sdk::serde_json::{Map, Value};
use emery_sdk::survey::Lines;
use emery_sdk::survey::code::{
    BindingKind, Call, Imported, Init, MemberKind, Scope, TypeKind, Use,
};
use emery_sdk::survey::resolve::Target;
use emery_sdk::survey::tests::Statement;
use emery_sdk::{Claim, ClaimKind};

use super::parse::Module;
use super::surface::{Surface, TRACE, Tree};
use super::{DIALECT, grouped, push_unique};

// How many sites a callee is listed at before the rest are counted.
const SITES: usize = 8;

// Methods that declare a command-line option or argument, whose `default`
// is a boundary.
const OPTIONS: &[&str] =
    &["add_argument", "add_option", "option", "argument", "Option", "Argument"];

// What a line `statements` lists is, for the stated-behaviours section.
pub const DESCRIBED: &str = "a test's docstring or name under its class's";

pub fn statements(module: &Module) -> Vec<Statement> {
    module
        .tests
        .iter()
        .map(|test| {
            let title = test.doc.clone().unwrap_or_else(|| {
                test.name.trim_start_matches("test").trim_start_matches('_').replace('_', " ")
            });
            let title =
                if test.parametrized { format!("{title} (over several values)") } else { title };
            let text = match &test.class {
                Some(class) => format!("{} › {title}", class.trim_start_matches("Test")),
                None => title,
            };
            Statement {
                text,
                line: test.lines.start,
            }
        })
        .collect()
}

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
        "Decision points in these modules, each at its lines: \
         the guards, matches, conditionals, raises, excepts, loop conditions, assertions, and \
         timers the code turns on. \
         A `requirement` anchors where a behaviour starts or its result is decided — at one of \
         these, at a `return`, at the line that opens the function or method whose whole body \
         is the behaviour, at a listed call or a package's construction, at a boundary or a \
         line that applies a named constant, at a step the function takes into the tree (a \
         write, a delete, a publish, a lookup it awaits), at the code a stated behaviour names, \
         or at a surface's registration or handler lines; a line that only wires or assigns — \
         a value passed on, a field set, a value computed from the ones in hand — is no \
         requirement's anchor, and what `start` constructs with a boundary's value is one \
         requirement at that construction:\n\n{}",
        lines.join("\n")
    ))
}

pub fn data<'m>(modules: impl IntoIterator<Item = &'m Module>) -> Option<String> {
    let files = grouped(modules.into_iter().flat_map(|module| {
        module.data().into_iter().map(move |path| (path, module.path.as_str()))
    }));
    if files.is_empty() {
        return None;
    }
    let lines: Vec<String> = files
        .iter()
        .map(|(path, by)| {
            let by: Vec<String> = by.iter().map(|module| format!("`{module}`")).collect();
            format!("- `{path}` — named by {}", by.join(", "))
        })
        .collect();
    Some(format!(
        "Data files these modules name by path, each with the modules reading it, laid after the \
         first module naming it — a large one after every module:\n\n{}",
        lines.join("\n")
    ))
}

// `widened` says the modules after the closure are the rest of the tree,
// laid for what these imports name.
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

// `anchored` is false for an inline value, which has no path to cite.
pub fn types<'m>(modules: impl IntoIterator<Item = &'m Module>, anchored: bool) -> Vec<Claim> {
    let mut claims = Vec::new();
    for module in modules {
        let anchor = |lines: Lines| anchored.then(|| format!("{}#{}", module.path, lines.anchor()));
        for decl in module.types.iter().filter(|decl| decl.exported) {
            let what = match decl.kind {
                TypeKind::Interface => "interface",
                TypeKind::Alias => "type alias",
                TypeKind::Enum => "enum",
                TypeKind::Functional => "type",
            };
            claims.push(claim(&decl.name, &decl.text, anchor(decl.lines), what));
        }
        for class in module.classes.iter().filter(|class| class.exported) {
            let mut signature = class.header.clone();
            for member in &class.members {
                let terminator = match member.kind {
                    MemberKind::Field => "",
                    MemberKind::Constructor
                    | MemberKind::Method
                    | MemberKind::Getter
                    | MemberKind::Setter
                    | MemberKind::Nested => ": ...",
                };
                signature.push_str("\n    ");
                signature.push_str(&member.signature);
                signature.push_str(terminator);
            }
            if class.members.is_empty() {
                signature.push_str("\n    ...");
            }
            let what = if super::parse::is_enum(class) { "enum" } else { "class" };
            claims.push(claim(&class.name, &signature, anchor(class.lines), what));
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

// `FIVE_SEC_DELAY`, never `count` or `i`: a function's working variables
// stay out of the list.
fn constant_name(name: &str) -> bool {
    name.len() > 1
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && name.chars().any(|c| c.is_ascii_uppercase())
}

// The bindings whose initializer spells a value of its own. A function local
// counts only where it is constant-named or an enumeration written out in
// place (`allowed_types = ["image/jpeg", "image/png"]`). A class field is
// named `Class.field`.
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
                || (*init == Init::Literal && head.starts_with(DIALECT.openers)) =>
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

fn defaulted(module: &Module) -> impl Iterator<Item = (String, &str, Lines)> {
    module.calls.iter().filter_map(|call| {
        if !OPTIONS.contains(&call.method()) {
            return None;
        }
        let default = call.keyword("default")?;
        let name = call.literal().map_or_else(|| call.method().to_owned(), str::to_owned);
        Some((format!("{name} default"), default.head.as_str(), call.lines))
    })
}

// Every anchor in the claim `path` grammar, once. A function's or method's
// head is its opening line, or the first decorator through the `def` line.
// A class field is declared policy (`permission_classes = (..)`), whatever
// initialises it. A line applying one of the tree's constant-named
// boundaries anchors wherever the constant is spelled, so the constants are
// read from the whole tree, not the seam's files.
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
            match binding.kind {
                BindingKind::Function
                | BindingKind::Value {
                    init: Init::Function, ..
                } => push(&module.path, head(binding.lines)),
                BindingKind::Field { .. } => push(&module.path, binding.lines),
                _ => {}
            }
        }
        for member in module.classes.iter().flat_map(|class| &class.members) {
            if !matches!(member.kind, MemberKind::Field | MemberKind::Nested) {
                push(&module.path, head(member.lines));
            }
        }
        for decorated in &module.decorated {
            push(&module.path, decorated.head);
        }
        for call in &module.calls {
            if callee(tree, module, call).is_some() {
                push(&module.path, call.lines);
            }
        }
        for (_, _, lines) in spelled(module).chain(defaulted(module)) {
            push(&module.path, lines);
        }
        for read in &module.env {
            push(&module.path, read.lines);
        }
    }
    anchors
}

// A call for its effect alone (`self.sessions.delete_by_user(..)`) or one
// awaited (`await self.customers.get(id)`), into the tree or on a member of
// its class. Not a registration handed a handler, a structural call, or a
// call at module level.
fn step(tree: &Tree, module: &Module, call: &Call) -> bool {
    if call.value == Use::Consumed || call.frames.is_empty() || call.structural(&DIALECT) {
        return false;
    }
    if call.args.iter().any(|arg| tree.handler(module, arg, &call.frames)) {
        return false;
    }
    of_tree(module, call.callee.head.as_str(), &call.frames, TRACE)
}

// Traced through the bindings that initialise it, as a package receiver is:
// the local holding `OrdersRepository(..)`, the parameter typed
// `mailer: Mailer`, the local a member of the class made
// (`serializer = self.InputSerializer(..)`).
fn of_tree(module: &Module, name: &str, frames: &[u32], budget: usize) -> bool {
    if budget == 0 {
        return false;
    }
    if name == DIALECT.self_name {
        return true;
    }
    let Some(binding) = module.binding(name, frames) else {
        return module.imported(name).is_some();
    };
    let heads = |type_path: &Option<Vec<String>>, root: &Option<Vec<String>>| {
        type_path
            .iter()
            .chain(root)
            .filter_map(|path| path.first())
            .any(|head| of_tree(module, head, frames, budget - 1))
    };
    match &binding.kind {
        BindingKind::Class { .. }
        | BindingKind::Function
        | BindingKind::Value {
            init: Init::Function, ..
        } => true,
        BindingKind::Value { type_path, root, .. } => heads(type_path, root),
        BindingKind::Param { type_path } => heads(type_path, &None),
        BindingKind::Field { .. } => false,
    }
}

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
        for (name, head, at) in defaulted(module) {
            spoken.push(at);
            lines.push(format!("- `{}#{}` — `{name} = {head}`", module.path, at.anchor()));
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
                "- `{}#{}` — env `{}` in `{text}`",
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

// A value written over several lines, as one.
fn collapsed(text: &str, at: Lines) -> Option<String> {
    const COLLAPSED: usize = 400;
    let start = at.start.saturating_sub(1) as usize;
    let count = (at.end.saturating_sub(at.start) + 1) as usize;
    let joined = text
        .lines()
        .skip(start)
        .take(count)
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !DIALECT.comment_prefixes.iter().any(|prefix| line.starts_with(prefix))
        })
        .collect::<Vec<_>>()
        .join(" ");
    let value = joined.split_once(" = ").map_or(joined.as_str(), |(_, value)| value);
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(match value.char_indices().nth(COLLAPSED) {
        Some((cut, _)) => format!("{}…", value[..cut].trim_end()),
        None => value.to_owned(),
    })
}

pub fn packages<'m>(modules: impl IntoIterator<Item = &'m Module>) -> Option<String> {
    let mut packages: BTreeMap<&str, (Vec<String>, Vec<&str>)> = BTreeMap::new();
    for module in modules {
        for import in module.imports.iter().filter(|import| !import.type_only) {
            if !matches!(import.target, Some(Target::Package(_))) {
                continue;
            }
            let bound = match &import.imported {
                Imported::Whole if import.local == import.specifier => {
                    format!("`{}`", import.local)
                }
                Imported::Whole => format!("the module as `{}`", import.local),
                Imported::Default => format!("`{}` (default)", import.local),
                Imported::Named(name) if *name == import.local => format!("`{name}`"),
                Imported::Named(name) => format!("`{name}` as `{}`", import.local),
                Imported::Star => "`*`".to_owned(),
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

// Left out as structure rather than a call:
// - a construction or a structural call
// - a call another in its chain is made on
// - a call handed nothing whose value is used in place
// - a registration or hook handed a handler
// - a call in a module-level declaration or a class body outside any function
pub fn calls(tree: &Tree, files: &[String]) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for module in files.iter().filter_map(|path| tree.modules.get(path)) {
        let declared = |call: &Call| {
            module
                .bindings
                .iter()
                .any(|binding| binding.scope == Scope::Module && binding.lines.contains(call.lines))
        };
        let sites = grouped(module.calls.iter().filter_map(|call| {
            if call.constructs
                || call.structural(&DIALECT)
                || call.inner
                || (call.args.is_empty() && call.value == Use::Consumed)
                || call.args.iter().any(|arg| tree.handler(module, arg, &call.frames))
                || (call.frames.is_empty() && (call.class.is_some() || declared(call)))
            {
                return None;
            }
            Some((callee(tree, module, call)?, call.lines))
        }));
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

// The `callee` a `call` claim spells:
// - `<package>:<members>` for a head imported from the package
//   (`requests:post`, `boto3:client`)
// - `<package>:<Head>.<members>` where the head is itself called first
//   (`httpx:Client.get` for `Client(..).get(..)`)
// - `<package>:<type or bound name>.<members>` for a head bound in the tree
//   (`sqlalchemy:Session.query`, `redis:Redis.get`)
// - a global as itself (`open`)
fn callee(tree: &Tree, module: &Module, call: &Call) -> Option<String> {
    let head = call.callee.head.as_str();
    let links: Vec<&str> = call.callee.links.iter().map(|link| link.name.as_str()).collect();
    let (bound, members) = if head == DIALECT.self_name {
        let [field, members @ ..] = links.as_slice() else { return None };
        (*field, members)
    } else {
        (head, links.as_slice())
    };
    let Some(receiver) = tree.receiver(module, call) else {
        let global = DIALECT.globals.contains(&head)
            && module.binding(head, &call.frames).is_none()
            && module.import(head).is_none();
        return global.then(|| {
            std::iter::once(head).chain(members.iter().copied()).collect::<Vec<_>>().join(".")
        });
    };
    let direct = head != DIALECT.self_name && module.package(head).is_some();
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
