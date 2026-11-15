//! Holds a change build's turn to its touch points outside the slice's crates, and the tree to the crates a change removed.

use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use emery_sdk::{Error, server_error};
use omnia_template::Manifest;
use syn::Item;

use super::Tree;
use crate::change::TouchPoint;
use crate::layout::{self, Toml};
use crate::scaffold::Scaffold;
use crate::syntax::{self, DeclaredItem};

impl Tree {
    // Every file outside the slice's crates stands as it did before the
    // turn, item by item as `syn` reads it, but for the touch points the
    // brief listed. A `use` of a file holding a touch point is admitted
    // too, since the edit there may move an import. A file the turn added
    // is `strays`'s.
    pub fn touched_beyond(&self, scaffold: &Scaffold) -> Result<Option<String>, Error> {
        let Some(prior_files) = scaffold.prior_files() else {
            return Ok(None);
        };
        let mut lines = Vec::new();
        for (path, prior) in prior_files {
            let after = match fs::read(self.root.join(path)) {
                Ok(after) => after,
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    lines.push(format!(
                        "{path}: removed outside the slice's crates; it stood as:\n```\n{}```",
                        String::from_utf8_lossy(&prior.bytes)
                    ));
                    continue;
                }
                Err(err) => return Err(server_error!("reading `{path}`: {err}")),
            };
            if after == prior.bytes {
                continue;
            }
            let (before, after) =
                (String::from_utf8_lossy(&prior.bytes), String::from_utf8_lossy(&after));
            let file = Path::new(path);
            if file.extension().is_some_and(|ext| ext == "rs") {
                lines.extend(changed_items(path, &before, &after, &prior.touch_points));
            } else if file.file_name().is_some_and(|name| name == "Cargo.toml") {
                lines.extend(changed_dependencies(path, &before, &after, &prior.touch_points));
            } else {
                lines.push(format!("{path}: changed, and it is no touch point"));
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "a change build rewrites the slice's own crates and leaves every other crate as it \
             stood, but for the touch points the brief lists: put each item named below back as \
             it stood before the turn through `edit_files`, and write a removed file back whole \
             through `write_files`:\n{}",
            lines.join("\n")
        )))
    }

    // No module names a crate the change removed, and no stem manifest
    // depends on one: the vanished stems the change names, and, at a verify
    // too, every crate a path entry names that the tree lacks.
    pub fn dangling_mentions(
        &self, vanished_packages: &BTreeSet<String>,
    ) -> Result<Option<String>, Error> {
        let mut packages = vanished_packages.clone();
        packages.extend(self.absent_packages()?);
        if packages.is_empty() {
            return Ok(None);
        }
        let removed: Vec<RemovedCrate> =
            packages.iter().map(String::as_str).map(RemovedCrate::new).collect();

        let mut lines = Vec::new();
        for module in self.files() {
            let Some(file) = module.file() else { continue };
            for target in &removed {
                for declared in syntax::items_mentioning(file, |ident| ident == target.ident) {
                    lines.push(format!(
                        "{}:{}: `{}` names `{}`, whose crate the tree no longer holds",
                        module.path, declared.line, declared.head, target.package
                    ));
                }
            }
        }
        for (dir, manifest) in self.stem_manifests() {
            for dep in Toml::read(manifest).dependencies() {
                if packages.contains(&dep.package) {
                    lines.push(format!(
                        "crates/{dir}/Cargo.toml:{}: `{}` is a dependency on a crate the tree no \
                         longer holds",
                        dep.line, dep.package
                    ));
                }
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "a crate the change removed is named nowhere in the tree: drop each mention below, or \
             reach what the slice puts in its place, through `edit_files`, and drop each \
             dependency line on it:\n{}",
            lines.join("\n")
        )))
    }

    // The packages a path entry of the root manifest or of a stem's names
    // whose crate the tree lacks.
    fn absent_packages(&self) -> Result<BTreeSet<String>, Error> {
        let held = |dir: &str| self.manifests.contains_key(dir);
        let mut absent = BTreeSet::new();
        let file = self.root.join(Manifest::PATH);
        if file.is_file() {
            let text = layout::read(&file)?;
            absent.extend(
                Toml::read(&text)
                    .path_entries()
                    .into_iter()
                    .filter(|entry| !held(&entry.dir))
                    .map(|entry| entry.package),
            );
        }
        for (_, manifest) in self.stem_manifests() {
            absent.extend(
                Toml::read(manifest)
                    .sibling_entries()
                    .into_iter()
                    .filter(|entry| !held(&entry.dir))
                    .map(|entry| entry.package),
            );
        }
        Ok(absent)
    }
}

struct RemovedCrate {
    package: String,
    ident: String,
}

impl RemovedCrate {
    fn new(package: &str) -> Self {
        Self {
            package: package.to_owned(),
            ident: package.replace('-', "_"),
        }
    }
}

// The items of `after` no item of `before` equals, and those of `before`
// none of `after` equals, each named unless a touch point admits it. A file
// `syn` refuses after the turn is `unparsed`'s.
fn changed_items(path: &str, before: &str, after: &str, points: &[TouchPoint]) -> Vec<String> {
    let Ok(after) = syntax::parse(after) else { return Vec::new() };
    let Ok(before) = syntax::parse(before) else {
        return vec![format!("{path}: changed, and it is no touch point")];
    };

    let mut prior: Vec<&Item> = before.items.iter().collect();
    let mut after_only = Vec::new();
    for item in &after.items {
        if let Some(at) = prior.iter().position(|old| *old == item) {
            prior.remove(at);
        } else {
            after_only.push(syntax::declared_item(item));
        }
    }
    let before_only: Vec<DeclaredItem> = prior.into_iter().map(syntax::declared_item).collect();

    let mut lines = Vec::new();
    if before.attrs != after.attrs {
        lines.push(format!("{path}:1: the module's inner attributes changed"));
    }
    for item in &after_only {
        if TouchPoint::admits_item(points, &item.head) {
            continue;
        }
        let what =
            if before_only.iter().any(|old| old.head == item.head) { "changed" } else { "added" };
        lines.push(format!(
            "{path}:{}: `{}` {what}, and it is no touch point",
            item.line, item.head
        ));
    }
    for item in &before_only {
        if TouchPoint::admits_item(points, &item.head)
            || after_only.iter().any(|new| new.head == item.head)
        {
            continue;
        }
        lines.push(format!(
            "{path}: `{}` removed, and it is no touch point; it stood at line {} before the turn",
            item.head, item.line
        ));
    }
    lines
}

// The `[dependencies]` lines of a stem manifest the turn added or removed,
// each named unless it is a touch point's dependency. A line outside
// `[dependencies]` is `stem_manifest_drift`'s.
fn changed_dependencies(
    path: &str, before: &str, after: &str, points: &[TouchPoint],
) -> Vec<String> {
    let before = Toml::dependency_entries(before);
    let after = Toml::dependency_entries(after);
    let mut lines = Vec::new();
    for dep in &after {
        if !before.iter().any(|old| old.entry == dep.entry)
            && !TouchPoint::admits_dependency(points, &dep.package)
        {
            lines.push(format!(
                "{path}:{}: `{}` added, and it is no touch point",
                dep.line, dep.entry
            ));
        }
    }
    for dep in &before {
        if !after.iter().any(|new| new.entry == dep.entry)
            && !TouchPoint::admits_dependency(points, &dep.package)
        {
            lines.push(format!(
                "{path}: `{}` removed, and it is no touch point; it stood at line {} before the \
                 turn",
                dep.entry, dep.line
            ));
        }
    }
    lines
}
