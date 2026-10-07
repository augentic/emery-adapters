//! Recognises what Python alone decides of a tree's surfaces.
//!
//! A surface is where control enters from outside the process: the
//! bootstrap, a handler handed to something a package provides, a `def` or
//! class under a package's decorator, or, in a library, what an entry
//! module exports. The SDK's pipeline lays the facts, puts the survey turn,
//! holds the answer to the tree, and cuts the seams; what it asks of this
//! adapter is what the language decides — where each import leads, what
//! the manifest declares, what a test states, which entry runs when the
//! tree loads and how, what a call hands and which class it hands as it
//! is, where blueprints and routers are mounted, which classes declare data
//! alone, and what the code says at an anchor the survey accepted: the stem
//! a route or literal spells, the id that tells the surface from the others
//! under its stem, a class's methods, and the modules the surface reaches.
//!
//! The rules read shapes, never a framework's name: a call handed a
//! function, a decorator with a literal, an export. Two exceptions spell
//! names. A decorator among the lifecycle hooks the data model lists
//! (`Decorated::registering`) shapes a handler rather than registers it,
//! which keeps a verb decorator beneath one of them the surface. A module
//! under `management/commands/` is read as a file-routed framework's module
//! is.

use std::collections::BTreeMap;

use emery_sdk::kebab;
use emery_sdk::survey::code::{
    Arg, BindingKind, Bootstrap, Call, ClassDecl, Decorated, Derived, Export, ExportKind, Imported,
    Init, Manifest, MemberKind, Parsed, Recogniser, Runs, Scope, Tree,
};
use emery_sdk::survey::tests::Statement;
use emery_sdk::survey::{Lines, route};

use super::parse::{self, Module};
use super::resolve::{self, Resolver};
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

// What the adapter recognises of a tree, over its resolver and manifest.
#[derive(Debug)]
pub struct Python {
    resolver: Resolver,
    manifest: resolve::Manifest,
}

enum At<'m> {
    Registration(&'m Call),
    Class(&'m Decorated),
    Def(&'m Decorated),
    Export(&'m Export),
}

impl Python {
    // The manifest read and the resolver built over the parsed tree, before
    // any import is settled. The tree's tests are the test modules the keep
    // set aside, which an import may still reach.
    pub(super) fn new(parsed: &Parsed<Module>) -> Self {
        let manifest = resolve::Manifest::read(&parsed.root);
        let resolver = Resolver::new(
            parsed.modules.keys().cloned(),
            parsed.data.iter().cloned(),
            parsed.tests.iter().cloned(),
            manifest.name.as_deref(),
        );
        Self { resolver, manifest }
    }

    // A module-level binding constructed from a package's class
    // (`app = FastAPI()`) or a function of the tree (`app = create_app()`)
    // runs at load, as a discarded call at module level does.
    fn runs_at_load(tree: &Tree<Self>, module: &Module) -> Option<Runs> {
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
            from_package || Self::tree_function(tree, module, head)
        });
        (discards || constructs).then_some(Runs::Load)
    }

    // A console script's target may reach its entry through an import from
    // the module that declares it.
    fn tree_binding(tree: &Tree<Self>, module: &Module, name: &str) -> bool {
        module.binding(name, &[]).is_some()
            || tree
                .exporter(module, name)
                .is_some_and(|(target, exported)| target.export(&exported).is_some())
    }

    fn tree_function(tree: &Tree<Self>, module: &Module, name: &str) -> bool {
        if let Some(binding) = module.binding(name, &[]) {
            return matches!(binding.kind, BindingKind::Function);
        }
        tree.exporter(module, name).is_some_and(|(target, exported)| {
            target.export(&exported).is_some_and(|export| export.kind == ExportKind::Function)
        })
    }

    // `views.list_orders` under `from . import views`, `list_orders` under
    // `from .views import list_orders`, and `a.b.c.run` under `import a.b.c`
    // each name a member of a module of the tree.
    fn member_of<'t>(
        tree: &'t Tree<Self>, module: &Module, root: &[String],
    ) -> Option<(&'t Module, String)> {
        let [name, rest @ ..] = root else { return None };
        let import = module.import(name)?;
        let target = tree.modules.get(import.target.as_ref()?.module()?)?;
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

    // The function, or the class handed by `as_view()`, kebab-cased.
    fn named_handler(&self, tree: &Tree<Self>, module: &Module, call: &Call) -> Option<String> {
        call.args
            .iter()
            .filter(|arg| !arg.is_hook(&DIALECT) && arg.root.is_some())
            .find(|arg| self.handler(tree, module, arg, &call.frames))
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

    // The stem is what the route or literal spells. The tell is the handler
    // handed by name, unless it is the stem itself, else the verb and the path
    // past the resource, else the registering method. A handed class's methods
    // and what its module references are reached too.
    fn at_registration(
        &self, tree: &Tree<Self>, module: &Module, call: &Call, stem: &str,
        mounts: &BTreeMap<String, String>,
    ) -> Derived {
        let (method, literal) = call.registered(tree.led(module, call));
        let literal = literal.as_deref();
        let route = routed(module, method, literal, mounts);
        let derived = route.as_deref().map_or_else(
            || literal.and_then(route::literal_stem),
            |route| DIALECT.route.stem(route),
        );
        let stem = derived.as_deref().unwrap_or(stem);
        let methods_keyword = call.keyword("methods").map(|arg| arg.head.as_str());
        let discriminator = self
            .named_handler(tree, module, call)
            .filter(|handler| handler != stem)
            .or_else(|| {
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
        let (methods, class) = match self.handed_class(tree, module, call) {
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
}

impl Recogniser for Python {
    type Module = Module;

    fn settle(&self, module: &mut Module) {
        self.resolver.settle(module);
    }

    fn manifest(&self) -> Manifest {
        self.manifest.summary(&self.resolver)
    }

    fn statements(&self, module: &Module) -> Vec<Statement> {
        module.statements()
    }

    // The first entry that runs something, probed in this order:
    // - the entries the manifest's scripts name
    // - the conventional entries the tree holds
    // - the one module of a tree of one
    // An entry runs by a console script whose function it declares or
    // imports from a module of the tree that does, by a `__main__` guard, or
    // at load. An entry that only declares is a library's.
    fn bootstrap<'t>(&self, tree: &'t Tree<Self>) -> Option<Bootstrap<'t, Module>> {
        let scripts =
            self.manifest.targets(&self.resolver).into_iter().filter_map(|(entry, function)| {
                let module = tree.modules.get(&entry)?;
                let script = self
                    .manifest
                    .scripts
                    .iter()
                    .find(|(_, target)| self.resolver.entry(target).as_deref() == Some(&entry))
                    .map(|(name, _)| name.clone());
                let runs = match (function, script) {
                    (Some(function), Some(name)) if Self::tree_binding(tree, module, &function) => {
                        Some(Runs::Script { name, function })
                    }
                    _ => Self::runs_at_load(tree, module),
                };
                Some((module, runs))
            });
        let conventional = BOOTSTRAPS
            .iter()
            .filter_map(|candidate| self.resolver.first(&[candidate]))
            .filter_map(|entry| tree.modules.get(&entry))
            .map(|module| (module, Self::runs_at_load(tree, module)));
        let alone = (tree.modules.len() == 1)
            .then(|| tree.modules.values().next())
            .flatten()
            .map(|module| (module, Self::runs_at_load(tree, module)));
        scripts
            .chain(conventional)
            .chain(alone)
            .find_map(|(module, runs)| Some(Bootstrap { module, runs: runs? }))
    }

    // A lambda, a view class handed by `as_view()`, a call carrying one, or
    // a name bound to a function or class of the tree, here or through an
    // import.
    fn handler(&self, tree: &Tree<Self>, module: &Module, arg: &Arg, frames: &[u32]) -> bool {
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
        let Some((target, member)) = Self::member_of(tree, module, root) else { return false };
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
    fn class_handed(&self, tree: &Tree<Self>, module: &Module, arg: &Arg, frames: &[u32]) -> bool {
        if arg.function || arg.called {
            return false;
        }
        let Some(root) = arg.root.as_deref() else { return false };
        let [name, rest @ ..] = root else { return false };
        if let Some(binding) = module.binding(name, frames) {
            return rest.is_empty() && matches!(binding.kind, BindingKind::Class { .. });
        }
        Self::member_of(tree, module, root)
            .and_then(|(target, member)| target.export(&member))
            .is_some_and(|export| export.kind == ExportKind::Class)
    }

    // A view handed by `as_view()`, a view set, a worker class, with the
    // module declaring it.
    fn handed_class<'t>(
        &self, tree: &'t Tree<Self>, module: &'t Module, call: &Call,
    ) -> Option<(&'t Module, String)> {
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
            let (target, member) = Self::member_of(tree, module, root)?;
            (target.export(&member)?.kind == ExportKind::Class).then_some((target, member))
        })
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
    fn mounts(&self, tree: &Tree<Self>) -> BTreeMap<String, String> {
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
                            let Some(head) = arg.root.as_deref().and_then(|root| root.first())
                            else {
                                continue;
                            };
                            let (target, under) = match tree.exporter(module, head) {
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

    fn declares_data(&self, class: &ClassDecl) -> bool {
        parse::declares_data(class)
    }

    // A module under `management/commands/` is read by its path before any
    // anchor. Otherwise the first registration, decorated class or `def`, or
    // export starting within `lines` is read. Failing one, the one enclosing
    // the first line is read, for an anchor within a handler.
    fn derive(
        &self, tree: &Tree<Self>, module: &Module, lines: Lines, name: &str, stem: &str,
        mounts: &BTreeMap<String, String>,
    ) -> Derived {
        if let Some(derived) = at_command(tree, module) {
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
            Some(At::Registration(call)) => self.at_registration(tree, module, call, stem, mounts),
            Some(At::Class(class)) => at_decorated_class(tree, module, class, mounts),
            Some(At::Def(decorated)) => at_decorated_def(tree, module, decorated, mounts),
            Some(At::Export(export)) => at_export(tree, module, export),
            None => Derived::named(tree, module, lines, name, stem),
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

// A management command is named by its file and declared by its `Command`
// class.
fn at_command(tree: &Tree<Python>, module: &Module) -> Option<Derived> {
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

// The stem is what the decorator's prefix spells under the module's mount;
// the tell is the class's name.
fn at_decorated_class(
    tree: &Tree<Python>, module: &Module, class: &Decorated, mounts: &BTreeMap<String, String>,
) -> Derived {
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
    let mut detail = vec![format!("class `{name}` {} under `@{}`", class.lines, spelled(class))];
    detail.extend(through(tree, module, class));
    detail.extend(noted(&methods));
    Derived {
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
    tree: &Tree<Python>, module: &Module, decorated: &Decorated, mounts: &BTreeMap<String, String>,
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
fn at_export(tree: &Tree<Python>, module: &Module, export: &Export) -> Derived {
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
        ExportKind::Value | ExportKind::Type | ExportKind::Unknown => ("value", Vec::new(), None),
    };
    let declared = module.declared_at(export);
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

fn through(tree: &Tree<Python>, module: &Module, decorated: &Decorated) -> Option<String> {
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

// `invoices.remind` is told from `invoices.void` by `remind`. Nothing for a
// literal of one segment or one naming nothing.
fn literal_tail(literal: &str) -> Option<String> {
    route::literal_stem(literal)?;
    let word = literal.split_whitespace().next()?;
    let (_, tail) = word.split_once('.')?;
    kebab(tail)
}
