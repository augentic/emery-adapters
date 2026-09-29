//! Has the model name the surfaces of a tree, from the facts the parser read.
//!
//! The parser's survey decides the surfaces from the code alone; this arm
//! hands what it read — the manifest, the bootstrap, every call that hands a
//! function to something a package provides, every method under a package's
//! decorator, what the entry modules export, the packages imported — to the
//! model, laid beside the modules that fit, and asks it to name each surface,
//! anchor it where it is registered or declared, and give the stem its ids
//! lead with. Code then holds the answer to the tree: every module is reached
//! by a named surface or listed as unreached, and the bootstrap's `start` is
//! the caller's, never the model's. From the accepted anchors the same code
//! as the parser's derives the closure each surface reaches, the bootstrap
//! surface, what tells each surface from the others under its stem (read
//! from the registration, decorator, or export at the anchor, never from the
//! model's name for it), and the ids, so the seams cut from either arm
//! differ in the naming alone.

use std::collections::BTreeMap;
use std::path::Path;

use emery_sdk::survey::{Facts, Inventory};
use emery_sdk::{Context, Doc, Error, Model};

use super::parse::{Call, ExportKind, Lines, Module};
use super::surface::{Receiver, Surface, Tree};
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
    let tree = &prepared.tree;
    let bootstrap = tree.bootstrap();
    let modules: Vec<String> = tree.modules.keys().cloned().collect();
    let text = facts(tree, bootstrap.as_deref());
    let lay = laid(tree, &prepared.root, bootstrap.as_deref());
    let facts = Facts {
        modules: &modules,
        text: &text,
        lay: &lay,
    };
    let mounts = surface::mounts(tree, &modules);

    let inventory = emery_sdk::survey::surfaces(ctx, docs, &facts, |answer| {
        check(tree, bootstrap.as_deref(), &mounts, answer)
    })
    .await?;

    let mut surfaces = build(tree, &inventory, &mounts);
    if let Some(module) = bootstrap.as_deref().and_then(|entry| tree.modules.get(entry)) {
        let start = surface::start(tree, module, &surfaces);
        surfaces.insert(0, start);
    }
    surface::identify(tree, &mut surfaces);

    // what no surface reaches, by code's own count: the modules the model
    // listed and the ones the facts said nothing of, which the gate took as
    // unreached without asking
    let covered = unique(surfaces.iter().flat_map(|surface| surface.closure.iter()));
    let unplaced: Vec<&String> =
        tree.modules.keys().filter(|path| !covered.contains(path)).collect();
    emery_sdk::tracing::info!(
        source = %prepared.source,
        surfaces = surfaces.len(),
        unplaced = unplaced.len(),
        "placed by model"
    );
    emery_sdk::tracing::debug!(source = %prepared.source, ?unplaced, "modules under no surface");
    Ok(surfaces)
}

// A module the facts locate a surface in: one that hands a function to a
// package's receiver outside any handler in the shape a registration has —
// discarded, constructing, or led by a literal, so a plugin definition
// handed to a wrapper for its value (`export default fp(async (app) => ..)`)
// is listed among the facts and locates nothing — or one a package's
// decorator marks a method of. One the survey leaves reached by no surface
// and unlisted is a finding; a module the facts say nothing of — a plugin a
// framework loads by directory, a helper nothing imports — is taken as
// unreached without one.
fn locates(tree: &Tree, module: &Module) -> bool {
    module.calls.iter().any(|call| {
        registration(tree, module, call).is_some() && surface::registers(tree, module, call)
    }) || module.decorated.iter().any(|decorated| {
        decorated.member.is_some()
            && decorated.name.first().is_some_and(|head| tree.package(module, head).is_some())
    })
}

// What the parser read of the tree, for the model to decide from: the
// manifest, the bootstrap and what the caller makes of it, every call that
// hands a function to a package's receiver, every decorated method, what the
// entry modules export, the packages imported, and what could not be
// followed.
fn facts(tree: &Tree, bootstrap: Option<&str>) -> String {
    let mut sections: Vec<String> = Vec::new();

    // the manifest
    let manifest = &tree.manifest;
    let mut said: Vec<String> = Vec::new();
    if let Some(name) = &manifest.name {
        said.push(format!("names the package `{name}`"));
    }
    if let Some(main) = &manifest.main {
        said.push(format!("`main` is `{main}`"));
    }
    if let Some(bin) = &manifest.bin {
        said.push(format!("`bin` is `{bin}`"));
    }
    if !manifest.scripts.is_empty() {
        let scripts: Vec<String> = manifest
            .scripts
            .iter()
            .map(|(name, command)| format!("`{name}` runs `{command}`"))
            .collect();
        said.push(format!("scripts: {}", scripts.join(", ")));
    }
    sections.push(if said.is_empty() {
        "The tree has no `package.json` the parser could read.".to_owned()
    } else {
        format!("The manifest `package.json` {}.", said.join("; "))
    });

    // the bootstrap
    sections.push(bootstrap.map_or_else(
        || {
            "No bootstrap runs at load: no entry the manifest names, and no conventional entry \
             the tree holds, runs anything when loaded. The surfaces are what the tree exposes \
             without one — what its entry modules export for a caller, or what a convention of \
             the framework it uses makes reachable: a route module a file path names, a handler \
             an export names."
                .to_owned()
        },
        |entry| {
            format!(
                "The bootstrap — the entry that runs something when loaded — is `{entry}`. It is \
                 the caller's `start` surface: the caller names it itself, so do not list it, and \
                 lead no surface with the stem `start`. What it mounts, registers, schedules, or \
                 subscribes — and what any module it reaches registers — are the surfaces to \
                 name, each anchored where it is registered."
            )
        },
    ));

    sections.extend(registrations(tree));
    sections.extend(decorated(tree));
    sections.extend(exports(tree));
    sections.extend(skeleton::packages(tree.modules.values(), &tree.resolver));
    sections.extend(skeleton::unfollowed(tree.modules.values(), &tree.resolver, false));

    sections.join("\n\n")
}

// Every call, outside any handler, that hands a function to something a
// package provides — where a framework is told what to run — with what led
// it and the package, at its lines. What each is, is the model's to say.
fn registrations(tree: &Tree) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for module in tree.modules.values() {
        for call in &module.calls {
            let Some(receiver) = registration(tree, module, call) else { continue };
            let literal = call.literal().map(str::to_owned).or_else(|| {
                call.callee.links.iter().find_map(|link| link.call.as_ref()?.literal.clone())
            });
            let led = literal.map(|literal| format!(" led by `\"{literal}\"`")).unwrap_or_default();
            let spelled = if call.is_new {
                format!("new {}", call.method())
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
        "Calls that hand a function to something a package provides, each at its lines — where \
         the code tells a framework, a queue, a scheduler, or a CLI what to run. A surface is \
         usually registered by one of these; a hook on a surface already registered (an error \
         listener, a completion handler) is not a surface of its own:\n\n{}",
        lines.join("\n")
    ))
}

// The receiver a call hands a function to, when the call is one outside any
// handler that hands one to something a package provides.
fn registration(tree: &Tree, module: &Module, call: &Call) -> Option<Receiver> {
    if call.depth > 0
        || call.structural()
        || !call.args.iter().any(|arg| tree.handler(module, arg, &call.frames))
    {
        return None;
    }
    tree.receiver(module, call)
}

// Every method and class under a decorator a package provides, with the
// decorator's literal, at its lines.
fn decorated(tree: &Tree) -> Option<String> {
    let mut lines: Vec<String> = Vec::new();
    for module in tree.modules.values() {
        for decorated in &module.decorated {
            let Some(package) = decorated.name.first().and_then(|head| tree.package(module, head))
            else {
                continue;
            };
            let decorator = decorated.name.join(".");
            let argument =
                decorated.literal.as_ref().map(|l| format!("(\"{l}\")")).unwrap_or_default();
            let on = decorated.member.as_ref().map_or_else(
                || format!("class `{}`", decorated.class),
                |member| format!("`{}.{member}`", decorated.class),
            );
            lines.push(format!(
                "- `{}#{}` — `@{decorator}{argument}` on {on}, through `{package}`",
                module.path,
                decorated.lines.anchor()
            ));
        }
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "Decorators a package provides, each at its lines — where a framework is told what a \
         class or method answers:\n\n{}",
        lines.join("\n")
    ))
}

// What the entry modules — the ones the manifest names, and the ones no
// other module imports — export, with the kind and lines of each.
fn exports(tree: &Tree) -> Option<String> {
    let mut entries = tree.manifest.entries(&tree.resolver);
    for root in surface::roots(tree) {
        push_unique(&mut entries, root);
    }
    let mut lines: Vec<String> = Vec::new();
    for module in entries.iter().filter_map(|path| tree.modules.get(path)) {
        let exported: Vec<String> = module
            .exports
            .iter()
            .filter_map(|export| {
                let kind = match &export.kind {
                    ExportKind::Function => "function",
                    ExportKind::Class { .. } => "class",
                    ExportKind::Value => "value",
                    ExportKind::Type | ExportKind::Unknown => return None,
                };
                Some(format!("`{}` ({kind}) {}", export.name, export.lines))
            })
            .collect();
        if exported.is_empty() {
            continue;
        }
        lines.push(format!("- `{}` exports {}", module.path, exported.join(", ")));
    }
    if lines.is_empty() {
        return None;
    }
    Some(format!(
        "What the entry modules — the ones the manifest names, and the ones no other module \
         imports — export, each with its kind and lines. In a tree with no bootstrap, or under \
         a framework that routes by file, these are where a caller enters:\n\n{}",
        lines.join("\n")
    ))
}

// The files laid into the turn whole, as far as they fit: the manifest, the
// bootstrap, the modules the facts cite — where a function is handed to a
// package, where a package's decorator marks a method, the entry modules
// whose exports are listed — then the rest of the tree in path order, so a
// tree past the budget lays what locates its surfaces before what does not.
fn laid(tree: &Tree, root: &str, bootstrap: Option<&str>) -> Vec<String> {
    let mut lay: Vec<String> = Vec::new();
    if Path::new(root).join("package.json").is_file() {
        lay.push("package.json".to_owned());
    }
    if let Some(entry) = bootstrap {
        push_unique(&mut lay, entry.to_owned());
    }
    for module in tree.modules.values().filter(|module| locates(tree, module)) {
        push_unique(&mut lay, module.path.clone());
    }
    for entry in tree.manifest.entries(&tree.resolver) {
        push_unique(&mut lay, entry);
    }
    for root in surface::roots(tree) {
        push_unique(&mut lay, root);
    }
    for path in tree.modules.keys() {
        push_unique(&mut lay, path.clone());
    }
    lay
}

// What the tree alone can hold the model's answer to: no surface under the
// bootstrap's stem, and every module the facts locate a surface in — or the
// manifest names — reached by a named surface or the bootstrap, or listed as
// unreached. A module the facts say nothing of is not asked after: a
// framework may load it by directory, and a model made to account for every
// such module invents surfaces to cover them.
fn check(
    tree: &Tree, bootstrap: Option<&str>, mounts: &BTreeMap<String, String>, answer: &Inventory,
) -> Vec<String> {
    let mut findings: Vec<String> = Vec::new();
    if bootstrap.is_some() {
        for named in answer.surfaces.iter().filter(|named| named.stem == "start") {
            findings.push(format!(
                "- surface `{}`: `start` is the bootstrap's stem, which the caller names itself; \
                 give the surface the stem of what a caller does through it",
                named.name
            ));
        }
    }

    // what the named surfaces reach, and what the bootstrap reaches beside them
    let surfaces = build(tree, answer, mounts);
    let mut covered: Vec<String> =
        unique(surfaces.iter().flat_map(|surface| surface.closure.iter().cloned()));
    if let Some(module) = bootstrap.and_then(|entry| tree.modules.get(entry)) {
        let start = surface::start(tree, module, &surfaces);
        for path in start.closure {
            push_unique(&mut covered, path);
        }
    }
    let unreached: Vec<&str> =
        answer.unreached.iter().map(|path| path.trim_start_matches("./")).collect();
    let entries = tree.manifest.entries(&tree.resolver);
    let missing: Vec<String> = tree
        .modules
        .values()
        .filter(|module| {
            !covered.contains(&module.path)
                && !unreached.contains(&module.path.as_str())
                && (entries.contains(&module.path) || locates(tree, module))
        })
        .map(|module| format!("`{}`", module.path))
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
             a job — name that surface, anchored at the registration, so the caller follows its \
             imports to the module; otherwise list {are} under `unreached` — a plugin, a hook, a \
             helper is no surface, and none is invented to cover a module",
            missing.join(", ")
        ));
    }
    findings
}

// The surfaces the answer names, each at the module and lines its anchor
// names, reaching what the code at those lines references and told from the
// others under its stem by what is registered, decorated, or exported there
// — as the parser's own are from a registration. A surface whose anchor the
// tree does not hold is left out; the SDK's gate has refused it already.
fn build(tree: &Tree, answer: &Inventory, mounts: &BTreeMap<String, String>) -> Vec<Surface> {
    answer
        .surfaces
        .iter()
        .filter_map(|named| {
            let anchor = named.anchor().ok()?;
            let module: &Module = tree.modules.get(anchor.path.trim_start_matches("./"))?;
            let lines = match anchor.lines {
                Some((start, end)) => Lines {
                    start: u32::try_from(start).unwrap_or(u32::MAX),
                    end: u32::try_from(end).unwrap_or(u32::MAX),
                },
                None => Lines {
                    start: 1,
                    end: u32::try_from(module.text.lines().count()).unwrap_or(u32::MAX).max(1),
                },
            };
            Some(Surface {
                name: named.name.clone(),
                entry: module.path.clone(),
                stem: named.stem.clone(),
                lines,
                detail: vec![format!("registered or declared {lines}, named by the survey")],
                closure: tree.reaches(module, lines, &[], None),
                discriminator: surface::discriminate(
                    tree,
                    module,
                    lines,
                    &named.name,
                    &named.stem,
                    mounts,
                ),
                methods: Vec::new(),
                ids: Vec::new(),
            })
        })
        .collect()
}
