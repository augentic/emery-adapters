//! Recognises Python surfaces and derives their metadata.

use std::collections::BTreeMap;

use emery_sdk::survey::code::{
    Arg, BindingKind, Bootstrap, Call, ClassDecl, Decorated, Derived, Export, ExportKind, Imported,
    Init, Manifest, MemberKind, Parsed, Recogniser, Runs, Scope, Tree,
};
use emery_sdk::survey::tests::Statement;
use emery_sdk::survey::{Lines, route};
use emery_sdk::{Route, Trigger, kebab, push_unique, unique};

use super::DIALECT;
use super::parse::{self, Module};
use super::resolve::{self, Resolver};

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
const MOUNTING: &[&str] = &["register_blueprint", "include_router", "mount"];
const PREFIXED: &[&str] = &["APIRouter", "Blueprint", "Router"];

// The registrations that serve `GET` where no `methods` keyword states a
// verb, as Flask's `route` and FastAPI's `api_route` do; a `path` or `url`
// hands the method to its view, and a websocket serves none, so neither
// types a route.
const DEFAULT_GET: &[&str] = &["route", "api_route", "add_api_route", "add_url_rule", "add_route"];
// The methods and decorators that register a handler for a message by name:
// a subscription, a consumer, a socket event. A task is none, since its name
// is the function's and no message's.
const SUBSCRIBING: &[&str] = &["subscribe", "subscriber", "consume", "basic_consume", "on"];
// The events a socket's `on` hears about the connection, never a caller.
const CONNECTION_EVENTS: &[&str] = &["connect", "disconnect", "connect_error", "error", "message"];

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
    pub fn new(parsed: &Parsed<Module>) -> Self {
        let manifest = resolve::Manifest::read(&parsed.root);
        let resolver = Resolver::new(parsed, manifest.name.as_deref());
        Self { resolver, manifest }
    }

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

    fn at_registration(
        &self, tree: &Tree<Self>, module: &Module, call: &Call, stem: &str,
    ) -> Derived {
        let (method, literal) = call.registered(tree.leading_literal(module, call));
        let literal = literal.as_deref();
        let route = routed(tree.mount(&module.path), method, literal);
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
        let (methods, class) = match self.class_run_by(tree, module, call) {
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
            trigger: route.as_deref().map_or_else(
                || typed_message(method, literal),
                |route| typed_route(method, methods_keyword, route),
            ),
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

    fn hands_bare_class(
        &self, tree: &Tree<Self>, module: &Module, arg: &Arg, frames: &[u32],
    ) -> bool {
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

    fn class_run_by<'t>(
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

    fn mounts(&self, tree: &Tree<Self>) -> BTreeMap<String, String> {
        let mut mounts: BTreeMap<String, String> = BTreeMap::new();

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

    fn derive(
        &self, tree: &Tree<Self>, module: &Module, lines: Lines, name: &str, stem: &str,
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
            Some(At::Registration(call)) => self.at_registration(tree, module, call, stem),
            Some(At::Class(class)) => at_decorated_class(tree, module, class),
            Some(At::Def(decorated)) => at_decorated_def(tree, module, decorated),
            Some(At::Export(export)) => at_export(tree, module, export),
            None => Derived::named(tree, module, lines, name, stem),
        }
    }
}

fn routed(mount: &str, method: &str, literal: Option<&str>) -> Option<String> {
    let literal = literal?;
    let verb = VERBS.contains(&method) && literal.starts_with('/');
    (verb || ROUTERS.contains(&method)).then(|| route::join(mount, &strip_pattern(literal)))
}

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

// The route a registration serves, typed, where its method is certain: a
// verb, or a `DEFAULT_GET` registration read with its `methods` keyword; a
// `trace`, and a path the grammar refuses, leave the surface unrouted.
fn typed_route(method: &str, methods: Option<&str>, path: &str) -> Option<Trigger> {
    if !(VERBS.contains(&method) || DEFAULT_GET.contains(&method)) {
        return None;
    }
    Route::parse(&format!("{} {path}", verb_of(method, methods))).ok().map(Trigger::Http)
}

// The message a registration or decorator subscribes its handler to, typed,
// where its method is one of `SUBSCRIBING`: a connection event, and a name
// the grammar refuses, leave the surface with no trigger.
fn typed_message(method: &str, literal: Option<&str>) -> Option<Trigger> {
    let name = literal.filter(|name| !name.starts_with('/'))?;
    let subscribing = SUBSCRIBING.contains(&method) && !CONNECTION_EVENTS.contains(&name);
    subscribing.then(|| Trigger::message(name).ok()).flatten()
}

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
        trigger: None,
    })
}

fn at_decorated_class(tree: &Tree<Python>, module: &Module, class: &Decorated) -> Derived {
    let name = class.class.as_deref().unwrap_or_default();
    let mount = tree.mount(&module.path);
    let route = route::join(mount, &strip_pattern(class.literal.as_deref().unwrap_or("")));
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
        trigger: None,
    }
}

fn at_decorated_def(tree: &Tree<Python>, module: &Module, decorated: &Decorated) -> Derived {
    let member = decorated.member.as_deref().unwrap_or_default();
    let mut detail = vec![decorated.class.as_ref().map_or_else(
        || format!("def `{member}` {}", decorated.lines),
        |class| format!("method `{class}.{member}` {}", decorated.lines),
    )];
    detail.push(format!("under `@{}`", spelled(decorated)));
    detail.extend(through(tree, module, decorated));
    let mount = tree.mount(&module.path);
    let route = decorated_route(mount, module, decorated);
    let group = group_of(module, decorated);
    let literal = || decorated.literal.as_deref().and_then(route::literal_stem);
    let stem = route.as_deref().map_or_else(
        || group.clone().or_else(literal),
        |route| parent_stem(mount, module, decorated).or_else(|| DIALECT.route.stem(route)),
    );
    let method = decorated.name.last().map_or("", String::as_str).to_ascii_lowercase();
    let discriminator = match (&route, &stem) {
        (Some(route), Some(stem)) => DIALECT
            .route
            .discriminator(&verb_of(&method, decorated.keyword("methods")), route, stem)
            .or_else(|| kebab(member)),
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
        trigger: route.as_deref().map_or_else(
            || typed_message(&method, decorated.literal.as_deref()),
            |route| typed_route(&method, decorated.keyword("methods"), route),
        ),
    }
}

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
        trigger: None,
    }
}

fn parent_stem(mount: &str, module: &Module, decorated: &Decorated) -> Option<String> {
    let prefix = decorated
        .class
        .as_deref()
        .and_then(|class| class_prefix(module, class))
        .and_then(|d| d.literal.as_deref())
        .unwrap_or("");
    DIALECT.route.stem(&route::join(mount, prefix)).or_else(|| router_tag(module, decorated))
}

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

fn decorated_route(mount: &str, module: &Module, decorated: &Decorated) -> Option<String> {
    let method = decorated.name.last()?.to_ascii_lowercase();
    if !(VERBS.contains(&method.as_str()) || ROUTERS.contains(&method.as_str())) {
        return None;
    }
    let literal = decorated.literal.as_deref().unwrap_or("");
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

fn dunder(name: &str) -> bool {
    name.starts_with("__") && name.ends_with("__")
}

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

fn strip_pattern(route: &str) -> String {
    route.trim_start_matches('^').trim_end_matches('$').to_owned()
}

fn literal_tail(literal: &str) -> Option<String> {
    route::literal_stem(literal)?;
    let word = literal.split_whitespace().next()?;
    let (_, tail) = word.split_once('.')?;
    kebab(tail)
}
