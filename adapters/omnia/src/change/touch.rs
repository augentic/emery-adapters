//! Finds the items and manifest lines outside reached or vanished crates that
//! mention one of those crates.

use std::collections::BTreeSet;
use std::fmt;

use crate::layout::{self, Toml};
use crate::syntax::{self, ItemHead};
use crate::tree::Tree;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TouchKind {
    Use,
    Item(String),
    Dependency(String),
}

impl fmt::Display for TouchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Use => f.write_str("use"),
            Self::Item(name) => f.write_str(name),
            Self::Dependency(package) => write!(f, "dependency {package}"),
        }
    }
}

impl From<ItemHead> for TouchKind {
    fn from(head: ItemHead) -> Self {
        match head {
            ItemHead::Use => Self::Use,
            ItemHead::Named(name) => Self::Item(name),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TouchPoint {
    pub file: String,
    pub kind: TouchKind,
    pub line: usize,
}

impl TouchPoint {
    pub fn crate_dir(&self) -> Option<&str> {
        self.file.strip_prefix("crates/")?.split('/').next()
    }

    pub fn item(&self) -> Option<&str> {
        match &self.kind {
            TouchKind::Item(name) => Some(name),
            TouchKind::Use | TouchKind::Dependency(_) => None,
        }
    }

    pub fn dependency(&self) -> Option<&str> {
        match &self.kind {
            TouchKind::Dependency(package) => Some(package),
            TouchKind::Use | TouchKind::Item(_) => None,
        }
    }
}

pub struct TouchPoints;

impl TouchPoints {
    pub fn read(tree: &Tree, stems: &[String]) -> Vec<TouchPoint> {
        if stems.is_empty() {
            return Vec::new();
        }
        let dirs: BTreeSet<String> =
            stems.iter().map(|stem| layout::crate_dir(stem).to_owned()).collect();
        let packages: BTreeSet<String> = stems.iter().map(|stem| layout::package(stem)).collect();
        let idents: BTreeSet<String> =
            packages.iter().map(|pkg| layout::crate_ident(pkg)).collect();

        let mut points = Vec::new();
        for (dir, manifest) in tree.stem_manifests() {
            if dirs.contains(dir) {
                continue;
            }
            let file = format!("crates/{dir}/Cargo.toml");
            for dep in Toml::read(manifest).dependencies() {
                if packages.contains(&dep.package) || dirs.contains(&dep.package) {
                    points.push(TouchPoint {
                        file: file.clone(),
                        kind: TouchKind::Dependency(dep.package),
                        line: dep.line,
                    });
                }
            }
        }

        for module in tree.stem_modules() {
            if module.crate_dir().is_some_and(|dir| dirs.contains(dir)) {
                continue;
            }
            let Some(file) = module.file() else { continue };
            for declared in syntax::items_mentioning(file, |ident| idents.contains(ident)) {
                points.push(TouchPoint {
                    file: module.path.clone(),
                    kind: TouchKind::from(declared.head),
                    line: declared.line,
                });
            }
        }
        points
    }
}
