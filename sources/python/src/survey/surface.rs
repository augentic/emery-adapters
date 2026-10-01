//! Derives what the code says of a tree's surfaces from the accepted anchors.
//!
//! A surface is where control enters from outside the process: the
//! bootstrap, a handler handed to something a package provides, a `def` or
//! class under a package's decorator, or, in a library, what an entry
//! module exports. The survey names each at the lines that register or
//! declare it. From those lines the code reads the stem a route or literal
//! spells, the id that tells the surface from the others under its stem, a
//! class's methods, and the modules the surface reaches, so two runs that
//! accept the same anchors lead their ids the same way and reach the same
//! modules.
//!
//! The rules read shapes, never a framework's name: a call handed a
//! function, a decorator with a literal, an export. Two exceptions spell
//! names. `DECORATOR_HOOKS` lists the decorators that shape a handler rather
//! than register it, which keeps a verb decorator beneath one of them the
//! surface. A module under `management/commands/` is read as a file-routed
//! framework's module is.

use std::collections::{BTreeMap, BTreeSet};

use super::parse::{
    Arg, Binding, BindingKind, Call, Decorated, Export, ExportKind, Imported, Init, Lines,
    MemberKind, Module, Scope, hooks, shapes,
};
use super::resolve::{Manifest, Resolver};
use super::{push_unique, unique};

// Looked for in this order when the manifest's scripts name no entry:
// Django's, a package run as a script, the conventional entries at the root
// and under `src/`, then the ones a top-level package holds.
const BOOTSTRAPS: &[&str] = &[
    "manage.py",
    "__main__.py",
    "*/__main__.py",
    "main.py",
    "app.py",
    "run.py",
    "server.py",
    "wsgi.py",
    "asgi.py",
    "src/main.py",
    "src/app.py",
    "src/run.py",
    "src/server.py",
    "src/wsgi.py",
    "src/asgi.py",
    "*/cli.py",
    "*/main.py",
    "*/app.py",
    "*/wsgi.py",
    "*/asgi.py",
];

const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options", "trace"];

// Methods and decorators that register a route with a path literal, whatever
// verb answers it.
const ROUTERS: &[&str] = &[
    "route",
    "api_route",
    "add_api_route",
    "add_url_rule",
    "add_route",
    "add_resource",
    "path",
    "re_path",
    "url",
    "websocket",
    "websocket_route",
    "add_websocket_route",
];

// Methods that mount what they are handed under a prefix: a blueprint, a
// router, a sub-application.
const MOUNTING: &[&str] = &["register_blueprint", "include_router", "mount"];

// Constructors whose `prefix` or `url_prefix` keyword mounts the module
// declaring them.
const PREFIXED: &[&str] = &["APIRouter", "Blueprint", "Router"];

// Path segments that version or namespace an API rather than name a surface.
const PATH_NOISE: &[&str] = &["api", "rest", "internal"];

// Decorators that hook what they decorate onto an application's or a
// signal's lifecycle rather than register a surface for a caller.
const DECORATOR_HOOKS: &[&str] = &[
    "connect",
    "receiver",
    "listens_for",
    "exception_handler",
    "errorhandler",
    "error_handler",
    "before_request",
    "after_request",
    "before_first_request",
    "middleware",
    "context_processor",
    "template_filter",
    "url_value_preprocessor",
    "url_defaults",
];

// Keywords a function is handed under as a hook on the thing being built,
// not as the handler it registers.
const HOOK_KEYWORDS: &[&str] = &[
    "lifespan",
    "on_startup",
    "on_shutdown",
    "exception_handlers",
    "default_factory",
    "default",
    "key",
    "callback",
    "dependencies",
    "middleware",
    "result_callback",
];

// How many bindings a receiver is traced through before it counts as local.
pub(super) const TRACE: usize = 4;

#[derive(Debug)]
pub struct Tree {
    // By root-relative path, each import settled.
    pub modules: BTreeMap<String, Module>,
    pub resolver: Resolver,
    // Empty where the tree has none.
    pub manifest: Manifest,
}

#[derive(Debug)]
pub struct Surface {
    // What a caller does through the surface, as the survey named it.
    pub name: String,
    // The module the surface is registered or declared in.
    pub entry: String,
    // What every `requirement` and `criterion` id of the surface leads with.
    pub stem: String,
    // The registration or declaration.
    pub lines: Lines,
    // Notes for the brief.
    pub detail: Vec<String>,
    // The modules the surface reaches, its entry first.
    pub closure: Vec<String>,
    // What tells the surface from the others under its stem: its handler's
    // name, its verb and the path past the resource, its method's name.
    pub discriminator: Option<String>,
    // The public methods of an exported or handed class, kebab-cased.
    pub methods: Vec<String>,
    // The stem alone for a stem's one surface, `<stem>.<discriminator>`
    // otherwise, and one more per method of a class.
    pub ids: Vec<String>,
}

#[derive(Debug)]
pub enum Runs {
    // A console script the manifest installs, by name, and the function it
    // calls.
    Script(String, String),
    Guard,
    Load,
}

impl Tree {
    // The first entry that runs something, in this order: the entries the
    // manifest's scripts name, the conventional entries the tree holds,
    // then the one module of a tree of one. An entry runs by a console
    // script whose function it declares or imports from a module of the
    // tree that does, by a `__main__` guard, or at load. An entry that only
    // declares is a library's.
    pub(super) fn bootstrap(&self) -> Option<(&Module, Runs)> {
        let scripts =
            self.manifest.entries(&self.resolver).into_iter().filter_map(|(entry, function)| {
                let module = self.modules.get(&entry)?;
                let script = self
                    .manifest
                    .scripts
                    .iter()
                    .find(|(_, target)| self.resolver.entry(target).as_deref() == Some(&entry))
                    .map(|(name, _)| name.clone());
                let runs = match (function, script) {
                    (Some(function), Some(script)) if self.tree_binding(module, &function) => {
                        Some(Runs::Script(script, function))
                    }
                    _ => self.runs_at_load(module),
                };
                Some((module, runs))
            });
        let conventional = BOOTSTRAPS
            .iter()
            .filter_map(|candidate| self.resolver.first(&[candidate]))
            .filter_map(|entry| self.modules.get(&entry))
            .map(|module| (module, self.runs_at_load(module)));
        let alone = (self.modules.len() == 1)
            .then(|| self.modules.values().next())
            .flatten()
            .map(|module| (module, self.runs_at_load(module)));
        scripts.chain(conventional).chain(alone).find_map(|(module, runs)| Some((module, runs?)))
    }

    // A module-level binding constructed from a package's class
    // (`app = FastAPI()`) or a function of the tree (`app = create_app()`)
    // runs at load, as a discarded call at module level does.
    fn runs_at_load(&self, module: &Module) -> Option<Runs> {
        if module.main_guard.is_some() {
            return Some(Runs::Guard);
        }
        let discards = module.calls.iter().any(|call| {
            call.discarded() && call.depth == 0 && call.function.is_none() && call.class.is_none()
        });
        let constructs = module.bindings.iter().any(|binding| {
            let (
                Scope::Module,
                BindingKind::Value {
                    root: Some(root),
                    init: Init::Construction | Init::Definition,
                    ..
                },
            ) = (&binding.scope, &binding.kind)
            else {
                return false;
            };
            let Some(head) = root.first() else { return false };
            let from_package = module.package(head).is_some()
                && root.last().is_some_and(|last| last.starts_with(char::is_uppercase));
            from_package || self.tree_function(module, head)
        });
        (discards || constructs).then_some(Runs::Load)
    }

    // A console script's target may reach its entry through an import from
    // the module that declares it.
    fn tree_binding(&self, module: &Module, name: &str) -> bool {
        module.binding(name, &[]).is_some()
            || self
                .exporter(module, name)
                .is_some_and(|(target, exported)| target.export(&exported).is_some())
    }

    fn tree_function(&self, module: &Module, name: &str) -> bool {
        if let Some(binding) = module.binding(name, &[]) {
            return matches!(binding.kind, BindingKind::Function);
        }
        self.exporter(module, name).is_some_and(|(target, exported)| {
            target.export(&exported).is_some_and(|export| export.kind == ExportKind::Function)
        })
    }

    // The exporting module and the name exported. For an import of a
    // submodule, the submodule and its own name.
    fn exporter(&self, module: &Module, local: &str) -> Option<(&Module, String)> {
        let import = module.import(local)?;
        let path = import.target.as_ref()?.module()?;
        let name = match &import.imported {
            Imported::Named(name) => name.clone(),
            Imported::Module => local.to_owned(),
            Imported::Star | Imported::Literal => return None,
        };
        Some((self.modules.get(path)?, name))
    }

    // `views.list_orders` under `from . import views`, `list_orders` under
    // `from .views import list_orders`, and `a.b.c.run` under `import a.b.c`
    // each name a member of a module of the tree.
    fn member_of(&self, module: &Module, root: &[String]) -> Option<(&Module, String)> {
        let [name, rest @ ..] = root else { return None };
        let import = module.import(name)?;
        let target = self.modules.get(import.target.as_ref()?.module()?)?;
        match (&import.imported, rest) {
            (Imported::Named(exported), []) if target.export(exported).is_some() => {
                Some((target, exported.clone()))
            }
            (Imported::Named(_), [member]) => Some((target, member.clone())),
            (Imported::Module, rest) => {
                let segments: Vec<&str> = import.specifier.split('.').collect();
                let bound =
                    if segments.first() == Some(&name.as_str()) { segments.len() - 1 } else { 0 };
                match rest {
                    [.., member] if rest.len() == bound + 1 => Some((target, member.clone())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    // `seeds` first in their order, then breadth-first, once each. A module
    // in `stop` is reached and not followed.
    pub(super) fn closure(&self, seeds: &[String], stop: &[String]) -> Vec<String> {
        let mut order: Vec<String> = Vec::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for seed in seeds.iter().filter(|seed| self.modules.contains_key(*seed)) {
            if seen.insert(seed.as_str()) {
                order.push(seed.clone());
            }
        }
        let mut next = 0;
        while next < order.len() {
            let from = self.modules.get(&order[next]).filter(|_| !stop.contains(&order[next]));
            next += 1;
            let Some(module) = from else { continue };
            for path in module.reached() {
                if seen.insert(path) {
                    order.push(path.to_owned());
                }
            }
        }
        order
    }

    // A literal, or a name bound to one here or in a module of the tree it
    // is imported from.
    fn constant(&self, module: &Module, arg: &Arg, frames: &[u32]) -> Option<String> {
        if let Some(literal) = &arg.literal {
            return Some(literal.clone());
        }
        if arg.called {
            return None;
        }
        let [name] = arg.root.as_deref()? else { return None };
        if let Some(binding) = module.binding(name, frames) {
            return match &binding.kind {
                BindingKind::Value { string, .. } => string.clone(),
                _ => None,
            };
        }
        let (target, imported) = self.exporter(module, name)?;
        match &exported_binding(target, &imported)?.kind {
            BindingKind::Value { string, .. } => string.clone(),
            _ => None,
        }
    }

    // A lambda, a view class handed by `as_view()`, a call carrying one, or
    // a name bound to a function or class of the tree, here or through an
    // import.
    pub(super) fn handler(&self, module: &Module, arg: &Arg, frames: &[u32]) -> bool {
        if arg.function {
            return true;
        }
        if arg.called || arg.literal.is_some() {
            return false;
        }
        let Some(root) = arg.root.as_deref() else { return false };
        let [name, rest @ ..] = root else { return false };
        if let Some(binding) = module.binding(name, frames) {
            return rest.is_empty()
                && matches!(
                    &binding.kind,
                    BindingKind::Function
                        | BindingKind::Class { .. }
                        | BindingKind::Value {
                            init: Init::Function,
                            ..
                        }
                );
        }
        let Some((target, member)) = self.member_of(module, root) else { return false };
        let Some(export) = target.export(&member) else { return false };
        match &export.kind {
            ExportKind::Function | ExportKind::Class => true,
            ExportKind::Value => target.binding(&export.name, &[]).is_some_and(|binding| {
                matches!(
                    binding.kind,
                    BindingKind::Value {
                        init: Init::Function,
                        ..
                    }
                )
            }),
            ExportKind::Type => false,
        }
    }

    // The class as it is, not a function or a view handed by `as_view()`.
    fn class_handed(&self, module: &Module, arg: &Arg, frames: &[u32]) -> bool {
        if arg.function || arg.called {
            return false;
        }
        let Some(root) = arg.root.as_deref() else { return false };
        let [name, rest @ ..] = root else { return false };
        if let Some(binding) = module.binding(name, frames) {
            return rest.is_empty() && matches!(binding.kind, BindingKind::Class { .. });
        }
        self.member_of(module, root)
            .and_then(|(target, member)| target.export(&member))
            .is_some_and(|export| export.kind == ExportKind::Class)
    }

    // By declaring module and name.
    pub(super) fn handed_classes(&self) -> BTreeSet<(String, String)> {
        self.modules
            .values()
            .flat_map(|module| module.calls.iter().map(move |call| (module, call)))
            .filter(|(module, call)| {
                handed(self, module, call).is_some() && registers(self, module, call)
            })
            .filter_map(|(module, call)| self.handed_class(module, call))
            .map(|(target, class)| (target.path.clone(), class))
            .collect()
    }

    // A view handed by `as_view()`, a view set, a worker class, with the
    // module declaring it.
    fn handed_class<'m>(&'m self, module: &'m Module, call: &Call) -> Option<(&'m Module, String)> {
        call.args.iter().filter(|arg| !hook(arg)).find_map(|arg| {
            let root = arg.root.as_deref()?;
            let root = match root {
                [.., last] if last == "as_view" => &root[..root.len() - 1],
                other => other,
            };
            let [name, rest @ ..] = root else { return None };
            if rest.is_empty()
                && let Some(binding) = module.binding(name, &call.frames)
            {
                return matches!(binding.kind, BindingKind::Class { .. })
                    .then(|| (module, name.clone()));
            }
            let (target, member) = self.member_of(module, root)?;
            (target.export(&member)?.kind == ExportKind::Class).then_some((target, member))
        })
    }

    // `None` for a receiver of the tree's own or of the runtime's. A member
    // a tree class inherits from a package's class is that package's.
    pub(super) fn receiver(&self, module: &Module, call: &Call) -> Option<Receiver> {
        let head = call.callee.head.as_str();
        let link = |index: usize| call.callee.links.get(index).map(|link| link.name.as_str());
        if head == "self" {
            let class = call.class.as_deref()?;
            let [field, _, ..] = call.callee.links.as_slice() else { return None };
            let field = module.field(class, &field.name)?;
            return match &field.kind {
                BindingKind::Field {
                    type_path: Some(path),
                    ..
                } => {
                    let package = module.package(path.first()?)?;
                    Some(Receiver {
                        package: package.to_owned(),
                        type_name: path.last().cloned(),
                    })
                }
                BindingKind::Field { root: Some(root), .. } => {
                    let [head, rest @ ..] = root.as_slice() else { return None };
                    let member = rest.first().map(String::as_str).or_else(|| link(1));
                    self.trace(module, head, &[], member, TRACE)
                }
                _ => None,
            };
        }
        self.trace(module, head, &call.frames, link(0), TRACE)
    }

    // Follows `name` through the bindings that initialise it, `budget` hops
    // at most. A function a package's decorator made into something (a
    // `click` group, a `typer` app) is that package's. `member` tells a
    // member a tree class declares, which is its own, from one it inherits
    // from a package's base.
    fn trace(
        &self, module: &Module, name: &str, frames: &[u32], member: Option<&str>, budget: usize,
    ) -> Option<Receiver> {
        if budget == 0 {
            return None;
        }
        if let Some(binding) = module.binding(name, frames) {
            return match &binding.kind {
                BindingKind::Value {
                    type_path: Some(path),
                    ..
                }
                | BindingKind::Param {
                    type_path: Some(path),
                } => {
                    let package = module.package(path.first()?)?;
                    Some(Receiver {
                        package: package.to_owned(),
                        type_name: path.last().cloned(),
                    })
                }
                BindingKind::Value {
                    root: Some(root),
                    call,
                    ..
                } => {
                    let [head, rest @ ..] = root.as_slice() else { return None };
                    let member = rest.first().map(String::as_str).or(member);
                    let traced = self.trace(module, head, frames, member, budget - 1)?;
                    let constructed = call
                        .and_then(|index| module.calls.get(index))
                        .filter(|init| init.constructs)
                        .map(|init| init.method().to_owned());
                    Some(Receiver {
                        package: traced.package,
                        type_name: constructed.or(traced.type_name),
                    })
                }
                BindingKind::Function => {
                    module.decorators_of(None, &binding.name).iter().find_map(|decorator| {
                        let head = decorator.name.first()?;
                        let traced = self.trace(module, head, &[], None, budget - 1)?;
                        let made = decorator
                            .name
                            .last()
                            .filter(|last| last.starts_with(char::is_uppercase))
                            .cloned();
                        Some(Receiver {
                            package: traced.package,
                            type_name: traced.type_name.or(made),
                        })
                    })
                }
                BindingKind::Class { bases } => {
                    let declared = module.classes.iter().any(|class| {
                        class.name == binding.name
                            && class.members.iter().any(|m| Some(m.name.as_str()) == member)
                    });
                    if declared {
                        return None;
                    }
                    bases.iter().find_map(|base| {
                        let traced = self.trace(module, base, &[], None, budget - 1)?;
                        Some(Receiver {
                            type_name: traced.type_name.or_else(|| Some(base.clone())),
                            ..traced
                        })
                    })
                }
                _ => None,
            };
        }
        if let Some(package) = module.package(name) {
            let type_name = name.starts_with(char::is_uppercase).then(|| name.to_owned());
            return Some(Receiver {
                package: package.to_owned(),
                type_name,
            });
        }
        let (target, imported) = self.exporter(module, name)?;
        let binding = exported_binding(target, &imported)?;
        self.trace(target, &binding.name, &[], member, budget - 1)
    }

    // Through the imports referenced, the types of the bindings referenced,
    // and, for a method, the types and initializers of its class's fields.
    fn reaches(
        &self, module: &Module, lines: Lines, frames: &[u32], class: Option<&str>,
    ) -> Vec<String> {
        let mut seeds = vec![module.path.clone()];
        let span = class
            .and_then(|class| module.classes.iter().find(|c| c.name == class))
            .map_or(lines, |c| c.lines);
        for name in module.referenced(span) {
            seed(&mut seeds, module, name);
            if let Some(binding) = module.binding(name, frames)
                && let BindingKind::Param {
                    type_path: Some(path),
                }
                | BindingKind::Value {
                    type_path: Some(path),
                    ..
                } = &binding.kind
                && let Some(head) = path.first()
            {
                seed(&mut seeds, module, head);
            }
        }
        if let Some(class) = class {
            let fields = module
                .bindings
                .iter()
                .filter(|b| matches!(&b.scope, Scope::Class(c) if c == class));
            for binding in fields {
                if let BindingKind::Field { type_path, root, .. } = &binding.kind {
                    for head in type_path.iter().chain(root).filter_map(|path| path.first()) {
                        seed(&mut seeds, module, head);
                    }
                }
            }
        }
        self.closure(&seeds, &[])
    }
}

fn seed(seeds: &mut Vec<String>, module: &Module, local: &str) {
    if let Some(path) = module.imported(local) {
        push_unique(seeds, path.to_owned());
    }
}

fn exported_binding<'m>(module: &'m Module, name: &str) -> Option<&'m Binding> {
    let export = module.export(name)?;
    module.binding(&export.name, &[])
}

pub struct Receiver {
    // By top-level name, as imported.
    pub package: String,
    // The class it is typed or constructed as, where the code says.
    pub type_name: Option<String>,
}

// A decorator's head is a binding of the tree (`app = FastAPI()`), traced
// as a call's receiver is.
pub(super) fn receiver_of(tree: &Tree, module: &Module, name: &str) -> Option<Receiver> {
    tree.trace(module, name, &[], None, TRACE)
}

fn hook(arg: &Arg) -> bool {
    arg.keyword.as_deref().is_some_and(|keyword| HOOK_KEYWORDS.contains(&keyword))
}

// Decided over every surface of the tree at once, so `build` runs it last:
// - a stem's one surface is its stem
// - surfaces sharing a stem are `<stem>.<tell>`, the tell its discriminator,
//   else its name, else its entry's module stem
// - two still alike take the nearest segment of their entries' paths that
//   spells neither the stem nor the tell
// - a class carries an id per public method beside its own
pub(super) fn identify(tree: &Tree, surfaces: &mut [Surface]) {
    let module_of = |surface: &Surface| tree.modules.get(&surface.entry);
    let mut owned: Vec<String> = surfaces
        .iter()
        .map(|surface| {
            if surfaces.iter().filter(|other| other.stem == surface.stem).count() == 1 {
                return surface.stem.clone();
            }
            let tell = surface
                .discriminator
                .clone()
                .or_else(|| kebab(&surface.name).filter(|tell| *tell != surface.stem))
                .or_else(|| module_of(surface).map(module_stem))
                .unwrap_or_else(|| "module".to_owned());
            format!("{}.{tell}", surface.stem)
        })
        .collect();
    let alike: Vec<bool> =
        owned.iter().map(|own| owned.iter().filter(|other| *other == own).count() > 1).collect();
    for (index, own) in owned.iter_mut().enumerate() {
        if alike[index]
            && let Some(module) = module_of(&surfaces[index])
            && let Some(segment) = tells_apart(module, &surfaces[index].stem, own)
        {
            own.push('.');
            own.push_str(&segment);
        }
    }
    for (surface, own) in surfaces.iter_mut().zip(owned) {
        let mut ids = vec![own.clone()];
        ids.extend(surface.methods.iter().map(|method| format!("{own}.{method}")));
        surface.ids = ids;
    }
}

// `local/files.py` and `s3/files.py` under `files` are told apart by `local`
// and `s3`.
fn tells_apart(module: &Module, stem: &str, own: &str) -> Option<String> {
    let dir = module.path.rsplit_once('/').map_or("", |(dir, _)| dir);
    std::iter::once(module.stem())
        .chain(dir.rsplit('/'))
        .filter_map(kebab)
        .find(|segment| segment != stem && !own.ends_with(&format!(".{segment}")))
}

// What the entry module does outside the handlers it registers. Another
// surface's entry is what the bootstrap mounts, not what it does, so it is
// reached and not followed; what loading that entry constructs runs before
// any handler and is the bootstrap's to reach.
pub(super) fn start(tree: &Tree, module: &Module, runs: &Runs, registered: &[Surface]) -> Surface {
    let registrations: Vec<Lines> = registered
        .iter()
        .filter(|surface| surface.entry == module.path)
        .map(|surface| surface.lines)
        .collect();
    let mut seeds = vec![module.path.clone()];
    for name in module.referenced_outside(&registrations) {
        seed(&mut seeds, module, name);
    }
    let entries = unique(
        registered
            .iter()
            .map(|surface| surface.entry.clone())
            .filter(|entry| *entry != module.path),
    );

    // what each mounted entry constructs at load
    for entry in &entries {
        let Some(target) = tree.modules.get(entry) else { continue };
        for local in target.constructed() {
            seed(&mut seeds, target, local);
        }
    }
    let how = match runs {
        Runs::Script(script, function) => {
            format!("run by the console script `{script}`, which calls `{function}()`")
        }
        Runs::Guard => "run under its `__main__` guard".to_owned(),
        Runs::Load => "run at load".to_owned(),
    };
    Surface {
        name: "start".to_owned(),
        entry: module.path.clone(),
        stem: "start".to_owned(),
        lines: module.span,
        detail: vec![format!(
            "the process bootstrap, {how}: what runs before each handler is registered, what it \
             awaits before serving, and at shutdown — `stop` and what a signal handler calls, \
             wherever declared"
        )],
        closure: tree.closure(&seeds, &entries),
        discriminator: None,
        methods: Vec::new(),
        ids: vec!["start".to_owned()],
    }
}

// The route prefix each module's routes sit under:
// - `app.register_blueprint(bp, url_prefix="/orders")` and
//   `app.include_router(orders.router, prefix="/api/v1")` mount the module
//   declaring what is registered, under the registering module's own mount;
//   where the registering module declares it itself, the prefix stands
//   alone
// - `path("orders/", include("orders.urls"))` mounts `orders/urls.py` under
//   the including module's own mount
// - a router's own `APIRouter(prefix=..)` joins the prefix it is included
//   under; a blueprint's own `url_prefix` stands only where nothing
//   registers it under another
// The first mount read of a module in one pass stands.
pub(super) fn mounts(tree: &Tree) -> BTreeMap<String, String> {
    let mut mounts: BTreeMap<String, String> = BTreeMap::new();

    // settle nested includes to a fixpoint, bounded where includes cycle
    for _ in 0..=tree.modules.len() {
        let mut next: BTreeMap<String, String> = BTreeMap::new();
        for module in tree.modules.values() {
            let own = mounts.get(&module.path).map_or("", String::as_str);
            for call in &module.calls {
                let method = call.method();
                if MOUNTING.contains(&method) {
                    let prefix = call
                        .keyword("prefix")
                        .or_else(|| call.keyword("url_prefix"))
                        .and_then(|arg| arg.literal.clone())
                        .or_else(|| call.literal().map(str::to_owned))
                        .unwrap_or_default();
                    for arg in call.args.iter().filter(|arg| arg.keyword.is_none()) {
                        let Some(head) = arg.root.as_deref().and_then(|root| root.first()) else {
                            continue;
                        };
                        let (target, under) = match tree.exporter(module, head) {
                            Some((target, _)) => (target.path.as_str(), own),
                            None if module.binding(head, &call.frames).is_some() => {
                                (module.path.as_str(), "")
                            }
                            None => continue,
                        };
                        let mounted = join_route(under, &prefix);
                        if mounted != "/" {
                            next.entry(target.to_owned()).or_insert(mounted);
                        }
                    }
                }
                if ROUTERS.contains(&method)
                    && let Some(literal) = call.literal()
                {
                    for arg in call.args.iter().filter(|arg| arg.called) {
                        let included = arg
                            .root
                            .as_deref()
                            .is_some_and(|root| root.last().is_some_and(|last| last == "include"));
                        let Some(dotted) = arg.inner.as_deref().filter(|_| included) else {
                            continue;
                        };
                        let Some(target) = tree.resolver.entry(dotted) else { continue };
                        let mounted = join_route(own, &strip_pattern(literal));
                        if mounted != "/" {
                            next.entry(target).or_insert(mounted);
                        }
                    }
                }
            }
        }
        let settled = next == mounts;
        mounts = next;
        if settled {
            break;
        }
    }

    // a module's own prefix
    for module in tree.modules.values() {
        for binding in &module.bindings {
            let (
                Scope::Module,
                BindingKind::Value {
                    root: Some(root),
                    call: Some(index),
                    ..
                },
            ) = (&binding.scope, &binding.kind)
            else {
                continue;
            };
            let Some(constructor) = root.last().filter(|last| PREFIXED.contains(&last.as_str()))
            else {
                continue;
            };
            let Some(prefix) = module
                .calls
                .get(*index)
                .and_then(|call| call.keyword("prefix").or_else(|| call.keyword("url_prefix")))
                .and_then(|arg| arg.literal.as_deref())
            else {
                continue;
            };
            let own = join_route("", prefix);
            match mounts.get(&module.path) {
                Some(_) if constructor == "Blueprint" => {}
                Some(outer) => {
                    let joined = join_route(outer, prefix);
                    mounts.insert(module.path.clone(), joined);
                }
                None if own != "/" => {
                    mounts.insert(module.path.clone(), own);
                }
                None => {}
            }
        }
    }
    mounts
}

// The function, or the class handed by `as_view()`, kebab-cased.
fn named_handler(tree: &Tree, module: &Module, call: &Call) -> Option<String> {
    call.args
        .iter()
        .filter(|arg| !hook(arg) && arg.root.is_some())
        .find(|arg| tree.handler(module, arg, &call.frames))
        .and_then(|arg| {
            let root = arg.root.as_ref()?;
            let name = match root.as_slice() {
                [.., class, last] if last == "as_view" => class,
                [.., last] => last,
                [] => return None,
            };
            kebab(name)
        })
}

// A string literal, or a constant bound to one in the tree.
pub(super) fn led(tree: &Tree, module: &Module, call: &Call) -> Option<String> {
    call.args
        .iter()
        .find(|arg| arg.keyword.is_none())
        .and_then(|arg| tree.constant(module, arg, &call.frames))
}

// A chain names itself at its first call with a literal (`command("x")`),
// else the call's own method and lead stand.
pub(super) fn registered(call: &Call, led: Option<String>) -> (&str, Option<String>) {
    let chained = call.callee.links.iter().find_map(|link| {
        let literal = link.call.as_ref()?.literal.clone()?;
        Some((link.name.as_str(), literal))
    });
    chained.map_or_else(|| (call.method(), led), |(method, literal)| (method, Some(literal)))
}

// Under the module's mount. A verb's literal is a path only when it leads
// with a slash; a router's (`path("orders/", ..)`) however spelled.
fn routed(
    module: &Module, method: &str, literal: Option<&str>, mounts: &BTreeMap<String, String>,
) -> Option<String> {
    let literal = literal?;
    let verb = VERBS.contains(&method) && literal.starts_with('/');
    (verb || ROUTERS.contains(&method)).then(|| {
        let prefix = mounts.get(&module.path).map_or("", String::as_str);
        join_route(prefix, &strip_pattern(literal))
    })
}

// The method's own name when it is a verb, else the first verb of its
// `methods=` keyword, else `get`.
fn verb_of(method: &str, methods: Option<&str>) -> String {
    if VERBS.contains(&method) {
        return method.to_owned();
    }
    methods
        .and_then(|methods| {
            methods
                .split(|c: char| !c.is_ascii_alphabetic())
                .map(str::to_ascii_lowercase)
                .find(|word| VERBS.contains(&word.as_str()))
        })
        .unwrap_or_else(|| "get".to_owned())
}

// What code reads at an anchor the survey accepted.
#[derive(Debug)]
pub struct Derived {
    // The stem the code spells at the anchor; `None` leaves the survey's
    // standing.
    pub stem: Option<String>,
    pub discriminator: Option<String>,
    // The public methods of the class declared, decorated, or handed at the
    // anchor, each with its lines.
    pub methods: Vec<(String, Lines)>,
    // The registration or declaration whole where the survey cited one line
    // of several, else the anchor.
    pub lines: Lines,
    // Notes for the brief.
    pub detail: Vec<String>,
    // The modules reached, the anchor's module first.
    pub closure: Vec<String>,
}

enum At<'m> {
    Registration(&'m Call),
    Class(&'m Decorated),
    Def(&'m Decorated),
    Export(&'m Export),
}

// A module under `management/commands/` is read by its path before any
// anchor. Otherwise the first registration, decorated class or `def`, or
// export starting within `lines` is read; failing one, the one enclosing
// the first line, for an anchor within a handler. Where nothing is found,
// the survey's `name` less its stem tells the surface apart and the lines
// are read as they are.
pub(super) fn derive(
    tree: &Tree, module: &Module, lines: Lines, name: &str, stem: &str,
    mounts: &BTreeMap<String, String>,
) -> Derived {
    if let Some(derived) = at_command(tree, module) {
        return derived;
    }
    let within = [
        registration_at(tree, module, lines).map(|call| (call.lines.start, At::Registration(call))),
        decorated_class_at(module, lines).map(|class| (class.lines.start, At::Class(class))),
        decorated_at(module, lines).map(|def| (def.lines.start, At::Def(def))),
        export_at(module, lines)
            .map(|export| (declared_at(module, export).start, At::Export(export))),
    ];
    let first = within
        .into_iter()
        .flatten()
        .filter(|(start, _)| lines.holds(*start))
        .min_by_key(|(start, _)| *start)
        .map(|(_, at)| at);
    let enclosing = || {
        registration_enclosing(tree, module, lines)
            .map(At::Registration)
            .or_else(|| decorated_enclosing(module, lines).map(At::Def))
            .or_else(|| export_enclosing(module, lines).map(At::Export))
    };
    match first.or_else(enclosing) {
        Some(At::Registration(call)) => at_registration(tree, module, call, stem, mounts),
        Some(At::Class(class)) => at_decorated_class(tree, module, class, mounts),
        Some(At::Def(decorated)) => at_decorated_def(tree, module, decorated, mounts),
        Some(At::Export(export)) => at_export(tree, module, export),
        None => Derived {
            stem: None,
            discriminator: normalised(name, stem),
            methods: Vec::new(),
            lines,
            detail: vec![format!("named by the survey at {lines}")],
            closure: tree.reaches(module, lines, &[], None),
        },
    }
}

// A management command is named by its file and declared by its `Command`
// class.
fn at_command(tree: &Tree, module: &Module) -> Option<Derived> {
    let (dir, file) = module.path.rsplit_once('/')?;
    if !dir.ends_with("management/commands") || file.starts_with('_') {
        return None;
    }
    let stem = kebab(file.strip_suffix(".py")?)?;
    let class = module.class("Command");
    let lines = class.map_or(module.span, |class| class.lines);
    let mut detail = vec![format!("management command `{}` by its path", file)];
    if let Some(class) = class {
        detail.push(format!("class `Command` {}", class.lines));
    }
    Some(Derived {
        stem: Some(stem),
        discriminator: None,
        methods: Vec::new(),
        lines,
        detail,
        closure: tree.reaches(module, lines, &[], class.map(|_| "Command")),
    })
}

// The stem is what the route or literal spells. The tell is the handler
// handed by name, unless it is the stem itself, else the verb and the path
// past the resource, else the registering method. A handed class's methods
// and what its module references are reached too.
fn at_registration(
    tree: &Tree, module: &Module, call: &Call, stem: &str, mounts: &BTreeMap<String, String>,
) -> Derived {
    let (method, literal) = registered(call, led(tree, module, call));
    let literal = literal.as_deref();
    let route = routed(module, method, literal, mounts);
    let derived = route.as_deref().map_or_else(|| literal.and_then(literal_stem), route_stem);
    let stem = derived.as_deref().unwrap_or(stem);
    let methods_keyword = call.keyword("methods").map(|arg| arg.head.as_str());
    let discriminator =
        named_handler(tree, module, call).filter(|handler| handler != stem).or_else(|| {
            route.as_deref().map_or_else(
                || kebab(method),
                |route| route_discriminator(&verb_of(method, methods_keyword), route, stem),
            )
        });
    let mut detail = vec![match (&call.class, &call.function) {
        (Some(class), Some(function)) => {
            format!("registered {} in `{class}.{function}`", call.lines)
        }
        (None, Some(function)) => format!("registered {} in `{function}`", call.lines),
        _ => format!("registered {} at module level", call.lines),
    }];
    if let Some(handler) = call.args.iter().find(|arg| arg.function && arg.root.is_none()) {
        detail.push(format!("handler {}", handler.lines));
    }
    if let Some(receiver) = tree.receiver(module, call) {
        detail.push(format!("through `{}`", receiver.package));
    }
    let (methods, class) = match tree.handed_class(module, call) {
        Some((declaring, class)) => {
            let methods: Vec<(String, Lines)> = view_methods(declaring, &class)
                .into_iter()
                .map(|(name, lines)| (name.to_owned(), lines))
                .collect();
            detail.push(format!("class `{class}` handed"));
            detail.extend(noted(&methods));
            (methods, Some((declaring, class)))
        }
        None => (Vec::new(), None),
    };
    let mut closure = tree.reaches(module, call.lines, &call.frames, call.class.as_deref());
    if let Some((declaring, class)) = class {
        for path in tree.reaches(declaring, declaring.span, &[], Some(&class)) {
            push_unique(&mut closure, path);
        }
    }
    Derived {
        stem: derived,
        discriminator,
        methods,
        lines: call.lines,
        detail,
        closure,
    }
}

// The stem is what the decorator's prefix spells under the module's mount;
// the tell is the class's name.
fn at_decorated_class(
    tree: &Tree, module: &Module, class: &Decorated, mounts: &BTreeMap<String, String>,
) -> Derived {
    let name = class.class.as_deref().unwrap_or_default();
    let prefix = mounts.get(&module.path).map_or("", String::as_str);
    let route = join_route(prefix, &strip_pattern(class.literal.as_deref().unwrap_or("")));
    let methods = unique(
        module
            .decorated
            .iter()
            .filter(|d| d.class == class.class && d.member.is_some() && registering(d))
            .filter_map(|d| Some((d.member.clone()?, d.lines))),
    );
    let mut detail = vec![format!("class `{name}` {} under `@{}`", class.lines, spelled(class))];
    detail.extend(through(tree, module, class));
    detail.extend(noted(&methods));
    Derived {
        stem: route_stem(&route),
        discriminator: kebab(name),
        methods,
        lines: class.lines,
        detail,
        closure: tree.reaches(module, class.lines, &[], Some(name)),
    }
}

// The stem is what the route spells, else the command group the `def` is
// under, else the literal's first word. The tell is the verb and the path
// past the resource, else the literal's tail, else the `def`'s name.
fn at_decorated_def(
    tree: &Tree, module: &Module, decorated: &Decorated, mounts: &BTreeMap<String, String>,
) -> Derived {
    let member = decorated.member.as_deref().unwrap_or_default();
    let mut detail = vec![decorated.class.as_ref().map_or_else(
        || format!("def `{member}` {}", decorated.lines),
        |class| format!("method `{class}.{member}` {}", decorated.lines),
    )];
    detail.push(format!("under `@{}`", spelled(decorated)));
    detail.extend(through(tree, module, decorated));
    let route = decorated_route(module, decorated, mounts);
    let group = group_of(module, decorated);
    let literal = || decorated.literal.as_deref().and_then(literal_stem);
    let stem = route.as_deref().map_or_else(|| group.clone().or_else(literal), route_stem);
    let discriminator = match (&route, &stem) {
        (Some(route), Some(stem)) => {
            let method = decorated.name.last().map_or("", String::as_str).to_ascii_lowercase();
            route_discriminator(&verb_of(&method, decorated.keyword("methods")), route, stem)
                .or_else(|| kebab(member))
        }
        (None, _) if group.is_some() => literal().or_else(|| kebab(member)),
        _ => decorated.literal.as_deref().and_then(literal_tail).or_else(|| kebab(member)),
    };
    if let Some(group) = &group {
        detail.push(format!("subcommand of the group `{group}`"));
    }
    Derived {
        stem,
        discriminator,
        methods: Vec::new(),
        lines: decorated.lines,
        detail,
        closure: tree.reaches(module, decorated.lines, &[], decorated.class.as_deref()),
    }
}

// The export's name tells it apart, and a class carries its public methods.
fn at_export(tree: &Tree, module: &Module, export: &Export) -> Derived {
    let (what, methods, class) = match &export.kind {
        ExportKind::Class => (
            "class",
            methods(module, &export.name)
                .into_iter()
                .map(|(name, lines)| (name.to_owned(), lines))
                .collect(),
            Some(export.name.as_str()),
        ),
        ExportKind::Function => ("function", Vec::new(), None),
        ExportKind::Value | ExportKind::Type => ("value", Vec::new(), None),
    };
    let declared = declared_at(module, export);
    let mut detail = vec![format!("exported {what} {declared}")];
    detail.extend(noted(&methods));
    Derived {
        stem: None,
        discriminator: kebab(&export.name),
        methods,
        lines: declared,
        detail,
        closure: tree.reaches(module, declared, &[], class),
    }
}

// `app.get("/orders")`, for a note.
fn spelled(decorated: &Decorated) -> String {
    let argument = decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
    format!("{}{argument}", decorated.name.join("."))
}

fn through(tree: &Tree, module: &Module, decorated: &Decorated) -> Option<String> {
    let receiver = receiver_of(tree, module, decorated.name.first()?)?;
    Some(format!("through `{}`", receiver.package))
}

fn noted(methods: &[(String, Lines)]) -> Option<String> {
    if methods.is_empty() {
        return None;
    }
    let listed: Vec<String> =
        methods.iter().map(|(name, lines)| format!("`{name}` {lines}")).collect();
    Some(format!("methods {}", listed.join(", ")))
}

// Registers what it decorates, rather than shaping it or hooking it on a
// lifecycle.
pub(super) fn registering(decorated: &Decorated) -> bool {
    decorated
        .name
        .last()
        .is_some_and(|name| !shapes(name) && !DECORATOR_HOOKS.contains(&name.as_str()))
        && !hooks(&decorated.name.join("."))
}

// `@orders.command("list")` where `orders` is `@cli.group()` is a
// subcommand of `orders`, the stem its subcommands share. `None` under the
// root group, which a package's decorator makes, so its commands lead with
// their own literals.
fn group_of(module: &Module, decorated: &Decorated) -> Option<String> {
    let [head, method] = decorated.name.as_slice() else { return None };
    let function = |name: &str| {
        module.binding(name, &[]).is_some_and(|b| matches!(b.kind, BindingKind::Function))
    };
    if method != "command" || !function(head) {
        return None;
    }
    let nested = module.decorators_of(None, head).into_iter().any(|d| {
        d.name.last().is_some_and(|last| last == "group")
            && d.name.first().is_some_and(|outer| function(outer))
    });
    nested.then(|| kebab(head)).flatten()
}

fn class_prefix<'m>(module: &'m Module, class: &str) -> Option<&'m Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.class.as_deref() == Some(class) && d.member.is_none() && d.literal.is_some())
        .find(|d| registering(d))
}

// Under the module's mount and, for a method, its class's prefix.
fn decorated_route(
    module: &Module, decorated: &Decorated, mounts: &BTreeMap<String, String>,
) -> Option<String> {
    let method = decorated.name.last()?.to_ascii_lowercase();
    if !(VERBS.contains(&method.as_str()) || ROUTERS.contains(&method.as_str())) {
        return None;
    }
    let literal = decorated.literal.as_deref().unwrap_or("");
    let mount = mounts.get(&module.path).map_or("", String::as_str);
    let prefix = decorated
        .class
        .as_deref()
        .and_then(|class| class_prefix(module, class))
        .and_then(|d| d.literal.as_deref())
        .unwrap_or("");
    Some(join_route(&join_route(mount, prefix), &strip_pattern(literal)))
}

fn decorated_class_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.member.is_none() && registering(d))
        .find(|d| lines.holds(d.lines.start))
}

fn decorated_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    registering_defs(module).find(|d| lines.holds(d.lines.start))
}

fn decorated_enclosing(module: &Module, lines: Lines) -> Option<&Decorated> {
    registering_defs(module).find(|d| d.lines.holds(lines.start))
}

fn registering_defs(module: &Module) -> impl Iterator<Item = &Decorated> {
    module.decorated.iter().filter(|d| d.member.is_some() && registering(d))
}

// A handler among the arguments, not under a hook keyword, and not to a
// structural call (`sorted`, `partial`, `Depends`). A class handed straight
// to a package's own function or constructor (`select(Product)`,
// `models.ForeignKey(Order)`) is queried or typed by it, not run; one
// handed to a receiver the code constructs (`router.register("orders",
// OrderViewSet)`) is run.
fn hands(tree: &Tree, module: &Module, call: &Call) -> bool {
    if call.structural() {
        return false;
    }
    let through_package = module.package(&call.callee.head).is_some();
    call.args.iter().any(|arg| {
        !hook(arg)
            && tree.handler(module, arg, &call.frames)
            && !(through_package && tree.class_handed(module, arg, &call.frames))
    })
}

// A call outside any handler that hands a function to something a package
// provides: where a framework, a queue, a scheduler, or a CLI is told what
// to run.
pub(super) fn handed(tree: &Tree, module: &Module, call: &Call) -> Option<Receiver> {
    (call.depth == 0 && hands(tree, module, call)).then(|| tree.receiver(module, call))?
}

// At any depth and whatever the receiver. A wrapper handed a function for
// its value (`handler = retrying(send)`) defines and registers nothing.
pub(super) fn registers(tree: &Tree, module: &Module, call: &Call) -> bool {
    hands(tree, module, call)
        && (call.discarded() || call.constructs || led(tree, module, call).is_some())
}

fn registration_at<'m>(tree: &Tree, module: &'m Module, lines: Lines) -> Option<&'m Call> {
    module
        .calls
        .iter()
        .filter(|call| registers(tree, module, call))
        .find(|call| lines.holds(call.lines.start))
}

fn registration_enclosing<'m>(tree: &Tree, module: &'m Module, lines: Lines) -> Option<&'m Call> {
    module
        .calls
        .iter()
        .filter(|call| registers(tree, module, call))
        .find(|call| call.lines.holds(lines.start))
}

fn declared_at(module: &Module, export: &Export) -> Lines {
    module.binding(&export.name, &[]).map_or(export.lines, |binding| binding.lines)
}

// A function or class before a value.
fn export_at(module: &Module, lines: Lines) -> Option<&Export> {
    let starts = |export: &&Export| lines.holds(declared_at(module, export).start);
    let exports = || module.exports.iter();
    exports()
        .filter(|export| callable(export))
        .find(starts)
        .or_else(|| exports().filter(|export| valued(export)).find(starts))
}

fn export_enclosing(module: &Module, lines: Lines) -> Option<&Export> {
    module
        .exports
        .iter()
        .filter(|export| callable(export) || valued(export))
        .find(|export| declared_at(module, export).holds(lines.start))
}

const fn callable(export: &Export) -> bool {
    matches!(export.kind, ExportKind::Function | ExportKind::Class)
}

const fn valued(export: &Export) -> bool {
    matches!(export.kind, ExportKind::Value)
}

// The survey's name less the words of its stem and the segments that only
// version or namespace a path: `GET /api/customers` under `customers` is
// `get`.
fn normalised(name: &str, stem: &str) -> Option<String> {
    let spelled = kebab(name)?;
    let kept: Vec<&str> = spelled
        .split('-')
        .filter(|word| !stem.split('-').any(|own| own == *word) && names_resource(word))
        .collect();
    kebab(&kept.join("-"))
}

fn methods<'m>(module: &'m Module, name: &str) -> Vec<(&'m str, Lines)> {
    module
        .class(name)
        .map(|class| {
            class
                .members
                .iter()
                .filter(|m| {
                    !m.private
                        && !dunder(&m.name)
                        && matches!(m.kind, MemberKind::Method | MemberKind::Getter)
                })
                .map(|m| (m.name.as_str(), m.lines))
                .collect()
        })
        .unwrap_or_default()
}

// A `__post_init__` or `__str__` is the runtime's to call, never a caller's
// by name, so it takes no id.
fn dunder(name: &str) -> bool {
    name.starts_with("__") && name.ends_with("__")
}

// A view's verb methods, else those under an `action` decorator, else all
// its public methods.
fn view_methods<'m>(module: &'m Module, name: &str) -> Vec<(&'m str, Lines)> {
    let all = methods(module, name);
    let verbs: Vec<(&str, Lines)> =
        all.iter().filter(|(method, _)| VERBS.contains(method)).copied().collect();
    if !verbs.is_empty() {
        return verbs;
    }
    let actions: Vec<(&str, Lines)> = all
        .iter()
        .filter(|(method, _)| {
            module
                .decorators_of(Some(name), method)
                .iter()
                .any(|d| d.name.last().is_some_and(|last| last == "action"))
        })
        .copied()
        .collect();
    if actions.is_empty() { all } else { actions }
}

// The modules no other imports; every module where each is imported.
pub(super) fn roots(tree: &Tree) -> Vec<String> {
    let imported: BTreeSet<&str> =
        tree.modules.values().flat_map(|module| module.reached()).collect();
    let roots: Vec<String> =
        tree.modules.keys().filter(|path| !imported.contains(path.as_str())).cloned().collect();
    if roots.is_empty() { tree.modules.keys().cloned().collect() } else { roots }
}

// `/prefix/path`, however either is spelled: `"/orders/"` and `"orders/"`
// both lead with one slash and end with none.
fn join_route(prefix: &str, path: &str) -> String {
    let segments: Vec<&str> =
        prefix.split('/').chain(path.split('/')).filter(|segment| !segment.is_empty()).collect();
    format!("/{}", segments.join("/"))
}

// Less the regex anchors `re_path` spells a pattern with.
fn strip_pattern(route: &str) -> String {
    route.trim_start_matches('^').trim_end_matches('$').to_owned()
}

// `<int:pk>` and `(?P<pk>\d+)` are `pk`; `{id}` and `:id` are `id`; any
// other segment is as written.
fn param_name(segment: &str) -> &str {
    if let Some(inner) = segment.strip_prefix('{').and_then(|rest| rest.strip_suffix('}')) {
        return inner.split_once(':').map_or(inner, |(name, _)| name);
    }
    if let Some(inner) = segment.strip_prefix('<').and_then(|rest| rest.strip_suffix('>')) {
        return inner.rsplit_once(':').map_or(inner, |(_, name)| name);
    }
    if let Some(start) = segment.find("(?P<") {
        let rest = &segment[start + 4..];
        return rest.split_once('>').map_or(rest, |(name, _)| name);
    }
    segment.trim_start_matches(':')
}

// The verb, then the segments past the one spelling the stem, a parameter
// by its bare name: `GET /api/orders/{id}` under `orders` is `get-id`. A
// route spelling no segment as its stem is relative to a mount the survey
// did not read, so every segment tells: `POST /<int:pk>/assign` under
// `tasks` is `post-pk-assign`.
fn route_discriminator(verb: &str, route: &str, stem: &str) -> Option<String> {
    let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
    let past = segments
        .iter()
        .position(|segment| kebab(segment).is_some_and(|spelled| spelled == stem))
        .map_or(0, |i| i + 1);
    let parts: Vec<&str> = std::iter::once(verb)
        .chain(segments[past..].iter().map(|segment| param_name(segment)))
        .collect();
    kebab(&parts.join("-"))
}

// The first segment that names a resource.
fn route_stem(route: &str) -> Option<String> {
    route
        .split('/')
        .filter(|segment| !segment.is_empty())
        .find(|segment| names_resource(segment))
        .and_then(kebab)
}

fn names_resource(segment: &str) -> bool {
    let version = segment
        .strip_prefix('v')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()));
    let pattern =
        segment.starts_with(':') || segment.contains(['*', '{', '(', '[', '<', '?', '\\']);
    !(PATH_NOISE.contains(&segment) || version || pattern)
}

// Literals a scheduler is led by that name its trigger, not its job.
const TRIGGERS: &[&str] = &["cron", "interval", "date"];

// The first word of a literal that names something, a dotted name by its
// first segment (`invoices.send` is `invoices`). Nothing for a pattern, a
// schedule, or a trigger.
fn literal_stem(literal: &str) -> Option<String> {
    let word = literal.split_whitespace().next()?;
    if !word.starts_with(|c: char| c.is_ascii_alphabetic())
        || word.contains(['<', '>', '[', ']', '*', '/', ':'])
        || TRIGGERS.contains(&word)
    {
        return None;
    }
    kebab(word.split('.').next().unwrap_or(word))
}

// `invoices.remind` is told from `invoices.void` by `remind`. Nothing for a
// literal of one segment or one naming nothing.
fn literal_tail(literal: &str) -> Option<String> {
    literal_stem(literal)?;
    let word = literal.split_whitespace().next()?;
    let (_, tail) = word.split_once('.')?;
    kebab(tail)
}

fn module_stem(module: &Module) -> String {
    kebab(module.stem()).unwrap_or_else(|| "module".to_owned())
}

// Camel humps split, anything not a letter or digit a hyphen, runs
// collapsed. `None` when nothing is left.
pub(super) fn kebab(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len() + 4);
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() && i > 0 {
                let prev = chars[i - 1];
                let next = chars.get(i + 1).copied();
                let hump = prev.is_ascii_lowercase()
                    || prev.is_ascii_digit()
                    || (prev.is_ascii_uppercase() && next.is_some_and(|n| n.is_ascii_lowercase()));
                if hump && !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
            }
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-');
    emery_sdk::is_kebab(trimmed).then(|| trimmed.to_owned())
}
