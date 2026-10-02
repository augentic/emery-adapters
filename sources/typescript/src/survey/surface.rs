//! Derives what the code says of a tree's surfaces from the accepted anchors.
//!
//! A surface is where control enters from outside the process: the
//! bootstrap the manifest names, a handler handed to something a package
//! provides, a method under a package's decorator, or, in a library, what
//! the entry module exports. The survey names each at the lines that
//! register or declare it. From those lines the code reads the stem a route
//! or literal spells, the id that tells the surface from the others under
//! its stem, a class's methods, and the modules the surface reaches, so two
//! runs that accept the same anchors lead their ids the same way and reach
//! the same modules.
//!
//! The rules read shapes, never a framework's name: a call handed a
//! function, a decorator with a literal, an export. The one exception
//! spells names. A decorator among those the data model lists as shaping a
//! handler (`Decorated::registering`) does not register it, which keeps a
//! verb decorator beneath one of them the surface.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::parse::{
    Arg, BindingKind, Call, Decorated, Export, ExportKind, Imported, Init, Lines, MemberKind,
    Module, Scope,
};
use super::resolve::{Manifest, Resolver, Target, normalize};
use super::{push_unique, unique};

// Looked for in this order when the manifest names no entry.
const BOOTSTRAPS: &[&str] =
    &["src/index.ts", "src/main.ts", "index.ts", "main.ts", "src/server.ts", "src/app.ts"];

const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options", "all"];

// Methods that mount what they are handed: `use` a router under a path, any
// of them a loader over a directory. A route registered in a mounted module
// is read under its mount.
const MOUNTING: &[&str] = &["use", "register", "mount", "plugin"];

// Path segments that version or namespace an API rather than name a surface.
const PATH_NOISE: &[&str] = &["api", "rest", "internal"];

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
    // The public methods of an exported class, kebab-cased.
    pub methods: Vec<String>,
    // The stem alone for a stem's one surface, `<stem>.<discriminator>`
    // otherwise, and one more per method of a class.
    pub ids: Vec<String>,
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
        let resolver = Resolver::new(root, modules.keys().cloned(), data);
        for module in modules.values_mut() {
            resolver.settle(module);
        }
        Self {
            modules,
            resolver,
            manifest: Manifest::read(root),
        }
    }

    // The first entry that runs something, probed in this order:
    // - the entries the manifest names
    // - the conventional entries the tree holds
    // An entry runs by a module-level call whose value is discarded, or an
    // import for its effect. One that only declares and exports is a
    // library's: a `main` naming the barrel while `start` runs the server.
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

    // The exporting module and the name exported.
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

    // `seeds` first in their order, then breadth-first, once each. A module
    // in `stop` is reached and not followed: another surface's entry is what
    // the bootstrap mounts, not what it does.
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
    // imports.
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

    // A function, an object or call carrying one, or a name bound to a
    // function in the tree: declared here, imported from a module of the
    // tree, or a member of a namespace import (`handlers.list`).
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

    // Traced through the bindings that construct it to the package it comes
    // from, a class of the tree's that extends a package's class among them
    // for a member it inherits. `None` for a receiver of the tree's own or
    // the runtime's.
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
        let binding = target.exported_binding(&imported)?;
        self.trace(target, &binding.name, &[], member, budget - 1)
    }

    // Through the imports referenced within `lines`, and the types of the
    // parameters and fields in scope.
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

    // A string literal, or a constant bound to one in the tree.
    pub(super) fn led(&self, module: &Module, call: &Call) -> Option<String> {
        call.args.first().and_then(|arg| self.constant(module, arg, &call.frames))
    }

    // Kebab-cased; none for an inline function.
    fn named_handler(&self, module: &Module, call: &Call) -> Option<String> {
        call.args
            .iter()
            .find(|arg| self.handler(module, arg, &call.frames))
            .filter(|arg| !arg.function)
            .and_then(|arg| kebab(arg.root.as_ref()?.last()?))
    }

    // A handler among the arguments, and not a structural call (`.then`,
    // `.map`) that takes one.
    fn hands(&self, module: &Module, call: &Call) -> bool {
        !call.structural() && call.args.iter().any(|arg| self.handler(module, arg, &call.frames))
    }

    // A call outside any handler that hands a function to something a package
    // provides: where a framework, a queue, a scheduler, or a CLI is told
    // what to run.
    pub(super) fn handed(&self, module: &Module, call: &Call) -> Option<Receiver> {
        (call.depth == 0 && self.hands(module, call)).then(|| self.receiver(module, call))?
    }

    // At any depth and whatever the receiver: hands a function and is
    // discarded, constructs, or is led by a literal. A wrapper handed a
    // function for its value (`export default fp(async (app) => ..)`)
    // defines a plugin and registers nothing.
    pub(super) fn registers(&self, module: &Module, call: &Call) -> bool {
        self.hands(module, call)
            && (call.discarded() || call.is_new || self.led(module, call).is_some())
    }

    // The first registration starting within `lines`, else the one enclosing
    // their first line, for an anchor within a handler.
    fn registration_at<'m>(&self, module: &'m Module, lines: Lines) -> Option<&'m Call> {
        let registers = |call: &&Call| self.registers(module, call);
        module.calls.iter().filter(registers).find(|call| lines.holds(call.lines.start)).or_else(
            || module.calls.iter().filter(registers).find(|call| call.lines.holds(lines.start)),
        )
    }

    // The modules no other imports; every module where each is imported.
    pub(super) fn roots(&self) -> Vec<String> {
        let imported: BTreeSet<&str> =
            self.modules.values().flat_map(|module| module.reached()).collect();
        let roots: Vec<String> =
            self.modules.keys().filter(|path| !imported.contains(path.as_str())).cloned().collect();
        if roots.is_empty() { self.modules.keys().cloned().collect() } else { roots }
    }

    // The route prefix each module's routes sit under:
    // - `app.use("/api", ordersRouter(..))` gives `routes/orders.ts` `/api`
    // - a loader registered over a directory of the tree
    //   (`app.register(autoload, { dir: join(__dirname, "routes") })`) gives
    //   each module beneath it the path of its directory from there, under
    //   the loader's `prefix` option where it names one, so
    //   `routes/api/auth/index.ts` is `/api/auth` and `routes/home.ts` sits
    //   at the root
    // The first mount read of a module stands.
    pub(super) fn mounts(&self) -> BTreeMap<String, String> {
        let mut mounts = BTreeMap::new();
        for module in self.modules.values() {
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
                        let Some((target, _)) = self.exporter(module, head) else { continue };
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
                    for path in self.modules.keys() {
                        let Some(beneath) = path.strip_prefix(&format!("{root}/")) else {
                            continue;
                        };
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
    // By its specifier as imported.
    pub package: String,
    // The class it is typed or constructed as, where the code says.
    pub type_name: Option<String>,
}

// `local/files.controller.ts` and `s3/files.controller.ts` under `files` are
// told apart by `local` and `s3`.
fn tells_apart(module: &Module, stem: &str, own: &str) -> Option<String> {
    let dir = module.path.rsplit_once('/').map_or("", |(dir, _)| dir);
    std::iter::once(module.stem())
        .chain(dir.rsplit('/'))
        .filter_map(kebab)
        .find(|segment| segment != stem && !own.ends_with(&format!(".{segment}")))
}

impl Surface {
    // What the entry module does outside the handlers it registers. Another
    // surface's entry is what the bootstrap mounts, not what it does, so it is
    // reached and not followed; what loading that entry constructs runs before
    // any handler and is the bootstrap's to reach.
    pub(super) fn start(tree: &Tree, module: &Module, registered: &[Self]) -> Self {
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
        Self {
            name: "start".to_owned(),
            entry: module.path.clone(),
            stem: "start".to_owned(),
            lines: module.span,
            detail: vec![
                "the process bootstrap: what runs before each handler is registered, what it \
                 awaits before serving, and at shutdown — `stop` and what a signal handler calls, \
                 wherever declared"
                    .to_owned(),
            ],
            closure: tree.closure(&seeds, &entries),
            discriminator: None,
            methods: Vec::new(),
            ids: vec!["start".to_owned()],
        }
    }
}

// Under the module's mount. A verb's literal is a path only when it leads
// with a slash.
fn routed(
    module: &Module, method: &str, literal: Option<&str>, mounts: &BTreeMap<String, String>,
) -> Option<String> {
    let path = literal.filter(|literal| literal.starts_with('/'))?;
    VERBS.contains(&method).then(|| {
        let prefix = mounts.get(&module.path).map_or("", String::as_str);
        join_route(prefix, path)
    })
}

// What code reads at an anchor the survey accepted.
#[derive(Debug)]
pub struct Derived {
    // The stem the code spells at the anchor; `None` leaves the survey's
    // standing.
    pub stem: Option<String>,
    pub discriminator: Option<String>,
    // The public methods of the class declared or decorated at the anchor,
    // each with its lines.
    pub methods: Vec<(String, Lines)>,
    // The registration or declaration whole where the survey cited one line
    // of several, else the anchor.
    pub lines: Lines,
    // Notes for the brief.
    pub detail: Vec<String>,
    // The modules reached, the anchor's module first.
    pub closure: Vec<String>,
}

impl Derived {
    // The registration, decorated class, decorated method, or export at
    // `lines` is read, the first of those found. Where nothing is found, the
    // survey's `name` less its stem tells the surface apart and the lines are
    // read as they are.
    pub(super) fn derive(
        tree: &Tree, module: &Module, lines: Lines, name: &str, stem: &str,
        mounts: &BTreeMap<String, String>,
    ) -> Self {
        if let Some(call) = tree.registration_at(module, lines) {
            return Self::at_registration(tree, module, call, stem, mounts);
        }
        if let Some(class) = decorated_class_at(module, lines) {
            return Self::at_decorated_class(tree, module, class);
        }
        if let Some(decorated) = decorated_at(module, lines) {
            return Self::at_decorated_method(tree, module, decorated);
        }
        if let Some(export) = export_at(module, lines) {
            return Self::at_export(tree, module, export, stem);
        }
        Self {
            stem: None,
            discriminator: normalised(name, stem),
            methods: Vec::new(),
            lines,
            detail: vec![format!("named by the survey at {lines}")],
            closure: tree.reaches(module, lines, &[], None),
        }
    }

    // The stem its route or literal spells. What tells it apart is the
    // handler handed by name, else the route's verb and path past the
    // resource, else the registering method.
    fn at_registration(
        tree: &Tree, module: &Module, call: &Call, stem: &str, mounts: &BTreeMap<String, String>,
    ) -> Self {
        let (method, literal) = call.registered(tree.led(module, call));
        let literal = literal.as_deref();
        let route = routed(module, method, literal, mounts);
        let derived = route.as_deref().map_or_else(|| literal.and_then(literal_stem), route_stem);
        let stem = derived.as_deref().unwrap_or(stem);
        let discriminator = tree.named_handler(module, call).or_else(|| {
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
        Self {
            stem: derived,
            discriminator,
            methods: Vec::new(),
            lines: call.lines,
            detail,
            closure: tree.reaches(module, call.lines, &call.frames, call.class.as_deref()),
        }
    }

    // The stem its registering decorator's prefix spells, its name, and its
    // decorated methods.
    fn at_decorated_class(tree: &Tree, module: &Module, class: &Decorated) -> Self {
        let route = join_route(class.literal.as_deref().unwrap_or(""), "");
        let methods = unique(
            module
                .decorated
                .iter()
                .filter(|d| d.class == class.class && d.registering())
                .filter_map(|d| Some((d.member.clone()?, d.lines))),
        );
        let mut detail =
            vec![format!("class `{}` {} under `@{}`", class.class, class.lines, spelled(class))];
        detail.extend(through(module, class));
        detail.extend(noted(&methods));
        Self {
            stem: route_stem(&route),
            discriminator: kebab(&class.class),
            methods,
            lines: class.lines,
            detail,
            closure: tree.reaches(module, class.lines, &[], Some(&class.class)),
        }
    }

    // The stem its route or literal spells under the class's prefix, and the
    // method's name.
    fn at_decorated_method(tree: &Tree, module: &Module, decorated: &Decorated) -> Self {
        let member = decorated.member.as_deref().unwrap_or_default();
        let mut detail = vec![format!("method `{}.{member}` {}", decorated.class, decorated.lines)];
        detail.extend(through(module, decorated));
        Self {
            stem: decorated_stem(module, decorated),
            discriminator: kebab(member),
            methods: Vec::new(),
            lines: decorated.lines,
            detail,
            closure: tree.reaches(module, decorated.lines, &[], Some(&decorated.class)),
        }
    }

    // The export's name joined with the segments a file-routed module's path
    // spells past its stem, and a class's public methods.
    fn at_export(tree: &Tree, module: &Module, export: &Export, stem: &str) -> Self {
        let local = export.local.as_deref().unwrap_or(&export.name);
        let display =
            if export.name == "default" { module_stem(module) } else { export.name.clone() };
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
            ExportKind::Value | ExportKind::Type | ExportKind::Unknown => {
                ("value", Vec::new(), None)
            }
        };
        let declared = module.declared_at(export);
        let mut detail = vec![format!("exported {what} {declared}")];
        detail.extend(noted(&methods));
        Self {
            stem: None,
            discriminator: kebab(&parts.join("-")),
            methods,
            lines: declared,
            detail,
            closure: tree.reaches(module, declared, &[], class),
        }
    }
}

// As the code spells it, for a note: `Controller("orders")`.
fn spelled(decorated: &Decorated) -> String {
    let argument = decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
    format!("{}{argument}", decorated.name.join("."))
}

// The package a decorator comes from, as a note; none for the tree's own.
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

// The class decorator that registers the class and carries its prefix
// (`@Controller("orders")`), never one that documents it (`@ApiTags("Users")`).
fn class_prefix<'m>(module: &'m Module, class: &str) -> Option<&'m Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.class == class && d.member.is_none() && d.literal.is_some())
        .find(|d| d.registering())
}

// A verb decorator's path under the class's prefix, by the route rule;
// another's literal, by its first word; none where it spells neither.
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
// `lines`: an anchor at the class, or the module alone.
fn decorated_class_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.member.is_none() && d.registering())
        .find(|d| lines.holds(d.lines.start))
}

// The registering decorator of the first method starting within `lines`,
// else of the one enclosing their first line.
fn decorated_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    let methods = || module.decorated.iter().filter(|d| d.member.is_some() && d.registering());
    methods()
        .find(|d| lines.holds(d.lines.start))
        .or_else(|| methods().find(|d| d.lines.holds(lines.start)))
}

// A function or class whose declaration starts within `lines`, else a
// value's, else the one enclosing their first line.
fn export_at(module: &Module, lines: Lines) -> Option<&Export> {
    let exports = || module.exports.iter();
    exports()
        .filter(|export| export.kind.callable())
        .find(|export| lines.holds(module.declared_at(export).start))
        .or_else(|| {
            exports()
                .filter(|export| export.kind.valued())
                .find(|export| lines.holds(module.declared_at(export).start))
        })
        .or_else(|| {
            exports()
                .filter(|export| export.kind.callable() || export.kind.valued())
                .find(|export| module.declared_at(export).holds(lines.start))
        })
}

// The directory segments past the one that spells `stem`, a parameter by its
// bare name, a group by nothing: `app/api/orders/[id]/route.ts` under
// `orders` spells `id`. None when no segment spells the stem.
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

// The survey's name kebab-cased, less the words of its stem and the segments
// that only version or namespace a path: `GET /api/customers` under
// `customers` is `get`. None when nothing is left.
fn normalised(name: &str, stem: &str) -> Option<String> {
    let spelled = kebab(name)?;
    let kept: Vec<&str> = spelled
        .split('-')
        .filter(|word| !stem.split('-').any(|own| own == *word) && names_resource(word))
        .collect();
    kebab(&kept.join("-"))
}

// The public methods and getters of the class `name`, each with its lines.
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

// `/prefix/path`, however either is spelled: `@Controller("orders")` and
// `app.use("/api/", ..)` both lead with one slash and end with none.
fn join_route(prefix: &str, path: &str) -> String {
    let segments: Vec<&str> =
        prefix.split('/').chain(path.split('/')).filter(|segment| !segment.is_empty()).collect();
    format!("/{}", segments.join("/"))
}

// The verb, then the path segments past the one that spells the stem, a
// parameter by its bare name: `GET /api/orders/:id` under `orders` is
// `get-id`. A route spelling no segment as its stem is relative to a mount
// the survey did not read, so every segment tells: `POST /:id/assign` under
// `tasks` is `post-id-assign`.
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

// The first segment that names a resource: not a version, an `api`
// namespace, a parameter, or a pattern.
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

// The first word of a literal that names something (a command, a queue, an
// event); nothing for a pattern or a schedule.
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

// Camel humps split, anything that is not a letter or digit a hyphen, runs
// collapsed; none when nothing is left.
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
