//! Recognises TypeScript and JavaScript surfaces and derives their metadata.

use std::collections::BTreeMap;

use emery_sdk::survey::code::{
    Arg, BindingKind, Bootstrap, Call, Decorated, Derived, Export, ExportKind, Imported, Init,
    Manifest, MemberKind, Parsed, Recogniser, Runs, Tree,
};
use emery_sdk::survey::resolve::{Target, normalize};
use emery_sdk::survey::tests::Statement;
use emery_sdk::survey::{Lines, route};
use emery_sdk::{Route, Trigger, kebab, unique};

use super::DIALECT;
use super::parse::Module;
use super::resolve::{self, Resolver};

const BOOTSTRAPS: &[&str] =
    &["src/index.ts", "src/main.ts", "index.ts", "main.ts", "src/server.ts", "src/app.ts"];

const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options", "all"];
const MOUNTING: &[&str] = &["use", "register", "mount", "plugin"];
// The methods, beside the dialect's listeners, that register a handler for a
// message by name; and the decorators that do.
const SUBSCRIBING: &[&str] = &["subscribe", "consume"];
const MESSAGE_DECORATORS: &[&str] = &["MessagePattern", "EventPattern", "SubscribeMessage"];
// A stream's payload events, which name the arrival and not the message.
const GENERIC_EVENTS: &[&str] = &["message", "data"];

#[derive(Debug)]
pub struct TypeScript {
    resolver: Resolver,
    manifest: resolve::Manifest,
}

impl TypeScript {
    pub fn new(parsed: &Parsed<Module>) -> Self {
        Self {
            resolver: Resolver::new(parsed),
            manifest: resolve::Manifest::read(&parsed.root),
        }
    }

    fn named_handler(&self, tree: &Tree<Self>, module: &Module, call: &Call) -> Option<String> {
        call.args
            .iter()
            .find(|arg| self.handler(tree, module, arg, &call.frames))
            .filter(|arg| !arg.function)
            .and_then(|arg| kebab(arg.root.as_ref()?.last()?))
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
        let discriminator = self.named_handler(tree, module, call).or_else(|| {
            route.as_deref().map_or_else(
                || kebab(method),
                |route| DIALECT.route.discriminator(method, route, stem),
            )
        });
        let mut detail = vec![match (&call.class, &call.function) {
            (Some(class), Some(function)) => {
                format!("registered {} in `{class}.{function}`", call.lines)
            }
            (None, Some(function)) => format!("registered {} in `{function}`", call.lines),
            _ => format!("registered {} at module level", call.lines),
        }];
        if let Some(handler) = call.args.iter().find(|arg| arg.function) {
            detail.push(format!("handler {}", handler.lines));
        }
        if let Some(receiver) = tree.receiver(module, call) {
            detail.push(format!("through `{}`", receiver.package));
        }
        Derived {
            stem: derived,
            discriminator,
            methods: Vec::new(),
            lines: call.lines,
            detail,
            closure: tree.reaches(module, call.lines, &call.frames, call.class.as_deref()),
            trigger: route
                .as_deref()
                .map_or_else(|| typed_message(method, literal), |route| typed_route(method, route)),
        }
    }
}

// The route a registration serves, typed, where its verb is one HTTP
// method: `all`, and a path the grammar refuses, leave the surface unrouted.
fn typed_route(verb: &str, path: &str) -> Option<Trigger> {
    Route::parse(&format!("{verb} {path}")).ok().map(Trigger::Http)
}

// The message a registration subscribes its handler to, typed, where its
// method listens or subscribes by name: a lifecycle event, a stream's payload
// event, and a name the grammar refuses leave the surface with no trigger.
fn typed_message(method: &str, literal: Option<&str>) -> Option<Trigger> {
    let name = literal.filter(|name| is_message(name))?;
    let subscribing = DIALECT.listeners.contains(&method) || SUBSCRIBING.contains(&method);
    let named = !DIALECT.lifecycle_events.contains(&name) && !GENERIC_EVENTS.contains(&name);
    (subscribing && named).then(|| Trigger::message(name).ok()).flatten()
}

fn is_message(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '~' | '%' | '@')
        })
}

impl Recogniser for TypeScript {
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

    // An entry that only declares and exports is a library barrel rather than a
    // bootstrap.
    fn bootstrap<'t>(&self, tree: &'t Tree<Self>) -> Option<Bootstrap<'t, Module>> {
        let conventional =
            BOOTSTRAPS.iter().filter_map(|candidate| self.resolver.first(&[candidate]));
        let module = tree
            .manifest
            .entries
            .iter()
            .cloned()
            .chain(conventional)
            .filter_map(|entry| tree.modules.get(&entry))
            .find(|module| {
                module.calls.iter().any(|call| {
                    call.discarded()
                        && call.depth == 0
                        && call.function.is_none()
                        && call.class.is_none()
                }) || module.imports.iter().any(|import| import.imported == Imported::Effect)
            })?;
        Some(Bootstrap {
            module,
            runs: Runs::Load,
        })
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
                        | BindingKind::Value {
                            init: Init::Function,
                            ..
                        }
                );
        }
        let Some(import) = module.import(name) else { return false };
        let Some(target) = import.target.as_ref().and_then(Target::module) else { return false };
        let Some(target) = tree.modules.get(target) else { return false };
        let exported = match (&import.imported, rest) {
            (Imported::Named(exported), []) => exported.as_str(),
            (Imported::Default, []) => "default",
            (Imported::Whole, [member]) => member.as_str(),
            _ => return false,
        };
        let Some(export) = target.export(exported) else { return false };
        match &export.kind {
            ExportKind::Function => true,
            ExportKind::Value => target.exported_binding(exported).is_some_and(|binding| {
                matches!(
                    binding.kind,
                    BindingKind::Value {
                        init: Init::Function,
                        ..
                    }
                )
            }),
            _ => false,
        }
    }

    fn mounts(&self, tree: &Tree<Self>) -> BTreeMap<String, String> {
        let mut mounts = BTreeMap::new();
        for module in tree.modules.values() {
            for call in &module.calls {
                if !MOUNTING.contains(&call.method()) {
                    continue;
                }
                if call.method() == "use"
                    && let Some(prefix) = call.literal().filter(|literal| literal.starts_with('/'))
                {
                    for arg in call.args.iter().skip(1) {
                        let Some(head) = arg.root.as_deref().and_then(|root| root.first()) else {
                            continue;
                        };
                        let Some((target, _)) = tree.exporter(module, head) else { continue };
                        mounts.entry(target.path.clone()).or_insert_with(|| prefix.to_owned());
                    }
                }
                for arg in &call.args {
                    let Some(dir) = arg.property("dir").filter(|dir| dir.relative) else {
                        continue;
                    };
                    let from = module.path.rsplit_once('/').map_or("", |(dir, _)| dir);
                    let Some(root) = normalize(from, &dir.value) else { continue };
                    let base = arg
                        .property("prefix")
                        .or_else(|| arg.property("options.prefix"))
                        .map_or("", |prefix| prefix.value.as_str());
                    for path in tree.modules.keys() {
                        let Some(beneath) = path.strip_prefix(&format!("{root}/")) else {
                            continue;
                        };
                        let dirs = beneath.rsplit_once('/').map_or("", |(dirs, _)| dirs);
                        let prefix = route::join(base, dirs);
                        if prefix != "/" {
                            mounts.entry(path.clone()).or_insert(prefix);
                        }
                    }
                }
            }
        }
        mounts
    }

    fn derive(
        &self, tree: &Tree<Self>, module: &Module, lines: Lines, name: &str, stem: &str,
    ) -> Derived {
        if let Some(call) = tree
            .registration_at(module, lines)
            .or_else(|| tree.registration_enclosing(module, lines))
        {
            return self.at_registration(tree, module, call, stem);
        }
        if let Some(class) = decorated_class_at(module, lines) {
            return at_decorated_class(tree, module, class);
        }
        if let Some(decorated) =
            decorated_at(module, lines).or_else(|| decorated_enclosing(module, lines))
        {
            return at_decorated_method(tree, module, decorated);
        }
        if let Some(export) = export_at(module, lines).or_else(|| export_enclosing(module, lines)) {
            return at_export(tree, module, export, stem);
        }
        Derived::named(tree, module, lines, name, stem)
    }
}

fn routed(mount: &str, method: &str, literal: Option<&str>) -> Option<String> {
    let path = literal.filter(|literal| literal.starts_with('/'))?;
    VERBS.contains(&method).then(|| route::join(mount, path))
}

fn at_decorated_class(tree: &Tree<TypeScript>, module: &Module, class: &Decorated) -> Derived {
    let name = class.class.as_deref().unwrap_or_default();
    let route = route::join(class.literal.as_deref().unwrap_or(""), "");
    let methods = unique(
        module
            .decorated
            .iter()
            .filter(|d| d.class == class.class && d.registering(&DIALECT))
            .filter_map(|d| Some((d.member.clone()?, d.lines))),
    );
    let mut detail = vec![format!("class `{name}` {} under `@{}`", class.lines, spelled(class))];
    detail.extend(through(module, class));
    detail.extend(noted(&methods));
    Derived {
        stem: DIALECT.route.stem(&route),
        discriminator: kebab(name),
        methods,
        lines: class.lines,
        detail,
        closure: tree.reaches(module, class.lines, &[], class.class.as_deref()),
        trigger: None,
    }
}

fn at_decorated_method(tree: &Tree<TypeScript>, module: &Module, decorated: &Decorated) -> Derived {
    let class = decorated.class.as_deref().unwrap_or_default();
    let member = decorated.member.as_deref().unwrap_or_default();
    let mut detail = vec![format!("method `{class}.{member}` {}", decorated.lines)];
    detail.extend(through(module, decorated));
    let routed = decorated_route(module, decorated);
    Derived {
        stem: routed.as_ref().map_or_else(
            || decorated.literal.as_deref().and_then(route::literal_stem),
            |(_, route)| DIALECT.route.stem(route),
        ),
        discriminator: kebab(member),
        methods: Vec::new(),
        lines: decorated.lines,
        detail,
        closure: tree.reaches(module, decorated.lines, &[], decorated.class.as_deref()),
        trigger: routed
            .as_ref()
            .map_or_else(|| decorated_message(decorated), |(verb, route)| typed_route(verb, route)),
    }
}

// The message a method under a message decorator consumes, typed.
fn decorated_message(decorated: &Decorated) -> Option<Trigger> {
    let name = decorated.name.last()?;
    let literal = decorated.literal.as_deref().filter(|name| is_message(name))?;
    MESSAGE_DECORATORS.contains(&name.as_str()).then(|| Trigger::message(literal).ok()).flatten()
}

fn at_export(tree: &Tree<TypeScript>, module: &Module, export: &Export, stem: &str) -> Derived {
    let local = export.local.as_deref().unwrap_or(&export.name);
    let display = if export.name == "default" { module_stem(module) } else { export.name.clone() };
    let mut parts = vec![display];
    parts.extend(file_routed(&module.path, stem));
    let (what, methods, class) = match export.kind {
        ExportKind::Class => (
            "class",
            methods(module, local)
                .into_iter()
                .map(|(name, lines)| (name.to_owned(), lines))
                .collect(),
            Some(local),
        ),
        ExportKind::Function => ("function", Vec::new(), None),
        ExportKind::Value | ExportKind::Type | ExportKind::Unknown => ("value", Vec::new(), None),
    };
    let declared = module.declared_at(export);
    let mut detail = vec![format!("exported {what} {declared}")];
    detail.extend(noted(&methods));
    Derived {
        stem: None,
        discriminator: kebab(&parts.join("-")),
        methods,
        lines: declared,
        detail,
        closure: tree.reaches(module, declared, &[], class),
        trigger: None,
    }
}

fn spelled(decorated: &Decorated) -> String {
    let argument = decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
    format!("{}{argument}", decorated.name.join("."))
}

fn through(module: &Module, decorated: &Decorated) -> Option<String> {
    let package = module.package(decorated.name.first()?)?;
    Some(format!("through `{package}`"))
}

fn noted(methods: &[(String, Lines)]) -> Option<String> {
    if methods.is_empty() {
        return None;
    }
    let listed: Vec<String> =
        methods.iter().map(|(name, lines)| format!("`{name}` {lines}")).collect();
    Some(format!("methods {}", listed.join(", ")))
}

fn class_prefix<'m>(module: &'m Module, class: Option<&str>) -> Option<&'m Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.class.as_deref() == class && d.member.is_none() && d.literal.is_some())
        .find(|d| d.registering(&DIALECT))
}

// The verb and the path a method's decorator registers under its class's
// prefix, `@Get(':id')` under `@Controller('orders')`; `None` for a
// decorator that is no verb.
fn decorated_route(module: &Module, decorated: &Decorated) -> Option<(String, String)> {
    let verb = decorated.name.last()?.to_ascii_lowercase();
    if !VERBS.contains(&verb.as_str()) {
        return None;
    }
    let prefix =
        class_prefix(module, decorated.class.as_deref()).and_then(|d| d.literal.as_deref());
    let route = route::join(prefix.unwrap_or(""), decorated.literal.as_deref().unwrap_or(""));
    Some((verb, route))
}

fn decorated_class_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.member.is_none() && d.registering(&DIALECT))
        .find(|d| lines.holds(d.lines.start))
}

fn decorated_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    registering_methods(module).find(|d| lines.holds(d.lines.start))
}

fn decorated_enclosing(module: &Module, lines: Lines) -> Option<&Decorated> {
    registering_methods(module).find(|d| d.lines.holds(lines.start))
}

fn registering_methods(module: &Module) -> impl Iterator<Item = &Decorated> {
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

fn file_routed(path: &str, stem: &str) -> Vec<String> {
    let dirs: Vec<&str> = path
        .rsplit_once('/')
        .map_or("", |(dir, _)| dir)
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    let Some(at) = dirs.iter().position(|segment| kebab(segment).as_deref() == Some(stem)) else {
        return Vec::new();
    };
    dirs[at + 1..]
        .iter()
        .filter(|segment| !(segment.starts_with('(') && segment.ends_with(')')))
        .map(|segment| segment.trim_matches(|c| matches!(c, '[' | ']' | ':' | '.')).to_owned())
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn methods<'m>(module: &'m Module, name: &str) -> Vec<(&'m str, Lines)> {
    module
        .class(name)
        .map(|class| {
            class
                .members
                .iter()
                .filter(|m| !m.private && matches!(m.kind, MemberKind::Method | MemberKind::Getter))
                .map(|m| (m.name.as_str(), m.lines))
                .collect()
        })
        .unwrap_or_default()
}

fn module_stem(module: &Module) -> String {
    kebab(module.stem(&DIALECT)).unwrap_or_else(|| "module".to_owned())
}
