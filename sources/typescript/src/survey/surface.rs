//! Derives what the code says of a tree's surfaces from the anchors the
//! survey accepted.
//!
//! A surface is where control enters from outside the process — the
//! bootstrap the manifest names, a handler handed to something a package
//! provides, a method under a package's decorator, or, in a library, what
//! the entry module exports — and the survey names each at the lines that
//! register or declare it. From those lines the code here reads the rest:
//! the stem a route or literal spells, the id that tells the surface from
//! the others under its stem, a class's methods, and the modules the surface
//! reaches, so two runs that accept the same anchors lead their ids the same
//! way and reach the same modules. The rules read shapes — a call handed a
//! function, a decorator with a literal, an export — never a framework's
//! name; the one list that spells names, the decorators that shape a handler
//! rather than register it, is what keeps a verb decorator beneath one of
//! them the surface.

use std::collections::{BTreeMap, BTreeSet};

use super::parse::{
    Arg, Binding, BindingKind, Call, Decorated, Export, ExportKind, Imported, Init, Lines,
    MemberKind, Module, Scope,
};
use super::resolve::{Manifest, Resolver, Target, normalize};
use super::{push_unique, unique};

// The bootstrap modules looked for when the manifest names none.
const BOOTSTRAPS: &[&str] =
    &["src/index.ts", "src/main.ts", "index.ts", "main.ts", "src/server.ts", "src/app.ts"];

const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options", "all"];

// The methods that mount what they are handed — `use` a router under a path,
// any of them a loader over a directory — so a route registered in a mounted
// module is read under its mount.
const MOUNTING: &[&str] = &["use", "register", "mount", "plugin"];

// Path segments that version or namespace an API rather than name a surface.
const PATH_NOISE: &[&str] = &["api", "rest", "internal"];

// Method decorators that shape a handler another decorator registers.
const DECORATOR_NOISE: &[&str] = &[
    "UseGuards",
    "UseInterceptors",
    "UsePipes",
    "UseFilters",
    "HttpCode",
    "Header",
    "Redirect",
    "Bind",
    "SetMetadata",
    "Roles",
    "Public",
    "Version",
    "Transactional",
    "Injectable",
    "Inject",
    "SerializeOptions",
];

// A decorator that shapes what it decorates rather than register it: the
// noise list, and the documentation decorators (`@ApiTags`, `@ApiOkResponse`).
fn shapes(decorator: &str) -> bool {
    DECORATOR_NOISE.contains(&decorator) || decorator.starts_with("Api")
}

// How many bindings a receiver is traced through before it counts as local.
pub(super) const TRACE: usize = 4;

/// The parsed tree with what locates its modules.
#[derive(Debug)]
pub struct Tree {
    /// Every production module by its root-relative path, each import settled.
    pub modules: BTreeMap<String, Module>,
    /// What locates a module the manifest or a convention names.
    pub resolver: Resolver,
    /// What `package.json` says, empty for none.
    pub manifest: Manifest,
}

/// One place control enters the source from outside the process.
#[derive(Debug)]
pub struct Surface {
    /// What a caller does through the surface, as the survey named it.
    pub name: String,
    /// The module the surface is registered or declared in.
    pub entry: String,
    /// The stem every `requirement` and `criterion` of the surface leads with.
    pub stem: String,
    /// The registration or declaration.
    pub lines: Lines,
    /// Notes for the brief: where it is registered, its hooks, its methods.
    pub detail: Vec<String>,
    /// The modules the surface reaches, its entry first.
    pub closure: Vec<String>,
    /// What tells the surface from the others under its stem: its handler's
    /// name, its verb and the path past the resource, its method's name.
    pub discriminator: Option<String>,
    /// The public methods of an exported class, kebab-cased.
    pub methods: Vec<String>,
    /// The ids the surface's requirements lead with: its stem alone when it
    /// is the stem's one surface, `<stem>.<discriminator>` otherwise, and one
    /// more per method of a class.
    pub ids: Vec<String>,
}

impl Tree {
    /// The bootstrap module: the first of the entries the manifest names,
    /// then the conventional entries the tree holds, that loading runs
    /// something in — a call at module level whose value is discarded, or an
    /// import for its effect. An entry that only declares and exports is a
    /// library's — a `main` naming the barrel while `start` runs the server
    /// — and a tree with no entry that runs has no bootstrap.
    pub(super) fn bootstrap(&self) -> Option<&Module> {
        let conventional =
            BOOTSTRAPS.iter().filter_map(|candidate| self.resolver.first(&[candidate]));
        self.manifest
            .entries(&self.resolver)
            .into_iter()
            .chain(conventional)
            .filter_map(|entry| self.modules.get(&entry))
            .find(|module| {
                module.calls.iter().any(|call| {
                    call.discarded()
                        && call.depth == 0
                        && call.function.is_none()
                        && call.class.is_none()
                }) || module.imports.iter().any(|import| import.imported == Imported::Effect)
            })
    }

    // The module of the tree that exports what `local` imports in `module`,
    // with the name it is exported under.
    fn exporter(&self, module: &Module, local: &str) -> Option<(&Module, String)> {
        let import = module.import(local)?;
        let path = import.target.as_ref()?.module()?;
        let name = match &import.imported {
            Imported::Named(name) => name.clone(),
            Imported::Default => "default".to_owned(),
            Imported::Namespace | Imported::Effect => return None,
        };
        Some((self.modules.get(path)?, name))
    }

    /// The modules `seeds` reach through imports and re-exports, `seeds`
    /// first in their order, then breadth-first, once each. A module in
    /// `stop` is reached and not followed: another surface's entry is what
    /// the bootstrap mounts, not what it does.
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

    // The string a leading argument stands for: a literal, or an identifier
    // bound to one in this module or an in-tree module it imports.
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

    /// Whether an argument is a handler: a function, an object or call
    /// carrying one, or a name bound to a function in the tree — declared
    /// here, imported from a module of the tree, or a member of a namespace
    /// import (`handlers.list`).
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
                        | BindingKind::Value {
                            init: Init::Function,
                            ..
                        }
                );
        }
        let Some(import) = module.import(name) else { return false };
        let Some(target) = import.target.as_ref().and_then(Target::module) else { return false };
        let Some(target) = self.modules.get(target) else { return false };
        let exported = match (&import.imported, rest) {
            (Imported::Named(exported), []) => exported.as_str(),
            (Imported::Default, []) => "default",
            (Imported::Namespace, [member]) => member.as_str(),
            _ => return false,
        };
        let Some(export) = target.export(exported) else { return false };
        match &export.kind {
            ExportKind::Function => true,
            ExportKind::Value => {
                let local = export.local.as_deref().unwrap_or(&export.name);
                target.binding(local, &[]).is_some_and(|binding| {
                    matches!(
                        binding.kind,
                        BindingKind::Value {
                            init: Init::Function,
                            ..
                        }
                    )
                })
            }
            _ => false,
        }
    }

    /// What a call's receiver is, traced through the bindings that construct
    /// it to the package it comes from — a class of the tree's that extends a
    /// package's class among them, for a member it inherits; `None` for a
    /// receiver of the tree's own or of the runtime's.
    pub(super) fn receiver(&self, module: &Module, call: &Call) -> Option<Receiver> {
        let head = call.callee.head.as_str();
        let link = |index: usize| call.callee.links.get(index).map(|link| link.name.as_str());
        if head == "this" {
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

    // `member` is the member accessed on the receiver, which tells a member a
    // class of the tree's declares from one it inherits.
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
                        .filter(|init| init.is_new)
                        .map(|init| init.method().to_owned());
                    Some(Receiver {
                        package: traced.package,
                        type_name: constructed.or(traced.type_name),
                    })
                }
                // a member the class declares is the tree's own; one it
                // inherits from a package's class is that package's
                BindingKind::Class { extends: Some(base) } => {
                    let declared = module.classes.iter().any(|class| {
                        class.name == binding.name
                            && class.members.iter().any(|m| Some(m.name.as_str()) == member)
                    });
                    if declared {
                        return None;
                    }
                    let traced = self.trace(module, base, &[], None, budget - 1)?;
                    Some(Receiver {
                        type_name: traced.type_name.or_else(|| Some(base.clone())),
                        ..traced
                    })
                }
                _ => None,
            };
        }
        if let Some(package) = module.package(name) {
            return Some(Receiver {
                package: package.to_owned(),
                type_name: None,
            });
        }
        let (target, imported) = self.exporter(module, name)?;
        let binding = exported_binding(target, &imported)?;
        self.trace(target, &binding.name, &[], member, budget - 1)
    }

    // The modules the names referenced within `lines` of `module` reach:
    // imports, and the types of the parameters and fields in scope.
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

// Seeds the in-tree module `local` is imported from in `module`, once.
fn seed(seeds: &mut Vec<String>, module: &Module, local: &str) {
    if let Some(path) = module.imported(local) {
        push_unique(seeds, path.to_owned());
    }
}

// The module-level binding an export of `module` names.
fn exported_binding<'m>(module: &'m Module, name: &str) -> Option<&'m Binding> {
    let export = module.export(name)?;
    let local = export.local.as_deref().unwrap_or(&export.name);
    module.binding(local, &[])
}

/// A receiver traced to the package that provides it.
pub struct Receiver {
    /// The package, by its specifier as imported.
    pub package: String,
    /// The class it is typed or constructed as, when the code says.
    pub type_name: Option<String>,
}

// The ids each surface's requirements lead with, decided over the surfaces
// that share the tree — so `build` runs it once every named surface is
// derived: a stem's one surface is its stem; surfaces sharing a stem are
// told apart by what discriminates each, else by their name, else by their
// entry's module, and two still alike by the nearest segment of their
// entries' paths that spells neither the stem nor the id's own tail; a class
// carries an id per public method beside its own.
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

// The nearest segment of a module's path that spells neither `stem` nor the
// tail `own` already ends with — the module's stem, then each directory
// above it: `local/files.controller.ts` and `s3/files.controller.ts` under
// `files` are told apart by `local` and `s3`.
fn tells_apart(module: &Module, stem: &str, own: &str) -> Option<String> {
    let dir = module.path.rsplit_once('/').map_or("", |(dir, _)| dir);
    std::iter::once(module.stem())
        .chain(dir.rsplit('/'))
        .filter_map(kebab)
        .find(|segment| segment != stem && !own.ends_with(&format!(".{segment}")))
}

// The bootstrap surface: what the entry module does outside the handlers
// it registers, from its first line to its last, reaching the other
// surfaces' entries — what it mounts — and, through each, what that entry
// constructs, and no further.
pub(super) fn start(tree: &Tree, module: &Module, registered: &[Surface]) -> Surface {
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

    // an entry is reached and not followed, but what loading it constructs
    // — its class fields' types and initializers, its module-level bindings
    // — runs before any handler and is the bootstrap's to reach
    for entry in &entries {
        let Some(target) = tree.modules.get(entry) else { continue };
        for local in target.constructed() {
            seed(&mut seeds, target, local);
        }
    }
    Surface {
        name: "start".to_owned(),
        entry: module.path.clone(),
        stem: "start".to_owned(),
        lines: module.span,
        detail: vec![
            "the process bootstrap: what runs before each handler is registered, what it awaits \
             before serving, and at shutdown — `stop` and what a signal handler calls, wherever \
             declared"
                .to_owned(),
        ],
        closure: tree.closure(&seeds, &entries),
        discriminator: None,
        methods: Vec::new(),
        ids: vec!["start".to_owned()],
    }
}

// Where each module's routes are mounted: `app.use("/api", ordersRouter(..))`
// gives `routes/orders.ts` the prefix `/api`; a loader registered over a
// directory of the tree (`app.register(autoload, { dir: join(__dirname,
// "routes") })`) gives each module beneath it the path of its directory from
// there, under the loader's own `prefix` option where it names one, so
// `routes/api/auth/index.ts` is `/api/auth` and `routes/home.ts` is mounted
// at the root. The first mount read of a module stands.
pub(super) fn mounts(tree: &Tree) -> BTreeMap<String, String> {
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
                let Some(dir) = arg.property("dir").filter(|dir| dir.relative) else { continue };
                let from = module.path.rsplit_once('/').map_or("", |(dir, _)| dir);
                let Some(root) = normalize(from, &dir.value) else { continue };
                let base = arg
                    .property("prefix")
                    .or_else(|| arg.property("options.prefix"))
                    .map_or("", |prefix| prefix.value.as_str());
                for path in tree.modules.keys() {
                    let Some(beneath) = path.strip_prefix(&format!("{root}/")) else { continue };
                    let dirs = beneath.rsplit_once('/').map_or("", |(dirs, _)| dirs);
                    let prefix = join_route(base, dirs);
                    if prefix != "/" {
                        mounts.entry(path.clone()).or_insert(prefix);
                    }
                }
            }
        }
    }
    mounts
}

// The handler a registration is handed by name, kebab-cased.
fn named_handler(tree: &Tree, module: &Module, call: &Call) -> Option<String> {
    call.args
        .iter()
        .find(|arg| tree.handler(module, arg, &call.frames))
        .filter(|arg| !arg.function)
        .and_then(|arg| kebab(arg.root.as_ref()?.last()?))
}

// The literal that leads a call's arguments: a string literal, or a constant
// bound to one in the tree.
pub(super) fn led(tree: &Tree, module: &Module, call: &Call) -> Option<String> {
    call.args.first().and_then(|arg| tree.constant(module, arg, &call.frames))
}

// The method a registration is spelled by and the literal that leads it: a
// chain names itself at its first call with a literal (`command("x")`), else
// the call's own method and lead.
pub(super) fn registered(call: &Call, led: Option<String>) -> (&str, Option<String>) {
    let chained = call.callee.links.iter().find_map(|link| {
        let literal = link.call.as_ref()?.literal.clone()?;
        Some((link.name.as_str(), literal))
    });
    chained.map_or_else(|| (call.method(), led), |(method, literal)| (method, Some(literal)))
}

// The route a verb registers with a path literal, under the module's mount.
fn routed(
    module: &Module, method: &str, literal: Option<&str>, mounts: &BTreeMap<String, String>,
) -> Option<String> {
    let path = literal.filter(|literal| literal.starts_with('/'))?;
    VERBS.contains(&method).then(|| {
        let prefix = mounts.get(&module.path).map_or("", String::as_str);
        join_route(prefix, path)
    })
}

/// What code reads at an anchor the survey accepted.
#[derive(Debug)]
pub struct Derived {
    /// The stem the code at the anchor spells — a route's resource, a
    /// literal's first word — where it spells one; `None` leaves the
    /// survey's standing.
    pub stem: Option<String>,
    /// What tells the surface from the others under its stem.
    pub discriminator: Option<String>,
    /// The public methods of the class declared or decorated at the anchor,
    /// each with its lines, an id apiece.
    pub methods: Vec<(String, Lines)>,
    /// The registration or declaration the code found at the anchor, whole
    /// — where the survey cited one line of several — else the anchor.
    pub lines: Lines,
    /// Notes for the brief: where the surface is registered or declared,
    /// through which package, its handler, its methods.
    pub detail: Vec<String>,
    /// The modules the code at the anchor reaches, the module first.
    pub closure: Vec<String>,
}

// What the code at `lines` of `module` says of the surface the survey
// anchored there, derived as the parser derives its own. At a registration:
// the stem its route or literal spells, and what tells it apart — the
// handler handed by name, else the route's verb and path past the resource,
// else the registering method — reaching what the call and its handler
// reference. At a decorated class: the stem its prefix spells, the class's
// name, and its decorated methods. At a decorated method: the stem its route
// or literal spells under the class's prefix, and the method's name — either
// reaching what the class references and its fields are typed as. At an
// export: the export's name joined with the segments a file-routed module's
// path spells past its stem, and a class's public methods. Failing every
// one, the survey's own `name` less the stem and what only namespaces tells
// it apart, its stem stands, and the anchor's lines are read as they are.
pub(super) fn derive(
    tree: &Tree, module: &Module, lines: Lines, name: &str, stem: &str,
    mounts: &BTreeMap<String, String>,
) -> Derived {
    if let Some(call) = registration_at(tree, module, lines) {
        return at_registration(tree, module, call, stem, mounts);
    }
    if let Some(class) = decorated_class_at(module, lines) {
        return at_decorated_class(tree, module, class);
    }
    if let Some(decorated) = decorated_at(module, lines) {
        return at_decorated_method(tree, module, decorated);
    }
    if let Some(export) = export_at(module, lines) {
        return at_export(tree, module, export, stem);
    }
    Derived {
        stem: None,
        discriminator: normalised(name, stem),
        methods: Vec::new(),
        lines,
        detail: vec![format!("named by the survey at {lines}")],
        closure: tree.reaches(module, lines, &[], None),
    }
}

// What a registration says of the surface anchored at it.
fn at_registration(
    tree: &Tree, module: &Module, call: &Call, stem: &str, mounts: &BTreeMap<String, String>,
) -> Derived {
    let (method, literal) = registered(call, led(tree, module, call));
    let literal = literal.as_deref();
    let route = routed(module, method, literal, mounts);
    let derived = route.as_deref().map_or_else(|| literal.and_then(literal_stem), route_stem);
    let stem = derived.as_deref().unwrap_or(stem);
    let discriminator = named_handler(tree, module, call).or_else(|| {
        route
            .as_deref()
            .map_or_else(|| kebab(method), |route| route_discriminator(method, route, stem))
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
    }
}

// What a decorated class says of the surface anchored at it: its
// registering decorator's prefix, its name, and its decorated methods.
fn at_decorated_class(tree: &Tree, module: &Module, class: &Decorated) -> Derived {
    let route = join_route(class.literal.as_deref().unwrap_or(""), "");
    let methods = unique(
        module
            .decorated
            .iter()
            .filter(|d| d.class == class.class && registering(d))
            .filter_map(|d| Some((d.member.clone()?, d.lines))),
    );
    let mut detail =
        vec![format!("class `{}` {} under `@{}`", class.class, class.lines, spelled(class))];
    detail.extend(through(module, class));
    detail.extend(noted(&methods));
    Derived {
        stem: route_stem(&route),
        discriminator: kebab(&class.class),
        methods,
        lines: class.lines,
        detail,
        closure: tree.reaches(module, class.lines, &[], Some(&class.class)),
    }
}

// What a decorated method says of the surface anchored at it.
fn at_decorated_method(tree: &Tree, module: &Module, decorated: &Decorated) -> Derived {
    let member = decorated.member.as_deref().unwrap_or_default();
    let mut detail = vec![format!("method `{}.{member}` {}", decorated.class, decorated.lines)];
    detail.extend(through(module, decorated));
    Derived {
        stem: decorated_stem(module, decorated),
        discriminator: kebab(member),
        methods: Vec::new(),
        lines: decorated.lines,
        detail,
        closure: tree.reaches(module, decorated.lines, &[], Some(&decorated.class)),
    }
}

// What an export says of the surface anchored at it: its declaration, and
// a class's public methods.
fn at_export(tree: &Tree, module: &Module, export: &Export, stem: &str) -> Derived {
    let local = export.local.as_deref().unwrap_or(&export.name);
    let display = if export.name == "default" { module_stem(module) } else { export.name.clone() };
    let mut parts = vec![display];
    parts.extend(file_routed(&module.path, stem));
    let (what, methods, class) = match &export.kind {
        ExportKind::Class { .. } => (
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
    let declared = module.binding(local, &[]).map_or(export.lines, |binding| binding.lines);
    let mut detail = vec![format!("exported {what} {declared}")];
    detail.extend(noted(&methods));
    Derived {
        stem: None,
        discriminator: kebab(&parts.join("-")),
        methods,
        lines: declared,
        detail,
        closure: tree.reaches(module, declared, &[], class),
    }
}

// The decorator as the code spells it, for a note: `Controller("orders")`.
fn spelled(decorated: &Decorated) -> String {
    let argument = decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
    format!("{}{argument}", decorated.name.join("."))
}

// The package a decorator comes from, as a note, when it is a package's.
fn through(module: &Module, decorated: &Decorated) -> Option<String> {
    let package = module.package(decorated.name.first()?)?;
    Some(format!("through `{package}`"))
}

// A class's methods as a note, each with its lines; none for none.
fn noted(methods: &[(String, Lines)]) -> Option<String> {
    if methods.is_empty() {
        return None;
    }
    let listed: Vec<String> =
        methods.iter().map(|(name, lines)| format!("`{name}` {lines}")).collect();
    Some(format!("methods {}", listed.join(", ")))
}

// A decorator that registers what it decorates rather than shape it.
fn registering(decorated: &Decorated) -> bool {
    decorated.name.last().is_some_and(|name| !shapes(name))
}

// The class decorator that registers the class and carries its prefix
// (`@Controller("orders")`); one that documents it (`@ApiTags("Users")`)
// does not.
fn class_prefix<'m>(module: &'m Module, class: &str) -> Option<&'m Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.class == class && d.member.is_none() && d.literal.is_some())
        .find(|d| registering(d))
}

// The stem a decorated method's route or literal spells: a verb decorator's
// path under the class's prefix, by the route rule; another's literal, by
// its first word; none where it spells neither.
fn decorated_stem(module: &Module, decorated: &Decorated) -> Option<String> {
    let verb = decorated.name.last()?.to_ascii_lowercase();
    if VERBS.contains(&verb.as_str()) {
        let prefix = class_prefix(module, &decorated.class).and_then(|d| d.literal.as_deref());
        let route = join_route(prefix.unwrap_or(""), decorated.literal.as_deref().unwrap_or(""));
        route_stem(&route)
    } else {
        decorated.literal.as_deref().and_then(literal_stem)
    }
}

// The registering decorator of a class whose declaration starts within
// `lines` — an anchor at the class, or the module alone.
fn decorated_class_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.member.is_none() && registering(d))
        .find(|d| lines.holds(d.lines.start))
}

// The registering decorator of the method declared at `lines`: the first
// whose method starts within them, else the one enclosing their first line.
fn decorated_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    let methods = || module.decorated.iter().filter(|d| d.member.is_some() && registering(d));
    methods()
        .find(|d| lines.holds(d.lines.start))
        .or_else(|| methods().find(|d| d.lines.holds(lines.start)))
}

// Whether a call hands a function to something: a handler among its
// arguments, and not a structural call (`.then`, `.map`) that takes one.
fn hands(tree: &Tree, module: &Module, call: &Call) -> bool {
    !call.structural() && call.args.iter().any(|arg| tree.handler(module, arg, &call.frames))
}

// The receiver a call hands a function to, when the call is one outside any
// handler that hands one to something a package provides — where a
// framework, a queue, a scheduler, or a CLI is told what to run.
pub(super) fn handed(tree: &Tree, module: &Module, call: &Call) -> Option<Receiver> {
    (call.depth == 0 && hands(tree, module, call)).then(|| tree.receiver(module, call))?
}

// Whether a call has the shape of a registration, at any depth and whatever
// its receiver: it hands a function to something and is discarded,
// constructs, or is led by a literal. A wrapper handed a function for its
// value — `export default fp(async (app) => ..)` — has not: it defines a
// plugin, it registers nothing.
pub(super) fn registers(tree: &Tree, module: &Module, call: &Call) -> bool {
    hands(tree, module, call)
        && (call.discarded() || call.is_new || led(tree, module, call).is_some())
}

// The registration at `lines`: the first call of that shape starting within
// them — since the survey has decided the lines register a surface — else
// the one enclosing their first line, for an anchor within a handler.
fn registration_at<'m>(tree: &Tree, module: &'m Module, lines: Lines) -> Option<&'m Call> {
    let registers = |call: &&Call| registers(tree, module, call);
    module.calls.iter().filter(registers).find(|call| lines.holds(call.lines.start)).or_else(|| {
        module.calls.iter().filter(registers).find(|call| call.lines.holds(lines.start))
    })
}

// The export declared at `lines`: a function or class whose declaration
// starts within them, else a value's, else the one enclosing their first
// line.
fn export_at(module: &Module, lines: Lines) -> Option<&Export> {
    let declared = |export: &Export| {
        let local = export.local.as_deref().unwrap_or(&export.name);
        module.binding(local, &[]).map_or(export.lines, |binding| binding.lines)
    };
    let callable =
        |export: &&Export| matches!(export.kind, ExportKind::Function | ExportKind::Class { .. });
    let value = |export: &&Export| matches!(export.kind, ExportKind::Value);
    let exports = || module.exports.iter();
    exports()
        .filter(callable)
        .find(|export| lines.holds(declared(export).start))
        .or_else(|| exports().filter(value).find(|export| lines.holds(declared(export).start)))
        .or_else(|| {
            exports()
                .filter(|export| callable(export) || value(export))
                .find(|export| declared(export).holds(lines.start))
        })
}

// The directory segments a file-routed module's path spells past the one
// that spells its stem — a parameter by its bare name, a group by nothing:
// `app/api/orders/[id]/route.ts` under `orders` spells `id`. None when no
// segment spells the stem.
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

// The survey's own name for a surface as what tells it apart: kebab-cased,
// less the words of its stem and the segments that only version or
// namespace a path — `GET /api/customers` under `customers` is `get`. None
// when nothing is left.
fn normalised(name: &str, stem: &str) -> Option<String> {
    let spelled = kebab(name)?;
    let kept: Vec<&str> = spelled
        .split('-')
        .filter(|word| !stem.split('-').any(|own| own == *word) && names_resource(word))
        .collect();
    kebab(&kept.join("-"))
}

// The public methods and getters of the class `name` declares, each with
// its lines.
fn methods<'m>(module: &'m Module, name: &str) -> Vec<(&'m str, Lines)> {
    module
        .classes
        .iter()
        .find(|class| class.name == name)
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

// The modules no other module imports; every module where each is imported.
pub(super) fn roots(tree: &Tree) -> Vec<String> {
    let imported: BTreeSet<&str> =
        tree.modules.values().flat_map(|module| module.reached()).collect();
    let roots: Vec<String> =
        tree.modules.keys().filter(|path| !imported.contains(path.as_str())).cloned().collect();
    if roots.is_empty() { tree.modules.keys().cloned().collect() } else { roots }
}

// `/prefix/path`, however either is spelled: `@Controller("orders")` and
// `app.use("/api/", ..)` both lead with one slash and end with none.
fn join_route(prefix: &str, path: &str) -> String {
    let segments: Vec<&str> =
        prefix.split('/').chain(path.split('/')).filter(|segment| !segment.is_empty()).collect();
    format!("/{}", segments.join("/"))
}

// What tells a route from the others under its stem: the verb, then the
// path segments past the one that spells the stem, a parameter by its bare
// name — `GET /api/orders/:id` under `orders` is `get-id`; a route spelling
// no segment as its stem is relative to a mount the survey did not read, so
// every segment tells — `POST /:id/assign` under `tasks` is `post-id-assign`.
fn route_discriminator(verb: &str, route: &str, stem: &str) -> Option<String> {
    let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
    let past = segments
        .iter()
        .position(|segment| kebab(segment).is_some_and(|spelled| spelled == stem))
        .map_or(0, |i| i + 1);
    let parts: Vec<&str> = std::iter::once(verb)
        .chain(segments[past..].iter().map(|segment| segment.trim_start_matches(':')))
        .collect();
    kebab(&parts.join("-"))
}

// The first segment of a route that names a resource: not a version, an
// `api` namespace, a parameter, or a pattern.
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
    let pattern = segment.starts_with(':') || segment.contains(['*', '{', '(', '[']);
    !(PATH_NOISE.contains(&segment) || version || pattern)
}

// The first word of a literal that names something — a command, a queue, an
// event — as a stem; nothing for a pattern or a schedule.
fn literal_stem(literal: &str) -> Option<String> {
    let word = literal.split_whitespace().next()?;
    if !word.starts_with(|c: char| c.is_ascii_alphabetic())
        || word.contains(['<', '>', '[', ']', '*', '/', ':'])
    {
        return None;
    }
    kebab(word)
}

fn module_stem(module: &Module) -> String {
    kebab(module.stem()).unwrap_or_else(|| "module".to_owned())
}

// `text` as a kebab-case id segment: camel humps split, anything that is
// not a letter or digit a hyphen, runs collapsed; none when nothing is left.
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
