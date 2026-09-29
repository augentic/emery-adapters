//! Decides the caller-facing surfaces of a tree from what its code registers.
//!
//! None of the rules names a framework. A surface is where control enters
//! from outside the process: the bootstrap the manifest names, a handler
//! handed to something a package provides, a method under a package's
//! decorator, or, in a library, what the entry module exports. Each carries
//! the stem its ids lead with, the id that tells it from the others under
//! that stem, and the modules it reaches, all derived from the code, so two
//! runs over one tree find the same surfaces and lead their ids the same way.

use std::collections::BTreeMap;

use crate::parse::{Arg, BindingKind, Call, ExportKind, Init, Lines, Module};
use crate::resolve::{Manifest, Resolver, Target};

// The bootstrap modules looked for when the manifest names none.
const BOOTSTRAPS: &[&str] =
    &["src/index.ts", "src/main.ts", "index.ts", "main.ts", "src/server.ts", "src/app.ts"];

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
];

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
    /// The bootstrap module: the entry the manifest names, else the first of
    /// the conventional entries the tree holds, when loading it runs
    /// something — a call at module level whose value is discarded, or an
    /// import for its effect. An entry that only declares and exports is a
    /// library's, and the tree has no bootstrap.
    pub fn bootstrap(&self) -> Option<String> {
        let entry =
            self.manifest.entry(&self.resolver).or_else(|| self.resolver.first(BOOTSTRAPS))?;
        let module = self.modules.get(&entry)?;
        let runs = module.calls.iter().any(|call| {
            call.discarded && call.depth == 0 && call.function.is_none() && call.class.is_none()
        }) || module
            .imports
            .iter()
            .any(|import| matches!(import.imported, crate::parse::Imported::Effect));
        runs.then_some(entry)
    }

    /// The package `local` is imported from in `module`, if it is an import
    /// of one.
    pub fn package(&self, module: &Module, local: &str) -> Option<String> {
        let import = module.import(local)?;
        match self.resolver.resolve(&module.path, &import.specifier)? {
            Target::Package(package) => Some(package),
            Target::Module(_) => None,
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
            crate::parse::Imported::Named(name) => name.clone(),
            crate::parse::Imported::Default => "default".to_owned(),
            crate::parse::Imported::Namespace | crate::parse::Imported::Effect => return None,
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
            (crate::parse::Imported::Named(exported), []) => exported.as_str(),
            (crate::parse::Imported::Default, []) => "default",
            (crate::parse::Imported::Namespace, [member]) => member.as_str(),
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
    /// it to the package it comes from; `None` for a receiver of the tree's
    /// own or of the runtime's.
    pub fn receiver(&self, module: &Module, call: &Call) -> Option<Receiver> {
        let head = call.callee.head.as_str();
        if head == "this" {
            let class = call.class.as_deref()?;
            if call.callee.links.len() < 2 {
                return None;
            }
            let field = module.field(class, &call.callee.links[0].name)?;
            return match &field.kind {
                BindingKind::Field {
                    type_path: Some(path),
                    ..
                } => {
                    let package = self.package(module, &path[0])?;
                    Some(Receiver {
                        package,
                        type_name: path.last().cloned(),
                        call: None,
                    })
                }
                BindingKind::Field { root: Some(root), .. } => {
                    self.trace(module, &root[0], &[], TRACE)
                }
                _ => None,
            };
        }
        self.trace(module, head, &call.frames, TRACE)
    }

    fn trace(
        &self, module: &Module, name: &str, frames: &[u32], budget: usize,
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
                    let package = self.package(module, &path[0])?;
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
                    let traced = self.trace(module, &root[0], frames, budget - 1)?;
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
                    let package = self.package(module, &path[0])?;
                    Some(Receiver {
                        package,
                        type_name: path.last().cloned(),
                        call: None,
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
        let traced = self.trace(target, &binding.name, &[], budget - 1)?;
        Some(Receiver { call: None, ..traced })
    }

    // The modules the names referenced within `lines` of `module` reach:
    // imports, and the types of the parameters and fields in scope.
    fn reaches(
        &self, module: &Module, lines: Lines, frames: &[u32], class: Option<&str>,
    ) -> Vec<String> {
        let mut seeds = vec![module.path.clone()];
        let push = |seeds: &mut Vec<String>, local: &str| {
            if let Some(import) = module.import(local)
                && let Some(Target::Module(path)) =
                    self.resolver.resolve(&module.path, &import.specifier)
                && !seeds.contains(&path)
            {
                seeds.push(path);
            }
        };
        let span = class
            .and_then(|class| module.classes.iter().find(|c| c.name == class))
            .map_or(lines, |c| c.lines);
        for name in module.referenced(span) {
            push(&mut seeds, name);
            if let Some(binding) = module.binding(name, frames)
                && let BindingKind::Param {
                    type_path: Some(path),
                }
                | BindingKind::Value {
                    type_path: Some(path),
                    ..
                } = &binding.kind
            {
                push(&mut seeds, &path[0]);
            }
        }
        if let Some(class) = class {
            for binding in module
                .bindings
                .iter()
                .filter(|b| b.scope == crate::parse::Scope::Class(class.to_owned()))
            {
                if let BindingKind::Field { type_path, root, .. } = &binding.kind {
                    if let Some(path) = type_path {
                        push(&mut seeds, &path[0]);
                    }
                    if let Some(root) = root {
                        push(&mut seeds, &root[0]);
                    }
                }
            }
        }
        self.resolver.closure(&self.modules, &seeds, &[])
    }
}

// The module-level binding an export of `module` names.
fn exported_binding<'m>(module: &'m Module, name: &str) -> Option<&'m crate::parse::Binding> {
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
fn identify(tree: &Tree, surfaces: &mut [Surface]) {
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
                .or_else(|| {
                    Some(kebab(&surface.name))
                        .filter(|tell| !tell.is_empty() && *tell != surface.stem)
                })
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
fn start(tree: &Tree, module: &Module, registered: &[Surface]) -> Surface {
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
        if let Some(import) = module.import(name)
            && let Some(Target::Module(path)) =
                tree.resolver.resolve(&module.path, &import.specifier)
            && !seeds.contains(&path)
        {
            seeds.push(path);
        }
    }
    let mut entries: Vec<String> = registered
        .iter()
        .map(|surface| surface.entry.clone())
        .filter(|entry| *entry != module.path)
        .collect();
    entries.dedup();

    // an entry is reached and not followed, but what loading it constructs
    // — its class fields' types and initializers, its module-level bindings
    // — runs before any handler and is the bootstrap's to reach
    for entry in &entries {
        let Some(target) = tree.modules.get(entry) else { continue };
        for local in target.constructed() {
            if let Some(import) = target.import(local)
                && let Some(Target::Module(path)) =
                    tree.resolver.resolve(&target.path, &import.specifier)
                && !seeds.contains(&path)
            {
                seeds.push(path);
            }
        }
    }
    Surface {
        name: "start".to_owned(),
        entry: module.path.clone(),
        stem: "start".to_owned(),
        lines,
        detail: vec![
            "the process bootstrap: what runs before each handler is registered, and at shutdown"
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
fn mounts(tree: &Tree, scope: &[String]) -> BTreeMap<String, String> {
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
                let Some(root) = arg.root.as_deref() else { continue };
                let Some((target, _)) = tree.imported(module, &root[0]) else { continue };
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
        let literal = match &lead {
            Some(Lead::Literal(literal)) => Some(literal.as_str()),
            _ => None,
        };
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
        let named = call
            .args
            .iter()
            .find(|arg| tree.handler(module, arg, &call.frames))
            .filter(|arg| !arg.function)
            .and_then(|arg| arg.root.as_ref()?.last())
            .map(|name| kebab(name))
            .filter(|name| !name.is_empty());
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

// What leads a call's arguments and names what it registers.
enum Lead {
    // a string literal, or a constant bound to one in the tree
    Literal(String),
    // an identifier bound to no literal the survey can read
    Ident(String),
}

impl Lead {
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
    // a chain names itself at its first call with a literal: `command("x")`
    let chained = call.callee.links.iter().find_map(|link| {
        let literal = link.call.as_ref()?.literal.clone()?;
        Some((link.name.as_str(), Lead::Literal(literal)))
    });
    let (method, lead) = match &chained {
        Some((method, lead)) => (*method, Some(lead)),
        None => (call.method(), lead),
    };
    let literal = match lead {
        Some(Lead::Literal(literal)) => Some(literal.as_str()),
        _ => None,
    };
    let typed = || receiver.type_name.as_deref().map(kebab).filter(|s| !s.is_empty());

    if let Some(path) = literal.filter(|l| l.starts_with('/'))
        && VERBS.contains(&method)
    {
        let prefix = mounts.get(&module.path).map_or("", String::as_str);
        let route = join_route(prefix, path);
        let name = format!("{} {route}", method.to_ascii_uppercase());
        let stem = route_stem(&route).or_else(typed).unwrap_or_else(|| module_stem(module));
        return (name, stem, route_discriminator(method, &route));
    }

    let receiver_name = receiver.type_name.clone().unwrap_or_else(|| {
        if call.callee.head == "this" {
            call.callee.links.first().map_or_else(|| "this".to_owned(), |link| link.name.clone())
        } else {
            call.callee.head.clone()
        }
    });
    let argument = lead.map(Lead::spelled).unwrap_or_default();
    let name = if call.is_new {
        format!("{}({argument})", call.method())
    } else if lead.is_some() {
        format!("{receiver_name}.{method}({argument})")
    } else {
        format!("{receiver_name}.{method}")
    };
    let stem = literal.and_then(literal_stem).or_else(typed).unwrap_or_else(|| module_stem(module));
    (name, stem, Some(kebab(method)).filter(|tell| !tell.is_empty()))
}

// The call as the code spells it, for a hook's note.
fn spelled(call: &Call, lead: Option<&Lead>) -> String {
    let receiver = if call.callee.head == "this" {
        call.callee.links.first().map_or("this", |link| link.name.as_str())
    } else {
        call.callee.head.as_str()
    };
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
    let mut seen: Vec<(String, String)> = Vec::new();
    for decorated in &module.decorated {
        let Some(member) = &decorated.member else { continue };
        let Some(decorator) = decorated.name.last() else { continue };
        if DECORATOR_NOISE.contains(&decorator.as_str()) || decorator.starts_with("Api") {
            continue;
        }
        let Some(package) = tree.package(module, &decorated.name[0]) else { continue };
        let key = (decorated.class.clone(), member.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);

        let prefix = module
            .decorated
            .iter()
            .find(|d| d.class == decorated.class && d.member.is_none() && d.literal.is_some())
            .and_then(|d| d.literal.as_deref())
            .unwrap_or("");
        let verb = decorator.to_ascii_lowercase();
        let (name, stem) = if VERBS.contains(&verb.as_str()) {
            let route = join_route(prefix, decorated.literal.as_deref().unwrap_or(""));
            let stem = route_stem(&route).unwrap_or_else(|| kebab(&decorated.class));
            (format!("{} {route}", verb.to_ascii_uppercase()), stem)
        } else {
            let argument =
                decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
            let name = format!("@{decorator}{argument} {}.{member}", decorated.class);
            let stem = decorated
                .literal
                .as_deref()
                .and_then(literal_stem)
                .unwrap_or_else(|| kebab(&decorated.class));
            (name, stem)
        };
        let stem = if stem.is_empty() { module_stem(module) } else { stem };

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
            discriminator: Some(kebab(member)).filter(|tell| !tell.is_empty()),
            methods: Vec::new(),
            ids: Vec::new(),
        });
    }
}

// Export surfaces, for a tree with no bootstrap: each function and class the
// entry modules export, one re-export hop deep, less error types and the
// functions that host registrations.
fn exports(tree: &Tree, surfaces: &mut Vec<Surface>) {
    for (path, name) in exported(tree) {
        let Some(module) = tree.modules.get(&path) else { continue };
        let Some(export) =
            module.exports.iter().find(|e| e.name == name || e.local.as_deref() == Some(&name))
        else {
            continue;
        };
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
        if surfaces.iter().any(|s| s.entry == path && declared.contains(s.lines)) {
            continue;
        }
        let mut detail = vec![format!("{what} {declared}")];
        if !methods.is_empty() {
            let noted: Vec<String> =
                methods.iter().map(|(name, lines)| format!("`{name}` {lines}")).collect();
            detail.push(format!("methods {}", noted.join(", ")));
        }
        let stem = kebab(&display);
        surfaces.push(Surface {
            name: display,
            entry: path.clone(),
            stem: if stem.is_empty() { module_stem(module) } else { stem },
            lines: declared,
            detail,
            closure: tree.reaches(module, declared, &[], None),
            discriminator: None,
            methods: methods
                .iter()
                .map(|(name, _)| kebab(name))
                .filter(|method| !method.is_empty())
                .collect(),
            ids: Vec::new(),
        });
    }
}

// What the entry modules export, as `(module, name)`: the manifest's entry
// or the conventional index, else the modules nothing imports; a re-export
// is followed one hop to the module that declares it.
fn exported(tree: &Tree) -> Vec<(String, String)> {
    let entries: Vec<String> = tree
        .manifest
        .entry(&tree.resolver)
        .into_iter()
        .chain(tree.resolver.first(&[
            "src/index.ts",
            "index.ts",
            "src/index.tsx",
            "index.tsx",
            "src/index.js",
            "index.js",
        ]))
        .collect();
    let entries = if entries.is_empty() { roots(tree) } else { entries };

    let mut listed: Vec<(String, String)> = Vec::new();
    for entry in &entries {
        let Some(module) = tree.modules.get(entry) else { continue };
        for export in &module.exports {
            let name = export.local.clone().unwrap_or_else(|| export.name.clone());
            listed.push((entry.clone(), name));
        }
        for reexport in module.reexports.iter().filter(|reexport| !reexport.type_only) {
            let Some(Target::Module(path)) = tree.resolver.resolve(entry, &reexport.specifier)
            else {
                continue;
            };
            let Some(target) = tree.modules.get(&path) else { continue };
            match &reexport.names {
                Some(names) => {
                    for (imported, _) in names {
                        listed.push((path.clone(), imported.clone()));
                    }
                }
                None => {
                    for export in &target.exports {
                        let name = export.local.clone().unwrap_or_else(|| export.name.clone());
                        listed.push((path.clone(), name));
                    }
                }
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
                .filter(|m| {
                    !m.private
                        && matches!(
                            m.kind,
                            crate::parse::MemberKind::Method | crate::parse::MemberKind::Getter
                        )
                })
                .map(|m| (m.name.as_str(), m.lines))
                .collect()
        })
        .unwrap_or_default()
}

// The modules no other module imports.
fn roots(tree: &Tree) -> Vec<String> {
    let mut imported: Vec<String> = Vec::new();
    for module in tree.modules.values() {
        for specifier in module
            .imports
            .iter()
            .map(|i| i.specifier.as_str())
            .chain(module.reexports.iter().map(|r| r.specifier.as_str()))
        {
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

// What tells a route from the others under its resource: the verb, then the
// path segments past the resource, a parameter by its bare name — `GET
// /api/orders/:id` is `get-id`.
fn route_discriminator(verb: &str, route: &str) -> Option<String> {
    let segments: Vec<&str> = route.split('/').filter(|segment| !segment.is_empty()).collect();
    let past = segments.iter().position(|segment| names_resource(segment)).map_or(0, |i| i + 1);
    let parts: Vec<&str> = std::iter::once(verb)
        .chain(segments[past..].iter().map(|segment| segment.trim_start_matches(':')))
        .collect();
    Some(kebab(&parts.join("-"))).filter(|tell| !tell.is_empty())
}

// The first segment of a route that names a resource: not a version, an
// `api` namespace, a parameter, or a pattern.
fn route_stem(route: &str) -> Option<String> {
    route
        .split('/')
        .filter(|segment| !segment.is_empty())
        .find(|segment| names_resource(segment))
        .map(kebab)
        .filter(|stem| !stem.is_empty())
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
    let stem = kebab(word);
    (!stem.is_empty()).then_some(stem)
}

fn module_stem(module: &Module) -> String {
    let stem = kebab(module.stem());
    if stem.is_empty() { "module".to_owned() } else { stem }
}

/// `text` as a kebab-case id segment: camel humps split, anything that is
/// not a letter or digit a hyphen, runs collapsed.
pub fn kebab(text: &str) -> String {
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
    if emery_sdk::is_kebab(trimmed) { trimmed.to_owned() } else { String::new() }
}
