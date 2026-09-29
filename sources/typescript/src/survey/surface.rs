//! Decides the caller-facing surfaces of a tree from what its code registers.
//!
//! None of the rules names a framework. A surface is where control enters
//! from outside the process: the bootstrap the manifest names, a handler
//! handed to something a package provides, a method under a package's
//! decorator, or, in a library, what the entry module exports. Each carries
//! the stem its ids lead with, the id that tells it from the others under
//! that stem, and the modules it reaches, all derived from the code, so two
//! runs over one tree find the same surfaces and lead their ids the same way.

use std::collections::{BTreeMap, BTreeSet};

use super::parse::{
    Arg, Binding, BindingKind, Call, Export, ExportKind, Imported, Init, Lines, MemberKind, Module,
    Scope,
};
use super::resolve::{Manifest, Resolver, Target};
use super::{push_unique, unique};

// The bootstrap modules looked for when the manifest names none.
const BOOTSTRAPS: &[&str] =
    &["src/index.ts", "src/main.ts", "index.ts", "main.ts", "src/server.ts", "src/app.ts"];

// The index modules a library's exports are read from when the manifest
// names no entry.
const INDEXES: &[&str] =
    &["src/index.ts", "index.ts", "src/index.tsx", "index.tsx", "src/index.js", "index.js"];

const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options", "all"];

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
const TRACE: usize = 4;

/// The parsed tree with what locates its modules.
#[derive(Debug)]
pub struct Tree {
    pub modules: BTreeMap<String, Module>,
    pub resolver: Resolver,
    pub manifest: Manifest,
}

/// One place control enters the source from outside the process.
#[derive(Debug)]
pub struct Surface {
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
    pub fn bootstrap(&self) -> Option<String> {
        let conventional =
            BOOTSTRAPS.iter().filter_map(|candidate| self.resolver.first(&[candidate]));
        self.manifest.entries(&self.resolver).into_iter().chain(conventional).find(|entry| {
            self.modules.get(entry).is_some_and(|module| {
                module.calls.iter().any(|call| {
                    call.discarded
                        && call.depth == 0
                        && call.function.is_none()
                        && call.class.is_none()
                }) || module.imports.iter().any(|import| import.imported == Imported::Effect)
            })
        })
    }

    /// The package `local` is imported from in `module`, if it is an import
    /// of one.
    pub fn package(&self, module: &Module, local: &str) -> Option<String> {
        let import = module.import(local)?;
        match self.resolver.resolve(&module.path, &import.specifier)? {
            Target::Package(package) => Some(package),
            Target::Module(_) | Target::Data(_) | Target::Unresolved(_) => None,
        }
    }

    // The module `local` is imported from in `module`, if it is an import
    // of one, with the name imported.
    fn imported(&self, module: &Module, local: &str) -> Option<(&Module, String)> {
        let import = module.import(local)?;
        let Target::Module(path) = self.resolver.resolve(&module.path, &import.specifier)? else {
            return None;
        };
        let name = match &import.imported {
            Imported::Named(name) => name.clone(),
            Imported::Default => "default".to_owned(),
            Imported::Namespace | Imported::Effect => return None,
        };
        Some((self.modules.get(&path)?, name))
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
        let (target, imported) = self.imported(module, name)?;
        match &exported_binding(target, &imported)?.kind {
            BindingKind::Value { string, .. } => string.clone(),
            _ => None,
        }
    }

    /// Whether an argument is a handler: a function, an object or call
    /// carrying one, or a name bound to a function in the tree — declared
    /// here, imported from a module of the tree, or a member of a namespace
    /// import (`handlers.list`).
    pub fn handler(&self, module: &Module, arg: &Arg, frames: &[u32]) -> bool {
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
        let Some(Target::Module(path)) = self.resolver.resolve(&module.path, &import.specifier)
        else {
            return false;
        };
        let Some(target) = self.modules.get(&path) else { return false };
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
    pub fn receiver(&self, module: &Module, call: &Call) -> Option<Receiver> {
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
                    let package = self.package(module, path.first()?)?;
                    Some(Receiver {
                        package,
                        type_name: path.last().cloned(),
                        call: None,
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
                    call,
                    ..
                } => {
                    let package = self.package(module, path.first()?)?;
                    Some(Receiver {
                        package,
                        type_name: path.last().cloned(),
                        call: *call,
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
                        call: *call,
                    })
                }
                BindingKind::Param {
                    type_path: Some(path),
                } => {
                    let package = self.package(module, path.first()?)?;
                    Some(Receiver {
                        package,
                        type_name: path.last().cloned(),
                        call: None,
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
                        call: None,
                        ..traced
                    })
                }
                _ => None,
            };
        }
        if let Some(package) = self.package(module, name) {
            return Some(Receiver {
                package,
                type_name: None,
                call: None,
            });
        }
        let (target, imported) = self.imported(module, name)?;
        let binding = exported_binding(target, &imported)?;
        let traced = self.trace(target, &binding.name, &[], member, budget - 1)?;
        Some(Receiver { call: None, ..traced })
    }

    // The modules the names referenced within `lines` of `module` reach:
    // imports, and the types of the parameters and fields in scope.
    pub(super) fn reaches(
        &self, module: &Module, lines: Lines, frames: &[u32], class: Option<&str>,
    ) -> Vec<String> {
        let mut seeds = vec![module.path.clone()];
        let span = class
            .and_then(|class| module.classes.iter().find(|c| c.name == class))
            .map_or(lines, |c| c.lines);
        for name in module.referenced(span) {
            self.seed(&mut seeds, module, name);
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
                self.seed(&mut seeds, module, head);
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
                        self.seed(&mut seeds, module, head);
                    }
                }
            }
        }
        self.resolver.closure(&self.modules, &seeds, &[])
    }

    // Seeds the in-tree module `local` is imported from in `module`, once.
    fn seed(&self, seeds: &mut Vec<String>, module: &Module, local: &str) {
        if let Some(import) = module.import(local)
            && let Some(Target::Module(path)) =
                self.resolver.resolve(&module.path, &import.specifier)
        {
            push_unique(seeds, path);
        }
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
    pub package: String,
    /// The class it is typed or constructed as, when the code says.
    pub type_name: Option<String>,
    // the call in the module that initializes its binding
    call: Option<usize>,
}

/// The surfaces of `tree`: its bootstrap, then what registers a handler with
/// a package, what a package's decorator marks, and, in a tree with no
/// bootstrap, what its entry exports. Empty when the tree exposes none.
pub fn survey(tree: &Tree) -> Vec<Surface> {
    let bootstrap = tree.bootstrap();
    let scope: Vec<String> = bootstrap.as_ref().map_or_else(
        || tree.modules.keys().cloned().collect(),
        |entry| tree.resolver.closure(&tree.modules, std::slice::from_ref(entry), &[]),
    );
    let mounts = mounts(tree, &scope);

    let mut surfaces = Vec::new();
    for path in &scope {
        let Some(module) = tree.modules.get(path) else { continue };
        callbacks(tree, module, &mounts, &mut surfaces);
        decorated(tree, module, &mut surfaces);
    }

    match &bootstrap {
        Some(entry) => {
            if let Some(module) = tree.modules.get(entry) {
                surfaces.insert(0, start(tree, module, &surfaces));
            }
        }
        None => exports(tree, &mut surfaces),
    }

    identify(tree, &mut surfaces);
    surfaces
}

// The ids each surface's requirements lead with: a stem's one surface is
// its stem; surfaces sharing a stem are told apart by what discriminates
// each, else by their name, else by their entry's module, and two still
// alike by their entries' modules; a class carries an id per public method
// beside its own.
pub(super) fn identify(tree: &Tree, surfaces: &mut [Surface]) {
    let module_of = |surface: &Surface| tree.modules.get(&surface.entry).map(module_stem);
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
                .or_else(|| module_of(surface))
                .unwrap_or_else(|| "module".to_owned());
            format!("{}.{tell}", surface.stem)
        })
        .collect();
    let alike: Vec<bool> =
        owned.iter().map(|own| owned.iter().filter(|other| *other == own).count() > 1).collect();
    for (index, own) in owned.iter_mut().enumerate() {
        if alike[index]
            && let Some(stem) = module_of(&surfaces[index])
            && !own.ends_with(&format!(".{stem}"))
        {
            own.push('.');
            own.push_str(&stem);
        }
    }
    for (surface, own) in surfaces.iter_mut().zip(owned) {
        let mut ids = vec![own.clone()];
        ids.extend(surface.methods.iter().map(|method| format!("{own}.{method}")));
        surface.ids = ids;
    }
}

// The bootstrap surface: what the entry module does outside the handlers
// it registers, from its first line to its last, reaching the other
// surfaces' entries — what it mounts — and, through each, what that entry
// constructs, and no further.
pub(super) fn start(tree: &Tree, module: &Module, registered: &[Surface]) -> Surface {
    let lines = Lines {
        start: 1,
        end: u32::try_from(module.text.lines().count()).unwrap_or(u32::MAX).max(1),
    };
    let registrations: Vec<Lines> = registered
        .iter()
        .filter(|surface| surface.entry == module.path)
        .map(|surface| surface.lines)
        .collect();
    let mut seeds = vec![module.path.clone()];
    for name in module.referenced_outside(&registrations) {
        tree.seed(&mut seeds, module, name);
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
            tree.seed(&mut seeds, target, local);
        }
    }
    Surface {
        name: "start".to_owned(),
        entry: module.path.clone(),
        stem: "start".to_owned(),
        lines,
        detail: vec![
            "the process bootstrap: what runs before each handler is registered, what it awaits \
             before serving, and at shutdown — `stop` and what a signal handler calls, wherever \
             declared"
                .to_owned(),
        ],
        closure: tree.resolver.closure(&tree.modules, &seeds, &entries),
        discriminator: None,
        methods: Vec::new(),
        ids: Vec::new(),
    }
}

// Where each module's routes are mounted: `app.use("/api", ordersRouter(..))`
// gives `routes/orders.ts` the prefix `/api`.
pub(super) fn mounts(tree: &Tree, scope: &[String]) -> BTreeMap<String, String> {
    let mut mounts = BTreeMap::new();
    for path in scope {
        let Some(module) = tree.modules.get(path) else { continue };
        for call in &module.calls {
            if call.method() != "use" {
                continue;
            }
            let Some(prefix) = call.literal().filter(|literal| literal.starts_with('/')) else {
                continue;
            };
            for arg in call.args.iter().skip(1) {
                let Some(head) = arg.root.as_deref().and_then(|root| root.first()) else {
                    continue;
                };
                let Some((target, _)) = tree.imported(module, head) else { continue };
                mounts.entry(target.path.clone()).or_insert_with(|| prefix.to_owned());
            }
        }
    }
    mounts
}

// Callback surfaces: a handler handed, outside any other handler, to a
// receiver a package provides, by a call whose value is discarded, that
// constructs, or that a string literal leads.
fn callbacks(
    tree: &Tree, module: &Module, mounts: &BTreeMap<String, String>, surfaces: &mut Vec<Surface>,
) {
    // registration call index → surface index, for the hooks on its binding
    let mut registered: BTreeMap<usize, usize> = BTreeMap::new();

    for (index, call) in module.calls.iter().enumerate() {
        if call.depth > 0 || call.structural() {
            continue;
        }
        if !call.args.iter().any(|arg| tree.handler(module, arg, &call.frames)) {
            continue;
        }
        let lead = call.args.first().and_then(|arg| lead(tree, module, arg, &call.frames));
        let literal = lead.as_ref().and_then(Lead::literal);
        if !(call.discarded || call.is_new || literal.is_some()) {
            continue;
        }
        let Some(receiver) = tree.receiver(module, call) else { continue };

        // a hook on a binding a registration initialised folds into it
        if let Some(surface) = receiver.call.and_then(|init| registered.get(&init)) {
            let hook = format!("`{}` {}", spelled(call, lead.as_ref()), call.lines);
            surfaces[*surface].detail.push(format!("hook {hook}"));
            continue;
        }

        let (name, stem, routed) = name_callback(module, call, &receiver, lead.as_ref(), mounts);
        let handler = call.args.iter().find(|arg| arg.function).map(|arg| arg.lines);
        // a handler passed by name tells the surface apart; an anonymous
        // route's verb and path past the resource do
        let named = named_handler(tree, module, call);
        let mut detail = vec![match (&call.class, &call.function) {
            (Some(class), Some(function)) => {
                format!("registered {} in `{class}.{function}`", call.lines)
            }
            (None, Some(function)) => format!("registered {} in `{function}`", call.lines),
            _ => format!("registered {} at module level", call.lines),
        }];
        if let Some(handler) = handler {
            detail.push(format!("handler {handler}"));
        }
        detail.push(format!("through `{}`", receiver.package));

        registered.insert(index, surfaces.len());
        surfaces.push(Surface {
            name,
            entry: module.path.clone(),
            stem,
            lines: call.lines,
            detail,
            closure: tree.reaches(module, call.lines, &call.frames, call.class.as_deref()),
            discriminator: named.or(routed),
            methods: Vec::new(),
            ids: Vec::new(),
        });
    }
}

// The handler a registration is handed by name, kebab-cased.
fn named_handler(tree: &Tree, module: &Module, call: &Call) -> Option<String> {
    call.args
        .iter()
        .find(|arg| tree.handler(module, arg, &call.frames))
        .filter(|arg| !arg.function)
        .and_then(|arg| kebab(arg.root.as_ref()?.last()?))
}

// What leads a call's arguments and names what it registers.
#[derive(Clone)]
enum Lead {
    // a string literal, or a constant bound to one in the tree
    Literal(String),
    // an identifier bound to no literal the survey can read
    Ident(String),
}

impl Lead {
    fn literal(&self) -> Option<&str> {
        match self {
            Self::Literal(literal) => Some(literal),
            Self::Ident(_) => None,
        }
    }

    fn spelled(&self) -> String {
        match self {
            Self::Literal(literal) => format!("\"{literal}\""),
            Self::Ident(name) => name.clone(),
        }
    }
}

fn lead(tree: &Tree, module: &Module, arg: &Arg, frames: &[u32]) -> Option<Lead> {
    if let Some(literal) = tree.constant(module, arg, frames) {
        return Some(Lead::Literal(literal));
    }
    match arg.root.as_deref() {
        Some([name]) if !arg.called && !arg.function => Some(Lead::Ident(name.clone())),
        _ => None,
    }
}

// `<Receiver>.<method>("<literal>")` — or the route `GET /path` a verb
// registers — and the stem: the path's first plain segment, the literal's
// first word, the receiver's type, else the module's name; and what tells
// the surface from the others under its stem: a route's verb and path past
// the resource, else the method that registers it.
fn name_callback(
    module: &Module, call: &Call, receiver: &Receiver, lead: Option<&Lead>,
    mounts: &BTreeMap<String, String>,
) -> (String, String, Option<String>) {
    let (method, lead) = registered(call, lead);
    let literal = lead.as_ref().and_then(Lead::literal);
    let typed = || receiver.type_name.as_deref().and_then(kebab);

    if let Some(route) = routed(module, method, literal, mounts) {
        let name = format!("{} {route}", method.to_ascii_uppercase());
        let stem = route_stem(&route).or_else(typed).unwrap_or_else(|| module_stem(module));
        let discriminator = route_discriminator(method, &route, &stem);
        return (name, stem, discriminator);
    }

    let receiver_name =
        receiver.type_name.clone().unwrap_or_else(|| call.callee.receiver().to_owned());
    let argument = lead.as_ref().map(Lead::spelled).unwrap_or_default();
    let name = if call.is_new {
        format!("{}({argument})", call.method())
    } else if lead.is_some() {
        format!("{receiver_name}.{method}({argument})")
    } else {
        format!("{receiver_name}.{method}")
    };
    let stem = literal.and_then(literal_stem).or_else(typed).unwrap_or_else(|| module_stem(module));
    (name, stem, kebab(method))
}

// The method a registration is spelled by and what leads it: a chain names
// itself at its first call with a literal (`command("x")`), else the call's
// own method and lead.
fn registered<'c>(call: &'c Call, lead: Option<&Lead>) -> (&'c str, Option<Lead>) {
    let chained = call.callee.links.iter().find_map(|link| {
        let literal = link.call.as_ref()?.literal.clone()?;
        Some((link.name.as_str(), Lead::Literal(literal)))
    });
    chained.map_or_else(|| (call.method(), lead.cloned()), |(method, lead)| (method, Some(lead)))
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

// What tells a registration from the others under its stem: a route's verb
// and path past the resource, else the method that registers it.
#[cfg(feature = "model-survey")]
fn tell_callback(
    module: &Module, call: &Call, lead: Option<&Lead>, stem: &str,
    mounts: &BTreeMap<String, String>,
) -> Option<String> {
    let (method, lead) = registered(call, lead);
    routed(module, method, lead.as_ref().and_then(Lead::literal), mounts)
        .map_or_else(|| kebab(method), |route| route_discriminator(method, &route, stem))
}

// What tells a surface the survey anchored at `lines` of `module` from the
// others under `stem`, derived as the parser derives its own: from the
// registration at those lines — the handler handed by name, else the route's
// verb and path past the resource, else the registering method — from the
// decorated method's name, or from the export's name joined with the
// segments a file-routed module's path spells past its stem; failing every
// one, from the survey's own `name` less the stem and what only namespaces.
#[cfg(feature = "model-survey")]
pub(super) fn discriminate(
    tree: &Tree, module: &Module, lines: Lines, name: &str, stem: &str,
    mounts: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(call) = registration_at(tree, module, lines) {
        let led = call.args.first().and_then(|arg| lead(tree, module, arg, &call.frames));
        return named_handler(tree, module, call)
            .or_else(|| tell_callback(module, call, led.as_ref(), stem, mounts));
    }
    if let Some(member) = module
        .decorated
        .iter()
        .filter(|decorated| overlaps(decorated.lines, lines))
        .find_map(|decorated| decorated.member.as_deref())
    {
        return kebab(member);
    }
    if let Some(export) = export_at(module, lines) {
        let display =
            if export.name == "default" { module_stem(module) } else { export.name.clone() };
        let mut parts = vec![display];
        parts.extend(file_routed(&module.path, stem));
        return kebab(&parts.join("-"));
    }
    normalised(name, stem)
}

// Whether a call has the shape of a registration as `callbacks` reads one,
// at any depth and whatever its receiver: it hands a function to something
// and is discarded, constructs, or is led by a literal. A wrapper handed a
// function for its value — `export default fp(async (app) => ..)` — has
// not: it defines a plugin, it registers nothing.
#[cfg(feature = "model-survey")]
pub(super) fn registers(tree: &Tree, module: &Module, call: &Call) -> bool {
    !call.structural()
        && call.args.iter().any(|arg| tree.handler(module, arg, &call.frames))
        && (call.discarded
            || call.is_new
            || call
                .args
                .first()
                .and_then(|arg| lead(tree, module, arg, &call.frames))
                .is_some_and(|led| led.literal().is_some()))
}

// The registration at `lines`: the first call of that shape starting within
// them — since the survey has decided the lines register a surface — else
// the one enclosing their first line, for an anchor within a handler.
#[cfg(feature = "model-survey")]
fn registration_at<'m>(tree: &Tree, module: &'m Module, lines: Lines) -> Option<&'m Call> {
    let registers = |call: &&Call| registers(tree, module, call);
    module.calls.iter().filter(registers).find(|call| lines.holds(call.lines.start)).or_else(|| {
        module.calls.iter().filter(registers).find(|call| call.lines.holds(lines.start))
    })
}

// The export declared at `lines`: a function or class whose declaration
// starts within them, else a value's, else the one enclosing their first
// line.
#[cfg(feature = "model-survey")]
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
#[cfg(feature = "model-survey")]
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
#[cfg(feature = "model-survey")]
fn normalised(name: &str, stem: &str) -> Option<String> {
    let spelled = kebab(name)?;
    let kept: Vec<&str> = spelled
        .split('-')
        .filter(|word| !stem.split('-').any(|own| own == *word) && names_resource(word))
        .collect();
    kebab(&kept.join("-"))
}

#[cfg(feature = "model-survey")]
const fn overlaps(a: Lines, b: Lines) -> bool {
    a.start <= b.end && b.start <= a.end
}

// The call as the code spells it, for a hook's note.
fn spelled(call: &Call, lead: Option<&Lead>) -> String {
    let receiver = call.callee.receiver();
    let argument = lead.map(Lead::spelled).unwrap_or_default();
    if lead.is_some() {
        format!("{receiver}.{}({argument})", call.method())
    } else {
        format!("{receiver}.{}", call.method())
    }
}

// Decorated surfaces: a method under a decorator a package provides, named
// by the decorator and its literal, or as the route a verb decorator maps.
fn decorated(tree: &Tree, module: &Module, surfaces: &mut Vec<Surface>) {
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for decorated in &module.decorated {
        let Some(member) = &decorated.member else { continue };
        let Some(decorator) = decorated.name.last() else { continue };
        if shapes(decorator) {
            continue;
        }
        let Some(package) = decorated.name.first().and_then(|head| tree.package(module, head))
        else {
            continue;
        };
        if !seen.insert((&decorated.class, member)) {
            continue;
        }

        // the class decorator that registers the class carries the prefix;
        // one that documents it (`@ApiTags("Users")`) does not
        let prefix = module
            .decorated
            .iter()
            .filter(|d| d.class == decorated.class && d.member.is_none() && d.literal.is_some())
            .find(|d| d.name.last().is_some_and(|name| !shapes(name)))
            .and_then(|d| d.literal.as_deref())
            .unwrap_or("");
        let verb = decorator.to_ascii_lowercase();
        let (name, stem) = if VERBS.contains(&verb.as_str()) {
            let route = join_route(prefix, decorated.literal.as_deref().unwrap_or(""));
            (format!("{} {route}", verb.to_ascii_uppercase()), route_stem(&route))
        } else {
            let argument =
                decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
            let name = format!("@{decorator}{argument} {}.{member}", decorated.class);
            (name, decorated.literal.as_deref().and_then(literal_stem))
        };
        let stem = stem.or_else(|| kebab(&decorated.class)).unwrap_or_else(|| module_stem(module));

        surfaces.push(Surface {
            name,
            entry: module.path.clone(),
            stem,
            lines: decorated.lines,
            detail: vec![
                format!("method `{}.{member}` {}", decorated.class, decorated.lines),
                format!("through `{package}`"),
            ],
            closure: tree.reaches(module, decorated.lines, &[], Some(&decorated.class)),
            discriminator: kebab(member),
            methods: Vec::new(),
            ids: Vec::new(),
        });
    }
}

// Export surfaces, for a tree with no bootstrap: each function and class the
// entry modules export, one re-export hop deep, less error types and the
// functions that host registrations.
fn exports(tree: &Tree, surfaces: &mut Vec<Surface>) {
    for (module, export) in exported(tree) {
        let path = &module.path;
        let local = export.local.as_deref().unwrap_or(&export.name);
        let display =
            if export.name == "default" { module_stem(module) } else { export.name.clone() };
        let (what, methods) = match &export.kind {
            ExportKind::Function => ("exported function", Vec::new()),
            ExportKind::Value
                if module.binding(local, &[]).is_some_and(|binding| {
                    matches!(
                        binding.kind,
                        BindingKind::Value {
                            init: Init::Function,
                            ..
                        }
                    )
                }) =>
            {
                ("exported function", Vec::new())
            }
            ExportKind::Class { extends } => {
                if extends.as_deref() == Some("Error")
                    || display.ends_with("Error")
                    || display.ends_with("Exception")
                {
                    continue;
                }
                ("exported class", methods(module, local))
            }
            _ => continue,
        };
        let declared = module.binding(local, &[]).map_or(export.lines, |binding| binding.lines);
        // a function that registers surfaces hosts them; it is not one
        if surfaces.iter().any(|s| s.entry == *path && declared.contains(s.lines)) {
            continue;
        }
        let mut detail = vec![format!("{what} {declared}")];
        if !methods.is_empty() {
            let noted: Vec<String> =
                methods.iter().map(|(name, lines)| format!("`{name}` {lines}")).collect();
            detail.push(format!("methods {}", noted.join(", ")));
        }
        let stem = kebab(&display).unwrap_or_else(|| module_stem(module));
        surfaces.push(Surface {
            name: display,
            entry: path.clone(),
            stem,
            lines: declared,
            detail,
            closure: tree.reaches(module, declared, &[], None),
            discriminator: None,
            methods: methods.iter().filter_map(|(name, _)| kebab(name)).collect(),
            ids: Vec::new(),
        });
    }
}

// What the entry modules export, each with the module that declares it: the
// manifest's entry or the conventional index, else the modules nothing
// imports; a re-export is followed one hop to the module that declares it.
fn exported(tree: &Tree) -> Vec<(&Module, &Export)> {
    let entries: Vec<String> = tree
        .manifest
        .entry(&tree.resolver)
        .into_iter()
        .chain(tree.resolver.first(INDEXES))
        .collect();
    let entries = if entries.is_empty() { roots(tree) } else { entries };

    let mut listed = Vec::new();
    for entry in &entries {
        let Some(module) = tree.modules.get(entry) else { continue };
        listed.extend(module.exports.iter().map(|export| (module, export)));
        for reexport in module.reexports.iter().filter(|reexport| !reexport.type_only) {
            let Some(Target::Module(path)) = tree.resolver.resolve(entry, &reexport.specifier)
            else {
                continue;
            };
            let Some(target) = tree.modules.get(&path) else { continue };
            match &reexport.names {
                Some(names) => listed.extend(names.iter().filter_map(|(imported, _)| {
                    let export = target.exports.iter().find(|export| {
                        export.name == *imported || export.local.as_deref() == Some(imported)
                    })?;
                    Some((target, export))
                })),
                None => listed.extend(target.exports.iter().map(|export| (target, export))),
            }
        }
    }
    listed
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

// The modules no other module imports.
pub(super) fn roots(tree: &Tree) -> Vec<String> {
    let mut imported: Vec<String> = Vec::new();
    for module in tree.modules.values() {
        for specifier in module.specifiers() {
            if let Some(Target::Module(path)) = tree.resolver.resolve(&module.path, specifier) {
                imported.push(path);
            }
        }
    }
    let roots: Vec<String> =
        tree.modules.keys().filter(|path| !imported.contains(path)).cloned().collect();
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
