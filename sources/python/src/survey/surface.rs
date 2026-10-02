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
//! names. A decorator among the lifecycle hooks the data model lists
//! (`Decorated::registering`) shapes a handler rather than registers it,
//! which keeps a verb decorator beneath one of them the surface. A module
//! under `management/commands/` is read as a file-routed framework's module
//! is.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::kebab;
use emery_sdk::survey::code::{
    Arg, BindingKind, Call, Decorated, Export, ExportKind, Imported, Init, MemberKind, Scope,
};
use emery_sdk::survey::{Lines, route};

use super::parse::Module;
use super::resolve::{Manifest, Resolver};
use super::{DIALECT, push_unique, unique};

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
    pub(super) fn read(root: &Path, paths: Vec<String>, data: Vec<String>) -> Self {
        let mut modules = BTreeMap::new();
        for path in paths {
            match std::fs::read_to_string(root.join(&path)) {
                Ok(text) => {
                    modules.insert(path.clone(), Module::parse(&path, text));
                }
                Err(error) => {
                    emery_sdk::tracing::warn!(path, %error, "module is not readable text; left out");
                }
            }
        }
        let manifest = Manifest::read(root);
        let resolver = Resolver::new(modules.keys().cloned(), data, manifest.name.as_deref());
        for module in modules.values_mut() {
            resolver.settle(module);
        }
        Self {
            modules,
            resolver,
            manifest,
        }
    }

    // The first entry that runs something, probed in this order:
    // - the entries the manifest's scripts name
    // - the conventional entries the tree holds
    // - the one module of a tree of one
    // An entry runs by a console script whose function it declares or
    // imports from a module of the tree that does, by a `__main__` guard, or
    // at load. An entry that only declares is a library's.
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
            Imported::Whole => local.to_owned(),
            Imported::Default | Imported::Star | Imported::Effect => return None,
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
            (Imported::Whole, rest) => {
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
            for path in module.reached(&DIALECT) {
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
        match &target.exported_binding(&imported)?.kind {
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
        match export.kind {
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
            ExportKind::Type | ExportKind::Unknown => false,
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
                self.handed(module, call).is_some() && self.registers(module, call)
            })
            .filter_map(|(module, call)| self.handed_class(module, call))
            .map(|(target, class)| (target.path.clone(), class))
            .collect()
    }

    // A view handed by `as_view()`, a view set, a worker class, with the
    // module declaring it.
    fn handed_class<'m>(&'m self, module: &'m Module, call: &Call) -> Option<(&'m Module, String)> {
        call.args.iter().filter(|arg| !arg.is_hook(&DIALECT)).find_map(|arg| {
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
        if head == DIALECT.self_name {
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
        let binding = target.exported_binding(&imported)?;
        self.trace(target, &binding.name, &[], member, budget - 1)
    }

    // Through the imports referenced, the types of the bindings referenced,
    // and, for a method, the types and initializers of its class's fields.
    fn reaches(
        &self, module: &Module, lines: Lines, frames: &[u32], class: Option<&str>,
    ) -> Vec<String> {
        let mut seeds = vec![module.path.clone()];
        let span = class.and_then(|class| module.class(class)).map_or(lines, |c| c.lines);
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

    // A decorator's head is a binding of the tree (`app = FastAPI()`), traced
    // as a call's receiver is.
    pub(super) fn receiver_of(&self, module: &Module, name: &str) -> Option<Receiver> {
        self.trace(module, name, &[], None, TRACE)
    }

    // A string literal, or a constant bound to one in the tree.
    pub(super) fn led(&self, module: &Module, call: &Call) -> Option<String> {
        call.args
            .iter()
            .find(|arg| arg.keyword.is_none())
            .and_then(|arg| self.constant(module, arg, &call.frames))
    }

    // The function, or the class handed by `as_view()`, kebab-cased.
    fn named_handler(&self, module: &Module, call: &Call) -> Option<String> {
        call.args
            .iter()
            .filter(|arg| !arg.is_hook(&DIALECT) && arg.root.is_some())
            .find(|arg| self.handler(module, arg, &call.frames))
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

    // A handler among the arguments, not under a hook keyword, and not to a
    // structural call (`sorted`, `partial`, `Depends`). A class handed straight
    // to a package's own function or constructor (`select(Product)`,
    // `models.ForeignKey(Order)`) is queried or typed by it, not run; one
    // handed to a receiver the code constructs (`router.register("orders",
    // OrderViewSet)`) is run.
    fn hands(&self, module: &Module, call: &Call) -> bool {
        if call.structural(&DIALECT) {
            return false;
        }
        let through_package = module.package(&call.callee.head).is_some();
        call.args.iter().any(|arg| {
            !arg.is_hook(&DIALECT)
                && self.handler(module, arg, &call.frames)
                && !(through_package && self.class_handed(module, arg, &call.frames))
        })
    }

    // A call outside any handler that hands a function to something a package
    // provides: where a framework, a queue, a scheduler, or a CLI is told what
    // to run.
    pub(super) fn handed(&self, module: &Module, call: &Call) -> Option<Receiver> {
        (call.depth == 0 && self.hands(module, call)).then(|| self.receiver(module, call))?
    }

    // At any depth and whatever the receiver. A wrapper handed a function for
    // its value (`handler = retrying(send)`) defines and registers nothing.
    pub(super) fn registers(&self, module: &Module, call: &Call) -> bool {
        self.hands(module, call)
            && (call.discarded() || call.constructs || self.led(module, call).is_some())
    }

    fn registration_at<'m>(&self, module: &'m Module, lines: Lines) -> Option<&'m Call> {
        module
            .calls
            .iter()
            .filter(|call| self.registers(module, call))
            .find(|call| lines.holds(call.lines.start))
    }

    fn registration_enclosing<'m>(&self, module: &'m Module, lines: Lines) -> Option<&'m Call> {
        module
            .calls
            .iter()
            .filter(|call| self.registers(module, call))
            .find(|call| call.lines.holds(lines.start))
    }

    // The modules no other imports; every module where each is imported.
    pub(super) fn roots(&self) -> Vec<String> {
        let imported: BTreeSet<&str> =
            self.modules.values().flat_map(|module| module.reached(&DIALECT)).collect();
        let roots: Vec<String> =
            self.modules.keys().filter(|path| !imported.contains(path.as_str())).cloned().collect();
        if roots.is_empty() { self.modules.keys().cloned().collect() } else { roots }
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
    pub(super) fn mounts(&self) -> BTreeMap<String, String> {
        let mut mounts: BTreeMap<String, String> = BTreeMap::new();

        // settle nested includes to a fixpoint, bounded where includes cycle
        for _ in 0..=self.modules.len() {
            let mut next: BTreeMap<String, String> = BTreeMap::new();
            for module in self.modules.values() {
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
                            let Some(head) = arg.root.as_deref().and_then(|root| root.first())
                            else {
                                continue;
                            };
                            let (target, under) = match self.exporter(module, head) {
                                Some((target, _)) => (target.path.as_str(), own),
                                None if module.binding(head, &call.frames).is_some() => {
                                    (module.path.as_str(), "")
                                }
                                None => continue,
                            };
                            let mounted = route::join(under, &prefix);
                            if mounted != "/" {
                                next.entry(target.to_owned()).or_insert(mounted);
                            }
                        }
                    }
                    if ROUTERS.contains(&method)
                        && let Some(literal) = call.literal()
                    {
                        for arg in call.args.iter().filter(|arg| arg.called) {
                            let included = arg.root.as_deref().is_some_and(|root| {
                                root.last().is_some_and(|last| last == "include")
                            });
                            let Some(dotted) = arg.inner.as_deref().filter(|_| included) else {
                                continue;
                            };
                            let Some(target) = self.resolver.entry(dotted) else { continue };
                            let mounted = route::join(own, &strip_pattern(literal));
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

        // join each module's own prefix onto its mount
        for module in self.modules.values() {
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
                let Some(constructor) =
                    root.last().filter(|last| PREFIXED.contains(&last.as_str()))
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
                let own = route::join("", prefix);
                match mounts.get(&module.path) {
                    Some(_) if constructor == "Blueprint" => {}
                    Some(outer) => {
                        let joined = route::join(outer, prefix);
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

    // Decided over every surface of the tree at once, so `build` runs it last:
    // - a stem's one surface is its stem
    // - surfaces sharing a stem are `<stem>.<tell>`, the tell its discriminator,
    //   else its name, else its entry's module stem
    // - two still alike take the nearest segment of their entries' paths that
    //   spells neither the stem nor the tell
    // - a class carries an id per public method beside its own
    pub(super) fn identify(&self, surfaces: &mut [Surface]) {
        let module_of = |surface: &Surface| self.modules.get(&surface.entry);
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
        let alike: Vec<bool> = owned
            .iter()
            .map(|own| owned.iter().filter(|other| *other == own).count() > 1)
            .collect();
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
}

fn seed(seeds: &mut Vec<String>, module: &Module, local: &str) {
    if let Some(path) = module.imported(local) {
        push_unique(seeds, path.to_owned());
    }
}

pub struct Receiver {
    // By top-level name, as imported.
    pub package: String,
    // The class it is typed or constructed as, where the code says.
    pub type_name: Option<String>,
}

// `local/files.py` and `s3/files.py` under `files` are told apart by `local`
// and `s3`.
fn tells_apart(module: &Module, stem: &str, own: &str) -> Option<String> {
    let dir = module.path.rsplit_once('/').map_or("", |(dir, _)| dir);
    std::iter::once(module.stem(&DIALECT))
        .chain(dir.rsplit('/'))
        .filter_map(kebab)
        .find(|segment| segment != stem && !own.ends_with(&format!(".{segment}")))
}

impl Surface {
    // What the entry module does outside the handlers it registers. Another
    // surface's entry is what the bootstrap mounts, not what it does, so it is
    // reached and not followed; what loading that entry constructs runs before
    // any handler and is the bootstrap's to reach.
    pub(super) fn start(tree: &Tree, module: &Module, runs: &Runs, registered: &[Self]) -> Self {
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
        Self {
            name: "start".to_owned(),
            entry: module.path.clone(),
            stem: "start".to_owned(),
            lines: module.span,
            detail: vec![format!(
                "the process bootstrap, {how}: what runs before each handler is registered, what \
                 it awaits before serving, and at shutdown — `stop` and what a signal handler \
                 calls, wherever declared"
            )],
            closure: tree.closure(&seeds, &entries),
            discriminator: None,
            methods: Vec::new(),
            ids: vec!["start".to_owned()],
        }
    }
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
        route::join(prefix, &strip_pattern(literal))
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

impl Derived {
    // A module under `management/commands/` is read by its path before any
    // anchor. Otherwise the first registration, decorated class or `def`, or
    // export starting within `lines` is read. Failing one, the one enclosing
    // the first line is read, for an anchor within a handler. Where nothing is
    // found, the survey's `name` less its stem tells the surface apart and the
    // lines are read as they are.
    pub(super) fn derive(
        tree: &Tree, module: &Module, lines: Lines, name: &str, stem: &str,
        mounts: &BTreeMap<String, String>,
    ) -> Self {
        if let Some(derived) = Self::at_command(tree, module) {
            return derived;
        }
        let within = [
            tree.registration_at(module, lines)
                .map(|call| (call.lines.start, At::Registration(call))),
            decorated_class_at(module, lines).map(|class| (class.lines.start, At::Class(class))),
            decorated_at(module, lines).map(|def| (def.lines.start, At::Def(def))),
            export_at(module, lines)
                .map(|export| (module.declared_at(export).start, At::Export(export))),
        ];
        let first = within
            .into_iter()
            .flatten()
            .filter(|(start, _)| lines.holds(*start))
            .min_by_key(|(start, _)| *start)
            .map(|(_, at)| at);
        let enclosing = || {
            tree.registration_enclosing(module, lines)
                .map(At::Registration)
                .or_else(|| decorated_enclosing(module, lines).map(At::Def))
                .or_else(|| export_enclosing(module, lines).map(At::Export))
        };
        match first.or_else(enclosing) {
            Some(At::Registration(call)) => Self::at_registration(tree, module, call, stem, mounts),
            Some(At::Class(class)) => Self::at_decorated_class(tree, module, class, mounts),
            Some(At::Def(decorated)) => Self::at_decorated_def(tree, module, decorated, mounts),
            Some(At::Export(export)) => Self::at_export(tree, module, export),
            None => Self {
                stem: None,
                discriminator: DIALECT.route.normalised(name, stem),
                methods: Vec::new(),
                lines,
                detail: vec![format!("named by the survey at {lines}")],
                closure: tree.reaches(module, lines, &[], None),
            },
        }
    }

    // A management command is named by its file and declared by its `Command`
    // class.
    fn at_command(tree: &Tree, module: &Module) -> Option<Self> {
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
        Some(Self {
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
    ) -> Self {
        let (method, literal) = call.registered(tree.led(module, call));
        let literal = literal.as_deref();
        let route = routed(module, method, literal, mounts);
        let derived = route.as_deref().map_or_else(
            || literal.and_then(route::literal_stem),
            |route| DIALECT.route.stem(route),
        );
        let stem = derived.as_deref().unwrap_or(stem);
        let methods_keyword = call.keyword("methods").map(|arg| arg.head.as_str());
        let discriminator =
            tree.named_handler(module, call).filter(|handler| handler != stem).or_else(|| {
                route.as_deref().map_or_else(
                    || kebab(method),
                    |route| {
                        DIALECT.route.discriminator(&verb_of(method, methods_keyword), route, stem)
                    },
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
        Self {
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
    ) -> Self {
        let name = class.class.as_deref().unwrap_or_default();
        let prefix = mounts.get(&module.path).map_or("", String::as_str);
        let route = route::join(prefix, &strip_pattern(class.literal.as_deref().unwrap_or("")));
        let methods = unique(
            module
                .decorated
                .iter()
                .filter(|d| d.class == class.class && d.member.is_some() && d.registering(&DIALECT))
                .filter_map(|d| Some((d.member.clone()?, d.lines))),
        );
        let mut detail =
            vec![format!("class `{name}` {} under `@{}`", class.lines, spelled(class))];
        detail.extend(through(tree, module, class));
        detail.extend(noted(&methods));
        Self {
            stem: DIALECT.route.stem(&route),
            discriminator: kebab(name),
            methods,
            lines: class.lines,
            detail,
            closure: tree.reaches(module, class.lines, &[], Some(name)),
        }
    }

    // The stem is what the route sits under, else what it spells itself, else
    // the command group the `def` is under, else the literal's first word. The
    // tell is the verb and the path past the resource, else the literal's tail,
    // else the `def`'s name.
    fn at_decorated_def(
        tree: &Tree, module: &Module, decorated: &Decorated, mounts: &BTreeMap<String, String>,
    ) -> Self {
        let member = decorated.member.as_deref().unwrap_or_default();
        let mut detail = vec![decorated.class.as_ref().map_or_else(
            || format!("def `{member}` {}", decorated.lines),
            |class| format!("method `{class}.{member}` {}", decorated.lines),
        )];
        detail.push(format!("under `@{}`", spelled(decorated)));
        detail.extend(through(tree, module, decorated));
        let route = decorated_route(module, decorated, mounts);
        let group = group_of(module, decorated);
        let literal = || decorated.literal.as_deref().and_then(route::literal_stem);
        let stem = route.as_deref().map_or_else(
            || group.clone().or_else(literal),
            |route| under(module, decorated, mounts).or_else(|| DIALECT.route.stem(route)),
        );
        let discriminator = match (&route, &stem) {
            (Some(route), Some(stem)) => {
                let method = decorated.name.last().map_or("", String::as_str).to_ascii_lowercase();
                DIALECT
                    .route
                    .discriminator(&verb_of(&method, decorated.keyword("methods")), route, stem)
                    .or_else(|| kebab(member))
            }
            (None, _) if group.is_some() => literal().or_else(|| kebab(member)),
            _ => decorated.literal.as_deref().and_then(literal_tail).or_else(|| kebab(member)),
        };
        if let Some(group) = &group {
            detail.push(format!("subcommand of the group `{group}`"));
        }
        Self {
            stem,
            discriminator,
            methods: Vec::new(),
            lines: decorated.lines,
            detail,
            closure: tree.reaches(module, decorated.lines, &[], decorated.class.as_deref()),
        }
    }

    // The export's name tells it apart, and a class carries its public methods.
    fn at_export(tree: &Tree, module: &Module, export: &Export) -> Self {
        let (what, methods, class) = match export.kind {
            ExportKind::Class => (
                "class",
                methods(module, &export.name, |name| !dunder(name))
                    .into_iter()
                    .map(|(name, lines)| (name.to_owned(), lines))
                    .collect(),
                Some(export.name.as_str()),
            ),
            ExportKind::Function => ("function", Vec::new(), None),
            ExportKind::Value | ExportKind::Type | ExportKind::Unknown => {
                ("value", Vec::new(), None)
            }
        };
        let declared = module.declared_at(export);
        let mut detail = vec![format!("exported {what} {declared}")];
        detail.extend(noted(&methods));
        Self {
            stem: None,
            discriminator: kebab(&export.name),
            methods,
            lines: declared,
            detail,
            closure: tree.reaches(module, declared, &[], class),
        }
    }
}

// The resource a route sits under before its own literal: what its module's
// mount or its class's prefix spells, else the first tag of the router it is
// declared on — `APIRouter(tags=["login"])` with no prefix groups its routes
// as `login`, however each spells its path. `None` where only the literal
// names one.
fn under(
    module: &Module, decorated: &Decorated, mounts: &BTreeMap<String, String>,
) -> Option<String> {
    let mount = mounts.get(&module.path).map_or("", String::as_str);
    let prefix = decorated
        .class
        .as_deref()
        .and_then(|class| class_prefix(module, class))
        .and_then(|d| d.literal.as_deref())
        .unwrap_or("");
    DIALECT.route.stem(&route::join(mount, prefix)).or_else(|| router_tag(module, decorated))
}

// The first `tags` literal of the `APIRouter` the decorator's head is bound
// to at module level.
fn router_tag(module: &Module, decorated: &Decorated) -> Option<String> {
    let binding = module.binding(decorated.name.first()?, &[])?;
    let BindingKind::Value {
        root: Some(root),
        call: Some(index),
        ..
    } = &binding.kind
    else {
        return None;
    };
    if binding.scope != Scope::Module || root.last().map(String::as_str) != Some("APIRouter") {
        return None;
    }
    module.calls.get(*index)?.keyword("tags")?.inner.as_deref().and_then(kebab)
}

// `app.get("/orders")`, for a note.
fn spelled(decorated: &Decorated) -> String {
    let argument = decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
    format!("{}{argument}", decorated.name.join("."))
}

fn through(tree: &Tree, module: &Module, decorated: &Decorated) -> Option<String> {
    let receiver = tree.receiver_of(module, decorated.name.first()?)?;
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
        .find(|d| d.registering(&DIALECT))
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
    Some(route::join(&route::join(mount, prefix), &strip_pattern(literal)))
}

fn decorated_class_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.member.is_none() && d.registering(&DIALECT))
        .find(|d| lines.holds(d.lines.start))
}

fn decorated_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    registering_defs(module).find(|d| lines.holds(d.lines.start))
}

fn decorated_enclosing(module: &Module, lines: Lines) -> Option<&Decorated> {
    registering_defs(module).find(|d| d.lines.holds(lines.start))
}

fn registering_defs(module: &Module) -> impl Iterator<Item = &Decorated> {
    module.decorated.iter().filter(|d| d.member.is_some() && d.registering(&DIALECT))
}

// A function or class before a value.
fn export_at(module: &Module, lines: Lines) -> Option<&Export> {
    let starts = |export: &&Export| lines.holds(module.declared_at(export).start);
    let exports = || module.exports.iter();
    exports()
        .filter(|export| export.kind.callable())
        .find(starts)
        .or_else(|| exports().filter(|export| export.kind.valued()).find(starts))
}

fn export_enclosing(module: &Module, lines: Lines) -> Option<&Export> {
    module
        .exports
        .iter()
        .filter(|export| export.kind.callable() || export.kind.valued())
        .find(|export| module.declared_at(export).holds(lines.start))
}

// The public methods and getters of the class `name` that `keep` admits,
// each with its lines.
fn methods<'m>(
    module: &'m Module, name: &str, keep: impl Fn(&str) -> bool,
) -> Vec<(&'m str, Lines)> {
    module
        .class(name)
        .map(|class| {
            class
                .members
                .iter()
                .filter(|m| {
                    !m.private
                        && keep(&m.name)
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
    let all = methods(module, name, |name| !dunder(name));
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

// Less the regex anchors `re_path` spells a pattern with.
fn strip_pattern(route: &str) -> String {
    route.trim_start_matches('^').trim_end_matches('$').to_owned()
}

// `<int:pk>` and `(?P<pk>\d+)` are `pk`; `{id}` and `:id` are `id`; any
// other segment is as written.
pub(super) fn param_name(segment: &str) -> &str {
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

// `invoices.remind` is told from `invoices.void` by `remind`. Nothing for a
// literal of one segment or one naming nothing.
fn literal_tail(literal: &str) -> Option<String> {
    route::literal_stem(literal)?;
    let word = literal.split_whitespace().next()?;
    let (_, tail) = word.split_once('.')?;
    kebab(tail)
}

fn module_stem(module: &Module) -> String {
    kebab(module.stem(&DIALECT)).unwrap_or_else(|| "module".to_owned())
}
