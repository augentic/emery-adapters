//! Checks that stem routers, topic constants, and stem dependencies are each
//! used by the workspace, that a stem which publishes is reached, that a
//! stem's entry declares its modules and holds nothing of its own, and that
//! the root manifest names no crate the tree lacks.

use emery_sdk::Error;
use omnia_template::Manifest;

use super::{Module, Tree};
use crate::layout::{self, Toml};
use crate::syntax;

impl Tree<'_> {
    // The template keeps a path entry by its shape alone, so one left behind
    // after its crate went is no drift and compiles nothing.
    pub fn dangling_entries(&self) -> Result<Option<String>, Error> {
        let file = self.root.join(Manifest::PATH);
        if !file.is_file() {
            return Ok(None);
        }
        let manifest = layout::read(&file)?;
        let lines: Vec<String> = Toml::read(&manifest)
            .path_entries()
            .filter(|entry| !self.root.join("crates").join(entry.dir).join("Cargo.toml").is_file())
            .map(|entry| {
                format!(
                    "{}:{}: `{}` names `crates/{}`, which the tree lacks",
                    Manifest::PATH,
                    entry.line,
                    entry.stem,
                    entry.dir
                )
            })
            .collect();
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "the root manifest's path list names a crate the tree holds for each stem, and no \
             other: an entry whose `crates/<dir>/Cargo.toml` is missing wires nothing and stands \
             by its shape alone; remove it and the stem's `<stem>.workspace = true` line through \
             `edit_files`:\n{}",
            lines.join("\n")
        )))
    }

    pub fn unused_dependencies(&self) -> Option<String> {
        let mut lines = Vec::new();
        for (stem, manifest) in self.stem_manifests() {
            let code: Vec<&Module> = self.stem_files(stem).collect();
            for (number, name) in Toml::read(manifest).dependencies() {
                let ident = name.replace('-', "_");
                if layout::STEM_DEPENDENCIES.contains(&name)
                    || name == layout::PROBE_DEPENDENCY
                    || code.iter().any(|module| module.mentions.contains(&ident))
                {
                    continue;
                }
                lines.push(format!(
                    "crates/{stem}/Cargo.toml:{number}: dependency `{name}` is used by no file of \
                     `crates/{stem}/`"
                ));
            }
        }
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "a stem's manifest names the crates its files use and nothing more, since a \
             dependency no module or test spells is compiled for nothing and hides the use the \
             slice meant to write: use the crate where the slice meant to, or remove its line \
             through `edit_files`:\n{}",
            lines.join("\n")
        ))
    }

    // The stem's sources and tests, where a dependency's use is read.
    fn stem_files<'s>(&'s self, stem: &str) -> impl Iterator<Item = &'s Module> {
        let prefix = format!("crates/{stem}/");
        self.modules
            .iter()
            .chain(&self.tests)
            .filter(move |module| module.path.starts_with(&prefix))
    }

    pub fn unmerged_stems(&self) -> Option<String> {
        let mut lines = Vec::new();
        for module in &self.modules {
            let Some(stem) = module.entry_stem() else { continue };
            let ident = layout::package(stem).replace('-', "_");
            if let Some(number) = module.exposes("router")
                && !self.list_names(layout::ROUTES, &[&ident, "router"])
            {
                lines.push(format!(
                    "{}:{number}: `{ident}::router()` is merged by no statement of `{}`",
                    module.path,
                    layout::ROUTES
                ));
            }
            if let Some(number) = module.exposes("topics")
                && !self.list_names(layout::MESSAGING, &[&ident, "topics"])
            {
                lines.push(format!(
                    "{}:{number}: `{ident}::topics(..)` is registered by no statement of `{}`",
                    module.path,
                    layout::MESSAGING
                ));
            }
        }
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a stem's `router()` and `topics(..)` are reached through the root lists alone, and \
             a stem the lists never merge compiles clean and serves nothing: add the stem's \
             statement at the end of its list through `edit_files`, `let router = \
             router.merge(<stem>::router());` in `{}` before `router.with_state(..)` and \
             `let router = <stem>::topics(router, &env);` in `{}` before its trace:\n{}",
            layout::ROUTES,
            layout::MESSAGING,
            lines.join("\n")
        ))
    }

    // Whether the root list at `path` names a path ending `tail`; a list
    // the tree lacks names nothing.
    fn list_names(&self, path: &str, tail: &[&str]) -> bool {
        self.module(path).is_some_and(|list| list.mentions.path_ends(tail))
    }

    // A module the entry does not declare is compiled by nothing, and an
    // item in an entry that composes parts, beyond its lists, is a part
    // folded into it; a verify is where both are left, the one turn under
    // no scaffold, when the shell moves a file and a `delete` takes one.
    pub fn undeclared_parts(&self) -> Option<String> {
        let mut lines = Vec::new();
        for entry in &self.modules {
            let Some(stem) = entry.entry_stem() else { continue };
            let Some(file) = entry.file() else { continue };
            let declared = syntax::mods(file);
            let prefix = format!("crates/{stem}/src/");
            for module in &self.modules {
                let Some(part) = module.part_under(&prefix) else { continue };
                if !declared.iter().any(|name| name == part) {
                    lines.push(format!(
                        "{}: declared by no `pub mod {part};` line of `{}`",
                        module.path, entry.path
                    ));
                }
            }
            if entry.exposes("router").is_none() && entry.exposes("topics").is_none() {
                continue;
            }
            for (number, head) in entry.own_items() {
                lines.push(format!(
                    "{}:{number}: `{head}` stands in the entry, which composes its parts and holds \
                     nothing of its own",
                    entry.path
                ));
            }
        }
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a stem's entry declares its parts and holds nothing of its own: every \
             `crates/<stem>/src/<part>.rs` is listed by a `pub mod <part>;` line of \
             `crates/<stem>/src/lib.rs`, since a module nothing declares is compiled by nothing \
             and serves nothing, and a handler, a type, or a constant in the entry is a part's, \
             folded in where its file was; add each missing line among the entry's `pub mod` \
             lines through `edit_files`, and put each item back in its part, \
             `crates/<stem>/src/<part>.rs`, declared by its `pub mod` line and named in the \
             entry's arrays, through `write_files` and `edit_files`:\n{}",
            lines.join("\n")
        ))
    }

    pub fn unnamed_topics(&self) -> Option<String> {
        let mut lines = Vec::new();
        for constant in &self.topics().constants {
            let named = self
                .modules
                .iter()
                .any(|module| !module.is_common() && module.mentions.contains(&constant.name));
            if !named {
                lines.push(format!(
                    "{}:{}: topic `{}` is named by no module",
                    layout::TOPICS,
                    constant.line,
                    constant.name
                ));
            }
        }
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "a topic of the shared table is named by no module, so nothing publishes or \
             consumes it: a constant is added by the slice whose part publishes or consumes the \
             topic, as `common::topic::<NAME>` qualified through `config::topic_for`, so name \
             each topic in the part that publishes or consumes it, or remove its constant and \
             its doc comment through `edit_files`:\n{}",
            lines.join("\n")
        ))
    }

    // A stem is run through its `routes()` and `topics(..)` or through
    // another stem's call, so a publishing part in a stem neither reaches is
    // never run; the root lists name every stem and tell nothing.
    pub fn unreached_publishers(&self) -> Option<String> {
        let mut lines = Vec::new();
        for stem in self.stem_crates() {
            let prefix = format!("crates/{stem}/");
            let (own, others): (Vec<&Module>, Vec<&Module>) =
                self.modules.iter().partition(|module| module.path.starts_with(&prefix));
            let registers =
                own.iter().any(|module| module.entry_stem() == Some(stem) && module.registers());
            let ident = layout::package(stem).replace('-', "_");
            let reached = others
                .iter()
                .any(|module| !module.is_common() && module.mentions.path_leads(&ident));
            if registers || reached {
                continue;
            }
            for module in own {
                if let Some(number) = module.publishes() {
                    lines.push(format!(
                        "{}:{number}: `Publish` in a stem no route or topic reaches",
                        module.path
                    ));
                }
            }
        }
        if lines.is_empty() {
            return None;
        }
        Some(format!(
            "the host runs a stem through its `routes()` and `topics(..)` alone, so a part that \
             publishes in a stem no route or topic reaches, and no other stem calls, is never \
             run: the trigger is the inbound stream the design's `## APIs and integrations` \
             lists for the stem — its bullet spells the topic and names the requirement its \
             deliveries set off — never a requirement at the registration, which the host owns; \
             add it through `edit_files`: the topic's constant at the end of `{}`, \
             `.route(config::topic_for(env, topic::<NAME>), messaging::consume_with(<handler>, \
             <decode>))` in the part's `topics(..)` with the handler calling the earlier parts' \
             functions, and `<part>::topics,` at the end of the entry's array:\n{}",
            layout::TOPICS,
            lines.join("\n")
        ))
    }
}

impl Module {
    // The stem whose `src/lib.rs` this module is, `crates/<stem>/src/lib.rs`.
    fn entry_stem(&self) -> Option<&str> {
        let stem = self.path.strip_prefix("crates/")?.strip_suffix("/src/lib.rs")?;
        (!stem.is_empty() && !stem.contains('/') && !self.is_common()).then_some(stem)
    }

    // Whether the entry names a part's `routes()` or `topics`, as an array
    // entry or a call; `common`'s own folds are not a part's.
    fn registers(&self) -> bool {
        self.mentions.paths().any(|path| {
            let [.., head, last] = path else { return false };
            head != "common" && (last == "routes" || last == "topics")
        })
    }

    fn publishes(&self) -> Option<usize> {
        self.mentions.first("Publish")
    }

    fn exposes(&self, name: &str) -> Option<usize> {
        syntax::pub_fn(self.file()?, name)
    }

    // The part this module is where it stands directly under `prefix`, the
    // stem's `src/`, as `<part>.rs`; the entry and a file deeper down are
    // none.
    fn part_under<'m>(&'m self, prefix: &str) -> Option<&'m str> {
        let part = self.path.strip_prefix(prefix)?.strip_suffix(".rs")?;
        (!part.is_empty() && !part.contains('/') && part != "lib").then_some(part)
    }

    // Every item of an entry beyond its lists: a `mod` declaration, a `use`,
    // and the two folds `router` and `topics` are the layout's, and anything
    // else a part's.
    fn own_items(&self) -> Vec<(usize, String)> {
        let Some(file) = self.file() else { return Vec::new() };
        file.items
            .iter()
            .filter(|item| match item {
                syn::Item::Mod(item) => item.content.is_some(),
                syn::Item::Fn(item) => item.sig.ident != "router" && item.sig.ident != "topics",
                _ => true,
            })
            .filter_map(syntax::item_head)
            .collect()
    }
}
