//! Finds the items and manifest lines outside reached or vanished crates that
//! mention one of those crates.

use std::collections::BTreeSet;

use syn::{Item, UseTree};

use crate::layout::{self, Toml};
use crate::syntax::{self, Mentions};
use crate::tree::Tree;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TouchPoint {
    pub file: String,
    pub item: String,
    pub line: usize,
}

pub struct TouchPoints;

impl TouchPoints {
    pub fn read(tree: &Tree<'_>, stems: &[String]) -> Vec<TouchPoint> {
        if stems.is_empty() {
            return Vec::new();
        }
        let dirs: BTreeSet<String> =
            stems.iter().map(|stem| layout::crate_dir(stem).to_owned()).collect();
        let packages: BTreeSet<String> = stems.iter().map(|stem| layout::package(stem)).collect();
        let idents: BTreeSet<String> = packages.iter().map(|pkg| pkg.replace('-', "_")).collect();

        let mut points = Vec::new();
        for (dir, manifest) in tree.stem_manifests() {
            if dirs.contains(dir) {
                continue;
            }
            let file = format!("crates/{dir}/Cargo.toml");
            for (line, name) in Toml::read(manifest).dependencies() {
                if packages.contains(name) || dirs.contains(name) {
                    points.push(TouchPoint {
                        file: file.clone(),
                        item: format!("dependency {name}"),
                        line,
                    });
                }
            }
        }

        for module in tree.stem_modules() {
            if module.crate_dir().is_some_and(|dir| dirs.contains(dir)) {
                continue;
            }
            let Some(file) = module.file() else { continue };
            let imported = imported_symbols(file, &idents);
            for item in &file.items {
                let Some((line, head)) = item_touch(item, &idents, &imported) else { continue };
                points.push(TouchPoint {
                    file: module.path.clone(),
                    item: head,
                    line,
                });
            }
        }
        points
    }
}

fn imported_symbols(file: &syn::File, idents: &BTreeSet<String>) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    for item in &file.items {
        if let Item::Use(item) = item {
            collect_imported(&item.tree, false, idents, &mut symbols);
        }
    }
    symbols
}

fn collect_imported(
    tree: &UseTree, under_target: bool, idents: &BTreeSet<String>, symbols: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            if under_target || idents.contains(&path.ident.to_string()) {
                collect_imported(&path.tree, true, idents, symbols);
            }
        }
        UseTree::Name(name) if under_target => {
            symbols.insert(name.ident.to_string());
        }
        UseTree::Rename(rename) if under_target => {
            symbols.insert(rename.rename.to_string());
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_imported(item, under_target, idents, symbols);
            }
        }
        _ => {}
    }
}

fn item_touch(
    item: &Item, idents: &BTreeSet<String>, imported: &BTreeSet<String>,
) -> Option<(usize, String)> {
    let mentions = Mentions::of_item(item);
    let touches = idents.iter().any(|ident| mentions.path_leads(ident))
        || (!matches!(item, Item::Use(_))
            && imported.iter().any(|symbol| mentions.contains(symbol)));
    if !touches {
        return None;
    }
    match item {
        Item::Use(item) => Some((syntax::line(item.use_token.span), "use".to_owned())),
        _ => syntax::item_head(item),
    }
}
