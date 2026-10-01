//! Has the model name the surfaces of a tree, from the facts the parser read.
//!
//! The parser reads the tree; the model decides what in it is a surface.
//! What the parser read — the manifest and the scripts it installs, the
//! bootstrap, every call that hands a function to something a package
//! provides, every `def` and class under a package's decorator, what the
//! entry modules export, the packages imported — is handed to the model,
//! laid beside the modules that fit, and it names each surface, anchors it
//! where it is registered or declared, and gives the stem its ids lead
//! with. Code then holds the answer to the tree: every module the facts
//! locate a surface in is reached by a named surface or listed as
//! unreached, and the bootstrap's `start` is the caller's, never the
//! model's. From the accepted anchors the code derives the rest — the
//! closure each surface reaches, the bootstrap surface, the stem the code
//! spells at the anchor where it spells one, what tells each surface from
//! the others under its stem, a class's methods, and the ids — read from the
//! registration, decorator, or export at the anchor, never from the model's
//! name for it, so two runs that accept the same anchors cut the same seams.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::survey::{Facts, Inventory};
use emery_sdk::{Context, Doc, Error, Model};

use super::parse::{ClassDecl, Decorated, Export, ExportKind, Lines, Module};
use super::resolve::Target;
use super::surface::{Runs, Surface, Tree};
use super::{Prepared, push_unique, skeleton, surface, unique};

/// The surfaces the model names in a prepared tree, held to it, with the
/// bootstrap surface before them and every id decided.
///
/// # Errors
///
/// The SDK's: the model rejects the request or no acceptable answer lands
/// within the rounds (`bad_request`), the prompt is not embedded
/// (`server_error`), or the model transport fails (`bad_gateway`).
pub async fn surfaces<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc], prepared: &Prepared,
) -> Result<Vec<Surface>, Error> {
    let located = Located::new(&prepared.tree);
    let modules: Vec<String> = located.tree.modules.keys().cloned().collect();
    let text = facts(&located);
    let files = laid(&located, &prepared.root);
    let facts = Facts {
        modules: &modules,
        text: &text,
        files: &files,
    };

    let inventory =
        emery_sdk::survey::surfaces(ctx, docs, &facts, |answer| check(&located, answer)).await?;

    let surfaces = build(&located, &inventory);
    let restemmed: Vec<String> = inventory
        .surfaces
        .iter()
        .filter_map(|named| {
            let built = surfaces.iter().find(|surface| surface.name == named.name)?;
            (built.stem != named.stem)
                .then(|| format!("`{}`: `{}` for `{}`", named.name, built.stem, named.stem))
        })
        .collect();
    if !restemmed.is_empty() {
        emery_sdk::tracing::debug!(
            source = %prepared.source,
            ?restemmed,
            "stems the code spells at the anchors, in place of the survey's"
        );
    }

    // what no surface reaches, by code's own count: the modules the model
    // listed and the ones the facts said nothing of, which the gate took as
    // unreached without asking
    let covered: BTreeSet<&str> =
        surfaces.iter().flat_map(|surface| surface.closure.iter().map(String::as_str)).collect();
    let unplaced: Vec<&String> =
        located.tree.modules.keys().filter(|path| !covered.contains(path.as_str())).collect();
    emery_sdk::tracing::info!(
        source = %prepared.source,
        surfaces = surfaces.len(),
        unplaced = unplaced.len(),
        "placed by model"
    );
    emery_sdk::tracing::debug!(source = %prepared.source, ?unplaced, "modules under no surface");
    Ok(surfaces)
}

// What the tree says of itself before the model is asked, read once and
// held through every round: the bootstrap and how it runs, the entries the
// manifest's scripts name, the modules no other imports, the classes a
// registration hands, the modules the facts locate a surface in, and where
// each module's routes are mounted.
struct Located<'t> {
    tree: &'t Tree,
    bootstrap: Option<(&'t Module, Runs)>,
    entries: Vec<String>,
    roots: Vec<String>,
    handed: BTreeSet<(String, String)>,
    locating: BTreeSet<&'t str>,
    mounts: BTreeMap<String, String>,
}

impl<'t> Located<'t> {
    fn new(tree: &'t Tree) -> Self {
        let handed = tree.handed_classes();
        Self {
            tree,
            bootstrap: tree.bootstrap(),
            entries: tree
                .manifest
                .entries(&tree.resolver)
                .into_iter()
                .map(|(module, _)| module)
                .collect(),
            roots: surface::roots(tree),
            locating: tree
                .modules
                .values()
                .filter(|module| locates(tree, module, &handed))
                .map(|module| module.path.as_str())
                .collect(),
            handed,
            mounts: surface::mounts(tree),
        }
    }
}

// A module the facts locate a surface in: one that hands a function to a
// package's receiver outside any handler in the shape a registration has —
// discarded, constructing, or led by a literal, so a function handed to a
// wrapper for its value (`handler = retrying(send)`) is listed among the
// facts and locates nothing — or one a package's decorator registers a `def`
// or a method of — a method of a class a registration hands excepted, which
// the registration locates. One the survey leaves reached by no surface and
// unlisted is a finding; a module the facts say nothing of — a settings
// module, a helper nothing imports — is taken as unreached without one.
fn locates(tree: &Tree, module: &Module, handed: &BTreeSet<(String, String)>) -> bool {
    module.calls.iter().any(|call| {
        surface::handed(tree, module, call).is_some() && surface::registers(tree, module, call)
    }) || module.decorated.iter().any(|decorated| {
        decorated.member.is_some()
            && surface::registering(decorated)
            && !of_handed(module, decorated, handed)
            && decorated
                .name
                .first()
                .is_some_and(|head| surface::receiver_of(tree, module, head).is_some())
    })
}

// Whether a decorator sits on a method of a class a registration hands —
// a view set's `@action` — which the registration carries as an id.
fn of_handed(module: &Module, decorated: &Decorated, handed: &BTreeSet<(String, String)>) -> bool {
    match (&decorated.class, &decorated.member) {
        (Some(class), Some(_)) => handed.contains(&(module.path.clone(), class.clone())),
        _ => false,
    }
}

// What the parser read of the tree, for the model to decide from: the
// manifest, the bootstrap and what the caller makes of it, every call that
// hands a function to a package's receiver, every decorated `def` and
// class, what the entry modules export, the packages imported, and what
// could not be followed.
fn facts(located: &Located<'_>) -> String {
    let tree = located.tree;
    let mut sections: Vec<String> = Vec::new();

    // the manifest
    let manifest = &tree.manifest;
    let mut said: Vec<String> = Vec::new();
    if let Some(name) = &manifest.name {
        said.push(format!("names the package `{name}`"));
    }
    if !manifest.scripts.is_empty() {
        let scripts: Vec<String> = manifest
            .scripts
            .iter()
            .map(|(name, target)| format!("`{name}` runs `{target}`"))
            .collect();
        said.push(format!("installs the console scripts {}", scripts.join(", ")));
    }
    sections.push(if said.is_empty() {
        "The tree has no `pyproject.toml` or `setup.cfg` the parser could read.".to_owned()
    } else {
        format!("The manifest {}.", said.join("; "))
    });

    // the bootstrap
    sections.push(located.bootstrap.as_ref().map_or_else(
        || {
            "No bootstrap runs at load: no module a console script names, and no conventional \
             entry the tree holds, runs anything when loaded or under a `__main__` guard. The \
             surfaces are what the tree exposes without one — what its entry modules export for \
             a caller, or what a convention of the framework it uses makes reachable: a URL \
             module a setting names, a command a directory names."
                .to_owned()
        },
        |(module, runs)| {
            let how = match runs {
                Runs::Script(script, function) => {
                    format!("the console script `{script}` calls its `{function}()`")
                }
                Runs::Guard => "it runs under its `__main__` guard".to_owned(),
                Runs::Load => "it runs or constructs the application at load".to_owned(),
            };
            format!(
                "The bootstrap — the entry that runs something — is `{}`: {how}. It is the \
                 caller's `start` surface: the caller names it itself, so do not list it, and \
                 lead no surface with the stem `start`. What it mounts, registers, schedules, or \
                 subscribes — and what any module it reaches registers — are the surfaces to \
                 name, each anchored where it is registered or declared.",
                module.path
            )
        },
    ));

    sections.extend(registrations(tree));
    sections.extend(decorated(located));
    sections.extend(exports(located));
    sections.extend(skeleton::packages(tree.modules.values()));
    sections.extend(skeleton::unfollowed(tree.modules.values(), false));

    sections.join("\n\n")
}

// Every call, outside any handler, that hands a function to something a
// package provides — where a framework is told what to run — with the
// literal that led it, as the code derives a stem from it, and the package,
// at its lines. What each is, is the model's to say.
fn registrations(tree: &Tree) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for module in tree.modules.values() {
        for call in &module.calls {
            let Some(receiver) = surface::handed(tree, module, call) else { continue };
            let (_, literal) = surface::registered(call, surface::led(tree, module, call));
            let led = literal.map(|literal| format!(" led by `\"{literal}\"`")).unwrap_or_default();
            let spelled = if call.constructs || call.callee.links.is_empty() {
                format!("{}(..)", call.method())
            } else {
                format!("{}.{}", call.callee.receiver(), call.method())
            };
            let typed = receiver
                .type_name
                .as_deref()
                .map(|name| format!(" as `{name}`"))
                .unwrap_or_default();
            lines.push(format!(
                "- `{}#{}` — `{spelled}`{led} handed a function, through `{}`{typed}",
                module.path,
                call.lines.anchor(),
                receiver.package
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Calls that hand a function or a class to something a package provides, each at its \
         lines — where the code tells a framework, a queue, a scheduler, or a CLI what to run. A \
         surface is usually registered by one of these; a hook on a surface already registered \
         (an error handler, a signal receiver, a startup event) is not a surface of its \
         own:\n\n{}",
        lines.join("\n")
    ))
}

// Every `def`, method, and class under a decorator a package provides —
// one that registers rather than shapes — with the decorator's literal, at
// its lines; a method of a class a registration hands is left to the
// registration, which carries the class's actions as ids.
fn decorated(located: &Located<'_>) -> Option<String> {
    let tree = located.tree;
    let mut lines: Vec<String> = Vec::new();
    for module in tree.modules.values() {
        for decorated in module
            .decorated
            .iter()
            .filter(|d| surface::registering(d) && !of_handed(module, d, &located.handed))
        {
            let Some(receiver) =
                decorated.name.first().and_then(|head| surface::receiver_of(tree, module, head))
            else {
                continue;
            };
            let decorator = decorated.name.join(".");
            let argument =
                decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
            let on = match (&decorated.class, &decorated.member) {
                (Some(class), Some(member)) => format!("`{class}.{member}`"),
                (None, Some(member)) => format!("`{member}`"),
                (Some(class), None) => format!("class `{class}`"),
                (None, None) => continue,
            };
            lines.push(format!(
                "- `{}#{}` — `@{decorator}{argument}` on {on}, through `{}`",
                module.path,
                decorated.lines.anchor(),
                receiver.package
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Decorators a package provides, each at its lines — where a framework is told what a \
         function, a method, or a class answers: a route, a command, a task, a consumer. A \
         decorator that only shapes what it decorates — a dataclass, a property, a cache, a \
         guard — is not listed:\n\n{}",
        lines.join("\n")
    ))
}

// What the entry modules — the ones the manifest's scripts name, and the
// ones no other module imports — export, with the kind and lines of each,
// and what a package's `__init__` among them re-exports, one hop, at the
// module that declares it.
fn exports(located: &Located<'_>) -> Option<String> {
    let tree = located.tree;
    let entries = unique(located.entries.iter().chain(&located.roots));
    let mut lines: Vec<String> = Vec::new();
    for module in entries.iter().filter_map(|path| tree.modules.get(*path)) {
        let exported = declared(module, module.exports.iter());
        if !exported.is_empty() {
            lines.push(format!("- `{}` exports {}", module.path, exported.join(", ")));
        }
        for reexport in module.reexports.iter().filter(|reexport| !reexport.type_only) {
            let Some(path) = reexport.target.as_ref().and_then(Target::module) else { continue };
            let Some(target) = tree.modules.get(path) else { continue };
            let exported = reexport.names.as_ref().map_or_else(
                || declared(target, target.exports.iter()),
                |names| {
                    declared(
                        target,
                        names.iter().filter_map(|(imported, _)| target.export(imported)),
                    )
                },
            );
            if !exported.is_empty() {
                lines.push(format!(
                    "- `{}` re-exports from `{}`, where each is declared: {}",
                    module.path,
                    target.path,
                    exported.join(", ")
                ));
            }
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "What the entry modules — the ones the manifest's scripts name, and the ones no other \
         module imports — export, each with its kind and lines, and what a package's `__init__` \
         among them re-exports at the module that declares it. In a tree with no bootstrap, \
         these are where a caller enters:\n\n{}",
        lines.join("\n")
    ))
}

// The exports of `module` that declare something a caller calls, each with
// its kind and lines; a type is none, and so is a class that declares data
// alone — a dataclass or a model of fields and properties — which the
// caller copies as a type.
fn declared<'e>(module: &Module, exports: impl Iterator<Item = &'e Export>) -> Vec<String> {
    exports
        .filter_map(|export| {
            let kind = match &export.kind {
                ExportKind::Function => "function",
                ExportKind::Class
                    if module.class(&export.name).is_some_and(ClassDecl::declares_data) =>
                {
                    return None;
                }
                ExportKind::Class => "class",
                ExportKind::Value => "value",
                ExportKind::Type => return None,
            };
            Some(format!("`{}` ({kind}) {}", export.name, export.lines))
        })
        .collect()
}

// The files laid into the turn whole, as far as they fit: the manifest, the
// bootstrap, the modules the facts cite — where a function is handed to a
// package, where a package's decorator registers a `def`, the entry modules
// whose exports are listed — then the rest of the tree in path order, so a
// tree past the budget lays what locates its surfaces before what does not.
fn laid(located: &Located<'_>, root: &str) -> Vec<String> {
    let tree = located.tree;
    let mut files: Vec<String> = Vec::new();
    for manifest in ["pyproject.toml", "setup.cfg"] {
        if Path::new(root).join(manifest).is_file() {
            files.push(manifest.to_owned());
        }
    }
    if let Some((module, _)) = &located.bootstrap {
        push_unique(&mut files, module.path.clone());
    }
    for path in tree
        .modules
        .keys()
        .filter(|path| located.locating.contains(path.as_str()))
        .chain(&located.entries)
        .chain(&located.roots)
        .chain(tree.modules.keys())
    {
        push_unique(&mut files, path.clone());
    }
    files
}

// What the tree alone can hold the model's answer to: no surface under the
// bootstrap's stem, and every module the facts locate a surface in — or a
// script names — reached by a named surface or the bootstrap, or listed as
// unreached. A module the facts say nothing of is not asked after: a
// framework may load it by a setting's name, and a model made to account
// for every such module invents surfaces to cover them.
fn check(located: &Located<'_>, answer: &Inventory) -> Vec<String> {
    let tree = located.tree;
    let mut findings: Vec<String> = Vec::new();
    if located.bootstrap.is_some() {
        for named in answer.surfaces.iter().filter(|named| named.stem == "start") {
            findings.push(format!(
                "- surface `{}`: `start` is the bootstrap's stem, which the caller names itself; \
                 give the surface the stem of what a caller does through it",
                named.name
            ));
        }
    }

    // what the named surfaces reach, and what the bootstrap reaches beside them
    let surfaces = build(located, answer);
    let covered: BTreeSet<&str> =
        surfaces.iter().flat_map(|surface| surface.closure.iter().map(String::as_str)).collect();
    let unreached: BTreeSet<&str> = answer.unreached.iter().map(String::as_str).collect();
    let missing: Vec<String> = tree
        .modules
        .keys()
        .filter(|path| {
            !covered.contains(path.as_str())
                && !unreached.contains(path.as_str())
                && (located.entries.contains(path) || located.locating.contains(path.as_str()))
        })
        .map(|path| format!("`{path}`"))
        .collect();
    if !missing.is_empty() {
        let (these, are) = if missing.len() == 1 {
            ("one module the facts list a registration or declaration in is", "it")
        } else {
            ("these modules the facts list a registration or declaration in are", "each")
        };
        findings.push(format!(
            "- {these} reached by no surface you named and not listed under `unreached`: {}; where \
             a caller outside the process reaches what is registered there — a route, a command, \
             a task — name that surface, anchored at the registration or the decorated `def`, so \
             the caller follows its imports to the module; otherwise list {are} under \
             `unreached` — a hook, a signal receiver, a helper is no surface, and none is \
             invented to cover a module",
            missing.join(", ")
        ));
    }
    findings
}

// The surfaces the answer names, each at the module and lines its anchor
// names, carrying what the code there says of it — the registration or
// declaration whole, the modules it reaches, the stem it spells, where it
// spells one, in place of the survey's; what tells it from the others under
// that stem; a class's methods — as the parser's own are read from a
// registration; the bootstrap's `start` before them, reaching what they do
// not; and every id decided over them all. A surface whose anchor the tree
// does not hold is left out; the SDK's gate has refused it already.
fn build(located: &Located<'_>, answer: &Inventory) -> Vec<Surface> {
    let tree = located.tree;
    let mut surfaces: Vec<Surface> = answer
        .surfaces
        .iter()
        .filter_map(|named| {
            let anchor = named.anchor().ok()?;
            let module = tree.modules.get(anchor.path)?;
            let lines = anchor.lines.map_or(module.span, Lines::from);
            let derived =
                surface::derive(tree, module, lines, &named.name, &named.stem, &located.mounts);
            Some(Surface {
                name: named.name.clone(),
                entry: module.path.clone(),
                stem: derived.stem.unwrap_or_else(|| named.stem.clone()),
                lines: derived.lines,
                detail: derived.detail,
                closure: derived.closure,
                discriminator: derived.discriminator,
                methods: derived
                    .methods
                    .iter()
                    .filter_map(|(name, _)| surface::kebab(name))
                    .collect(),
                ids: Vec::new(),
            })
        })
        .collect();
    if let Some((module, runs)) = &located.bootstrap {
        let start = surface::start(tree, module, runs, &surfaces);
        surfaces.insert(0, start);
    }
    surface::identify(tree, &mut surfaces);
    surfaces
}
