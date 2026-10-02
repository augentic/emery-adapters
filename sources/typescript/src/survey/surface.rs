//! Recognises what TypeScript and JavaScript alone decide of a tree's surfaces.
//!
//! A surface is where control enters from outside the process: the
//! bootstrap the manifest names, a handler handed to something a package
//! provides, a method under a package's decorator, or, in a library, what
//! the entry module exports. The SDK's pipeline lays the facts, puts the
//! survey turn, holds the answer to the tree, and cuts the seams; what it
//! asks of this adapter is what the language decides — where each import
//! leads, what the manifest declares, what a test states, which entry runs
//! when the tree loads, what a call hands, where routers are mounted, and
//! what the code says at an anchor the survey accepted: the stem a route or
//! literal spells, the id that tells the surface from the others under its
//! stem, a class's methods, and the modules the surface reaches.
//!
//! The rules read shapes, never a framework's name: a call handed a
//! function, a decorator with a literal, an export. The one exception
//! spells names. A decorator among those the data model lists as shaping a
//! handler (`Decorated::registering`) does not register it, which keeps a
//! verb decorator beneath one of them the surface.

use std::collections::BTreeMap;
use std::path::Path;

use emery_sdk::kebab;
use emery_sdk::survey::code::{
    Arg, BindingKind, Bootstrap, Call, Decorated, Derived, Export, ExportKind, Imported, Init,
    Manifest, MemberKind, Parsed, Recogniser, Runs, Tree,
};
use emery_sdk::survey::resolve::{Target, normalize};
use emery_sdk::survey::tests::Statement;
use emery_sdk::survey::{Lines, route};

use super::parse::Module;
use super::resolve::{self, Resolver};
use super::{DIALECT, unique};

// Looked for in this order when the manifest names no entry.
const BOOTSTRAPS: &[&str] =
    &["src/index.ts", "src/main.ts", "index.ts", "main.ts", "src/server.ts", "src/app.ts"];

const VERBS: &[&str] = &["get", "post", "put", "patch", "delete", "head", "options", "all"];

// Methods that mount what they are handed: `use` a router under a path, any
// of them a loader over a directory. A route registered in a mounted module
// is read under its mount.
const MOUNTING: &[&str] = &["use", "register", "mount", "plugin"];

// What the adapter recognises of a tree, over its resolver and manifest.
#[derive(Debug)]
pub struct TypeScript {
    resolver: Resolver,
    manifest: resolve::Manifest,
}

impl TypeScript {
    // The manifest read and the resolver built over the parsed tree, before
    // any import is settled. `tests` are the test modules the keep set
    // aside, which an import may still reach.
    pub(super) fn new(root: &Path, parsed: &Parsed<Module>, tests: Vec<String>) -> Self {
        Self {
            resolver: Resolver::new(
                parsed.modules.keys().cloned(),
                parsed.data.iter().cloned(),
                tests,
                root,
            ),
            manifest: resolve::Manifest::read(root),
        }
    }

    // Kebab-cased; none for an inline function.
    fn named_handler(&self, tree: &Tree<Self>, module: &Module, call: &Call) -> Option<String> {
        call.args
            .iter()
            .find(|arg| self.handler(tree, module, arg, &call.frames))
            .filter(|arg| !arg.function)
            .and_then(|arg| kebab(arg.root.as_ref()?.last()?))
    }

    // The stem its route or literal spells. What tells it apart is the
    // handler handed by name, else the route's verb and path past the
    // resource, else the registering method.
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
        }
    }
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

    // The first entry that runs something, probed in this order:
    // - the entries the manifest names
    // - the conventional entries the tree holds
    // An entry runs by a module-level call whose value is discarded, or an
    // import for its effect. One that only declares and exports is a
    // library's: a `main` naming the barrel while `start` runs the server.
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

    // A function, an object or call carrying one, or a name bound to a
    // function in the tree: declared here, imported from a module of the
    // tree, or a member of a namespace import (`handlers.list`).
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

    // The route prefix each module's routes sit under:
    // - `app.use("/api", ordersRouter(..))` gives `routes/orders.ts` `/api`
    // - a loader registered over a directory of the tree
    //   (`app.register(autoload, { dir: join(__dirname, "routes") })`) gives
    //   each module beneath it the path of its directory from there, under
    //   the loader's `prefix` option where it names one, so
    //   `routes/api/auth/index.ts` is `/api/auth` and `routes/home.ts` sits
    //   at the root
    // The first mount read of a module stands.
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

    // The registration, decorated class, decorated method, or export at
    // `lines` is read, the first of those found.
    fn derive(
        &self, tree: &Tree<Self>, module: &Module, lines: Lines, name: &str, stem: &str,
        mounts: &BTreeMap<String, String>,
    ) -> Derived {
        if let Some(call) = tree
            .registration_at(module, lines)
            .or_else(|| tree.registration_enclosing(module, lines))
        {
            return self.at_registration(tree, module, call, stem, mounts);
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
        Derived::named(tree, module, lines, name, stem)
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
        route::join(prefix, path)
    })
}

// The stem its registering decorator's prefix spells, its name, and its
// decorated methods.
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
    }
}

// The stem its route or literal spells under the class's prefix, and the
// method's name.
fn at_decorated_method(tree: &Tree<TypeScript>, module: &Module, decorated: &Decorated) -> Derived {
    let class = decorated.class.as_deref().unwrap_or_default();
    let member = decorated.member.as_deref().unwrap_or_default();
    let mut detail = vec![format!("method `{class}.{member}` {}", decorated.lines)];
    detail.extend(through(module, decorated));
    Derived {
        stem: decorated_stem(module, decorated),
        discriminator: kebab(member),
        methods: Vec::new(),
        lines: decorated.lines,
        detail,
        closure: tree.reaches(module, decorated.lines, &[], decorated.class.as_deref()),
    }
}

// The export's name joined with the segments a file-routed module's path
// spells past its stem, and a class's public methods.
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
fn class_prefix<'m>(module: &'m Module, class: Option<&str>) -> Option<&'m Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.class.as_deref() == class && d.member.is_none() && d.literal.is_some())
        .find(|d| d.registering(&DIALECT))
}

// A verb decorator's path under the class's prefix, by the route rule;
// another's literal, by its first word; none where it spells neither.
fn decorated_stem(module: &Module, decorated: &Decorated) -> Option<String> {
    let verb = decorated.name.last()?.to_ascii_lowercase();
    if VERBS.contains(&verb.as_str()) {
        let prefix =
            class_prefix(module, decorated.class.as_deref()).and_then(|d| d.literal.as_deref());
        let route = route::join(prefix.unwrap_or(""), decorated.literal.as_deref().unwrap_or(""));
        DIALECT.route.stem(&route)
    } else {
        decorated.literal.as_deref().and_then(route::literal_stem)
    }
}

// The registering decorator of a class whose declaration starts within
// `lines`: an anchor at the class, or the module alone.
fn decorated_class_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    module
        .decorated
        .iter()
        .filter(|d| d.member.is_none() && d.registering(&DIALECT))
        .find(|d| lines.holds(d.lines.start))
}

// The registering decorator of the first method starting within `lines`,
// else of the one enclosing their first line.
fn decorated_at(module: &Module, lines: Lines) -> Option<&Decorated> {
    let methods =
        || module.decorated.iter().filter(|d| d.member.is_some() && d.registering(&DIALECT));
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

// The public methods and getters of the class `name`, each with its lines.
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
