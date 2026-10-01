//! Has the model name the surfaces of a tree from the facts the parser read.
//!
//! The parser reads; the model decides what is a surface. The facts laid
//! before it are the manifest and its scripts, the bootstrap, every call
//! that hands a function to something a package provides, every `def` and
//! class under a package's decorator, what the entry modules export, and
//! the packages imported. The model names each surface, anchors it where it
//! is registered or declared, and gives the stem its ids lead with.
//!
//! Code holds the answer to the tree and derives the rest from the accepted
//! anchors, never from the model's names, so two runs that accept the same
//! anchors cut the same seams. The bootstrap's `start` is the code's own
//! surface, never the model's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::survey::{Facts, Inventory};
use emery_sdk::{Context, Doc, Error, Model};

use super::parse::{ClassDecl, Decorated, Export, ExportKind, Lines, Module};
use super::resolve::Target;
use super::surface::{Runs, Surface, Tree};
use super::{Prepared, push_unique, skeleton, surface, unique};

// The bootstrap surface leads, and every id is decided over them all. The
// errors are the SDK's: `bad_request` when no acceptable answer lands within
// the rounds, `server_error` for a prompt not embedded, `bad_gateway` for a
// transport failure.
pub async fn surfaces<P: Model>(
    ctx: &Context<'_, P>, docs: &'static [Doc], prepared: &Prepared,
) -> Result<Vec<Surface>, Error> {
    let located = Located::new(&prepared.tree);
    let modules: Vec<String> = located.tree.modules.keys().cloned().collect();
    let text = facts(&located);
    let files = laid(&located, &prepared.root);
    // the facts as one JSON string, for a host reading what the model was
    // told without a turn
    if emery_sdk::tracing::enabled!(emery_sdk::tracing::Level::TRACE) {
        let json = emery_sdk::serde_json::Value::String(text.clone()).to_string();
        emery_sdk::tracing::trace!(source = %prepared.source, facts = %json, "survey facts");
    }
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

    // log the modules under no surface
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

// Read once before the model is asked and held through every correction
// round.
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

// Whether the facts place a registration or a registering decorator in
// `module`, so the answer must reach it or list it `unreached`. A method of
// a class a registration hands is the registration's to locate.
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

// A view set's `@action` is an id of the registration, not a surface.
fn of_handed(module: &Module, decorated: &Decorated, handed: &BTreeSet<(String, String)>) -> bool {
    match (&decorated.class, &decorated.member) {
        (Some(class), Some(_)) => handed.contains(&(module.path.clone(), class.clone())),
        _ => false,
    }
}

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

    // the registrations, decorations, exports, and what could not be followed
    sections.extend(registrations(tree));
    sections.extend(decorated(located));
    sections.extend(exports(located));
    sections.extend(skeleton::packages(tree.modules.values()));
    sections.extend(skeleton::unfollowed(tree.modules.values(), false));

    sections.join("\n\n")
}

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

// A re-export is followed one hop, to the module that declares it.
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

// A type, or a class declaring data alone, is the caller's to copy, not to
// call.
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

// Ordered so a tree past the budget lays what locates its surfaces before
// what does not.
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

// A module the facts say nothing of is not asked after: a framework may
// load it by a setting's name, and a model made to account for every such
// module invents surfaces to cover them.
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

    // modules the facts locate a surface in that no surface reaches
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

// An anchor the tree does not hold is skipped: the SDK's gate has refused it
// already.
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
