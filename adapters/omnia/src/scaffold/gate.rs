//! The gate on the requests under one prefix: where the tree holds it, which stem of the slice lays it, and how a handler reaches it.

use std::collections::BTreeSet;
use std::path::Path;

use emery_sdk::Error;

use super::reference::Gate;
use crate::{layout, syntax};

// The gate as one stem of the slice reaches it: the gate read from the
// design, where its function stands, and how this stem's code gets to it.
pub struct Gating {
    pub gate: Gate,
    // The module that holds the gate's function, relative to the root.
    pub file: String,
    pub source: Source,
}

impl Gating {
    // Where the stem at `dir`, whose part module is `module` at `file`,
    // reaches the gate: the one its held module declares, else the one the
    // tree holds, else the one the slice's first stem under the prefix
    // lays, this stem where it is that one.
    pub fn resolve(
        gated: &Gated<'_>, dir: &str, module: &str, file: &str, held: bool,
    ) -> Option<Self> {
        let gate = gated.gate.clone();
        if held {
            return Some(Self {
                gate,
                file: file.to_owned(),
                source: Source::Held,
            });
        }
        if let Some(found) = gated.found {
            let source = if found.shape {
                Source::Imported {
                    path: found.path_from(dir),
                    dependency: found.dependency_from(dir),
                }
            } else {
                Source::Named {
                    path: format!("{}::{}", found.path_from(dir), found.name),
                }
            };
            return Some(Self {
                gate,
                file: found.file.clone(),
                source,
            });
        }
        let first = gated.first?;
        let (file, source) = if first.dir == dir && first.module == module {
            (file.to_owned(), Source::Laid)
        } else {
            let dependency = (first.dir != dir).then(|| dependency(&first.dir));
            let lead = if first.dir == dir { "crate".to_owned() } else { ident(&first.dir) };
            (
                format!("crates/{}/src/{}.rs", first.dir, first.module),
                Source::Imported {
                    path: format!("{lead}::{}", first.module),
                    dependency,
                },
            )
        };
        Some(Self { gate, file, source })
    }

    // Whether the stem's handlers under the prefix take the headers and
    // call the gate: every source but the one the laid code cannot call.
    pub const fn calls(&self) -> bool {
        !matches!(self.source, Source::Named { .. })
    }

    pub const fn lays(&self) -> bool {
        matches!(self.source, Source::Laid)
    }

    // The module another stem imports the gate from, where it imports one.
    pub fn imported(&self) -> Option<&str> {
        match &self.source {
            Source::Imported { path, .. } => Some(path),
            _ => None,
        }
    }

    pub const fn dependency(&self) -> Option<&Dependency> {
        match &self.source {
            Source::Imported {
                dependency: Some(dependency),
                ..
            } => Some(dependency),
            _ => None,
        }
    }
}

// How a stem's handlers reach the gate.
pub enum Source {
    // Laid in this stem's module by this slice.
    Laid,
    // Declared in this module by an earlier slice of the stem.
    Held,
    // Exported by another module, imported from `path`, with the crate's
    // line in the manifest where the module is another crate's.
    Imported { path: String, dependency: Option<Dependency> },
    // A function taking the request's headers that another slice wrote
    // outside the laid shape: named to the turn, called by no laid code.
    Named { path: String },
}

// A stem crate another stem depends on through its manifest.
pub struct Dependency {
    pub package: String,
    pub dir: String,
}

impl Dependency {
    // `<package> = { path = "../<dir>" }`, the stem's line for the crate.
    pub fn line(&self) -> String {
        format!("{} = {{ path = \"../{}\" }}", self.package, self.dir)
    }
}

// A `pub fn` taking the request's headers that a stem crate's module
// already exports.
pub struct Found {
    dir: String,
    module: String,
    pub name: String,
    pub file: String,
    // Whether it is the laid shape: `authenticate` beside a `GateError`.
    pub shape: bool,
}

impl Found {
    // The gate the tree holds: the first `pub fn` taking a `HeaderMap` in a
    // module of a stem crate, in crate and path order, the slice's own part
    // modules passed over since they are this slice's to write, and a file
    // `syn` refuses passed over since the checks name it.
    pub fn scan(
        root: &Path, crates: &BTreeSet<String>, own: &[String],
    ) -> Result<Option<Self>, Error> {
        for dir in crates.iter().filter(|dir| layout::is_stem_crate(dir)) {
            for source in layout::sources(&root.join("crates").join(dir).join("src"))? {
                let Some(module) = source.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                let relative = layout::relative(root, &source);
                if module == "lib" || own.contains(&relative) {
                    continue;
                }
                let text = layout::read(&source)?;
                let Ok(file) = syntax::parse(&text) else { continue };
                let declared = syntax::Declared::of(&file);
                for exported in syntax::exported_fns(&file) {
                    if !syntax::signature(&text, &exported).contains("HeaderMap") {
                        continue;
                    }
                    let shape = exported.name == Gate::FN
                        && declared.types.iter().any(|name| name == Gate::ERROR);
                    return Ok(Some(Self {
                        dir: dir.clone(),
                        module: module.to_owned(),
                        name: exported.name,
                        file: relative,
                        shape,
                    }));
                }
            }
        }
        Ok(None)
    }

    // The module's path from a stem crate `dir`: the crate's own where it
    // stands in it, else through the crate's package.
    fn path_from(&self, dir: &str) -> String {
        let lead = if self.dir == dir { "crate".to_owned() } else { ident(&self.dir) };
        format!("{lead}::{}", self.module)
    }

    fn dependency_from(&self, dir: &str) -> Option<Dependency> {
        (self.dir != dir).then(|| dependency(&self.dir))
    }
}

// The crate identifier of the stem a crate directory holds.
fn ident(dir: &str) -> String {
    layout::package(stem_of(dir)).replace('-', "_")
}

fn stem_of(dir: &str) -> &str {
    if dir == layout::crate_dir("common") { "common" } else { dir }
}

fn dependency(dir: &str) -> Dependency {
    Dependency {
        package: layout::package(stem_of(dir)),
        dir: dir.to_owned(),
    }
}

// What a stem resolves its gate against: the gate read, the one the tree
// holds, and the stem of this slice that lays it where the tree holds none.
pub struct Gated<'g> {
    pub gate: &'g Gate,
    pub found: Option<&'g Found>,
    pub first: Option<&'g Laying>,
}

// The stem of the slice that lays the gate: the first with a route under
// the prefix.
pub struct Laying {
    pub module: String,
    pub dir: String,
}
