//! Locates modules: the manifest's scripts and where each import leads.
//!
//! An absolute import is probed as the interpreter probes it, beneath each
//! root the tree lays its packages under: the root itself, `src/`, and the
//! directory above every top-level package. A relative import climbs from
//! the importing module's directory by its dots. A string spelling a dotted
//! path (`"orders.urls"`, `"app.main:serve"`) is probed as an absolute
//! import is; one spelling a data file's name (`"config.toml"`) names the
//! file alone.
//!
//! What the tree does not answer is a package, third-party and standard
//! library alike, unless its first segment is a package of the tree's own.
//! That import is `Target::Unresolved`, never dropped, so the survey can say
//! what it could not follow and widen what it lays. Imports are settled
//! once, after the tree is read, and carried on the module.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::survey::code::Imported;
use emery_sdk::survey::resolve::{Target, normalize};

use super::parse::Module;
use super::unique;

#[derive(Debug)]
pub struct Resolver {
    modules: BTreeSet<String>,
    data: BTreeSet<String>,
    // Root-relative, `""` for the root itself.
    roots: Vec<String>,
    // The first segment of every absolute import the tree should answer:
    // its top-level packages and modules, and the manifest's name.
    owned: BTreeSet<String>,
}

impl Resolver {
    // A top-level package is one whose parent directory is no package.
    // `owned` is the manifest's name, when it has one.
    pub fn new(
        modules: impl IntoIterator<Item = String>, data: impl IntoIterator<Item = String>,
        owned: Option<&str>,
    ) -> Self {
        let modules: BTreeSet<String> = modules.into_iter().collect();
        let packages: BTreeSet<&str> =
            modules.iter().filter_map(|path| path.strip_suffix("/__init__.py")).collect();
        let mut roots = vec![String::new(), "src".to_owned()];
        let mut owned: BTreeSet<String> =
            owned.map(|name| name.replace('-', "_")).into_iter().collect();

        // the roots and the owned names, from the top-level packages
        for package in &packages {
            let (parent, name) = package.rsplit_once('/').unwrap_or(("", package));
            if packages.contains(parent) {
                continue;
            }
            if !roots.iter().any(|root| root == parent) {
                roots.push(parent.to_owned());
            }
            if parent.is_empty() || parent == "src" {
                owned.insert(name.to_owned());
            }
        }

        // the owned names from the top-level modules
        for path in &modules {
            let under = |root: &str| {
                if root.is_empty() {
                    (!path.contains('/')).then_some(path.as_str())
                } else {
                    path.strip_prefix(&format!("{root}/")).filter(|rest| !rest.contains('/'))
                }
            };
            if let Some(file) = under("").or_else(|| under("src"))
                && let Some(stem) = file.strip_suffix(".py")
            {
                owned.insert(stem.to_owned());
            }
        }

        Self {
            modules,
            data: data.into_iter().collect(),
            roots,
            owned,
        }
    }

    // A literal import is kept only where the tree answers it.
    pub fn settle(&self, module: &mut Module) {
        let from = module.path.clone();
        for import in &mut module.imports {
            import.target = match &import.imported {
                Imported::Effect => self.literal(&from, &import.specifier),
                Imported::Named(name) => self.resolve(&from, &import.specifier, Some(name)),
                Imported::Default | Imported::Whole | Imported::Star => {
                    self.resolve(&from, &import.specifier, None)
                }
            };
        }
        module
            .imports
            .retain(|import| import.imported != Imported::Effect || import.target.is_some());
        for reexport in &mut module.reexports {
            reexport.target = self.resolve(&from, &reexport.specifier, None);
        }
    }

    // `name` is what a `from` import takes. An import of a namespace package
    // binds no module and is `None`.
    pub fn resolve(&self, from: &str, specifier: &str, name: Option<&str>) -> Option<Target> {
        let unresolved = || Some(Target::Unresolved(specifier.to_owned()));
        let dots = specifier.len() - specifier.trim_start_matches('.').len();
        if dots > 0 {
            let rest = &specifier[dots..];
            let mut dir = from.rsplit_once('/').map_or("", |(dir, _)| dir);
            for _ in 1..dots {
                if dir.is_empty() {
                    return unresolved();
                }
                dir = dir.rsplit_once('/').map_or("", |(parent, _)| parent);
            }
            let base = join(dir, &rest.replace('.', "/"));
            return self.module_at(&base, name).map(Target::Module).or_else(unresolved);
        }

        let path = specifier.replace('.', "/");
        for root in &self.roots {
            if let Some(found) = self.module_at(&join(root, &path), name) {
                return Some(Target::Module(found));
            }
        }
        let first = specifier.split('.').next().unwrap_or(specifier);
        if self.owned.contains(first) {
            if self.roots.iter().any(|root| self.holds_package(&join(root, &path))) {
                return None;
            }
            return unresolved();
        }
        Some(Target::Package(first.to_owned()))
    }

    // What a string spells, probed in order:
    // - a module at the dotted path whole, then less its last segment where
    //   that names an attribute (`orders.apps.OrdersConfig`); never where
    //   the last segment is a data file's extension, which spells a file
    // - a data file at the path from the module's directory, then from the
    //   root
    // - a bare file name joined onto a path the code computes
    //   (`Path(__file__).parent / "data" / "rates.json"`), as the one data
    //   file of that name in the tree
    fn literal(&self, from: &str, value: &str) -> Option<Target> {
        if let Some((path, last)) = value.rsplit_once('.')
            && !value.contains('/')
            && !super::DATA_EXTENSIONS.contains(&last)
        {
            let whole = value.replace('.', "/");
            let shorter = path.replace('.', "/");
            for root in &self.roots {
                if let Some(found) = self.module_at(&join(root, &whole), None) {
                    return Some(Target::Module(found));
                }
            }
            for root in &self.roots {
                if let Some(found) = self.module_at(&join(root, &shorter), None) {
                    return Some(Target::Module(found));
                }
            }
        }
        let dir = from.rsplit_once('/').map_or("", |(dir, _)| dir);
        [normalize(dir, value), normalize("", value)]
            .into_iter()
            .flatten()
            .find(|path| self.data.contains(path))
            .or_else(|| {
                if value.contains('/') {
                    return None;
                }
                let mut named =
                    self.data.iter().filter(|path| path.rsplit('/').next() == Some(value));
                let found = named.next()?;
                named.next().is_none().then(|| found.clone())
            })
            .map(Target::Data)
    }

    pub fn entry(&self, dotted: &str) -> Option<String> {
        let dotted = dotted.split_once(':').map_or(dotted, |(module, _)| module).trim();
        let path = dotted.replace('.', "/");
        self.roots.iter().find_map(|root| self.module_at(&join(root, &path), None))
    }

    // A candidate led by `*/` matches any top-level package: `*/__main__.py`
    // is the first `<package>/__main__.py` beneath a root.
    pub fn first(&self, candidates: &[&str]) -> Option<String> {
        candidates.iter().find_map(|candidate| {
            let Some(rest) = candidate.strip_prefix("*/") else {
                return self.modules.contains(*candidate).then(|| (*candidate).to_owned());
            };
            self.roots.iter().find_map(|root| {
                let prefix = if root.is_empty() { String::new() } else { format!("{root}/") };
                self.modules
                    .iter()
                    .find(|path| {
                        path.strip_prefix(&prefix).is_some_and(|beneath| {
                            beneath.split_once('/').is_some_and(|(package, file)| {
                                file == rest
                                    && self
                                        .modules
                                        .contains(&format!("{prefix}{package}/__init__.py"))
                            })
                        })
                    })
                    .cloned()
            })
        })
    }

    // `name` is probed as a submodule first, as the interpreter probes it.
    fn module_at(&self, base: &str, name: Option<&str>) -> Option<String> {
        if let Some(name) = name
            && let Some(found) = self.file_or_init(&join(base, name))
        {
            return Some(found);
        }
        self.file_or_init(base)
    }

    fn file_or_init(&self, base: &str) -> Option<String> {
        if base.is_empty() {
            return None;
        }
        [format!("{base}.py"), format!("{base}/__init__.py")]
            .into_iter()
            .find(|path| self.modules.contains(path))
    }

    // A namespace package: a directory of modules with no `__init__`.
    fn holds_package(&self, base: &str) -> bool {
        !base.is_empty() && self.modules.iter().any(|path| path.starts_with(&format!("{base}/")))
    }
}

fn join(dir: &str, path: &str) -> String {
    match (dir.is_empty(), path.is_empty()) {
        (true, _) => path.to_owned(),
        (false, true) => dir.to_owned(),
        (false, false) => format!("{dir}/{path}"),
    }
}

#[derive(Debug, Default)]
pub struct Manifest {
    pub name: Option<String>,
    // By script name, each the `module:function` it runs.
    pub scripts: BTreeMap<String, String>,
}

impl Manifest {
    // `pyproject.toml` first, then `setup.cfg` for what it leaves unsaid. An
    // absent or unreadable manifest is empty.
    pub fn read(root: &Path) -> Self {
        let mut manifest = Self::default();
        if let Ok(text) = std::fs::read_to_string(root.join("pyproject.toml")) {
            let sections = sections(&text);
            manifest.name = ["project", "tool.poetry"]
                .iter()
                .find_map(|section| sections.get(*section).and_then(|keys| keys.get("name")))
                .cloned();
            for section in ["project.scripts", "project.gui-scripts", "tool.poetry.scripts"] {
                if let Some(scripts) = sections.get(section) {
                    for (name, target) in scripts {
                        manifest.scripts.entry(name.clone()).or_insert_with(|| target.clone());
                    }
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string(root.join("setup.cfg")) {
            let sections = sections(&text);
            if manifest.name.is_none() {
                manifest.name = sections.get("metadata").and_then(|keys| keys.get("name")).cloned();
            }
            if let Some(console) =
                sections.get("options.entry_points").and_then(|keys| keys.get("console_scripts"))
            {
                for line in console.lines() {
                    if let Some((name, target)) = line.split_once('=') {
                        manifest
                            .scripts
                            .entry(name.trim().to_owned())
                            .or_insert_with(|| target.trim().to_owned());
                    }
                }
            }
        }
        manifest
    }

    // In script order, once each, with the function each target names.
    pub fn entries(&self, resolver: &Resolver) -> Vec<(String, Option<String>)> {
        unique(self.scripts.values().filter_map(|target| {
            let module = resolver.entry(target)?;
            let function = target.split_once(':').map(|(_, function)| function.trim().to_owned());
            Some((module, function))
        }))
    }
}

// A hand reader for the string keys of TOML and INI sections. A quoted
// string is unquoted, a bare INI value kept as written, and indented
// continuation lines joined with newlines. Arrays and tables are skipped,
// so a manifest it misreads is at worst a manifest it does not read.
fn sections(text: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut last_key: Option<String> = None;
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with(['#', ';']) {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[')
            && let Some(name) = rest.split_once(']').map(|(name, _)| name.trim())
        {
            current = Some(name.trim_matches('[').trim_matches(']').trim().to_owned());
            last_key = None;
            continue;
        }
        let Some(section) = &current else { continue };
        if line.starts_with([' ', '\t']) {
            if let Some(key) = &last_key
                && let Some(value) = sections.get_mut(section).and_then(|keys| keys.get_mut(key))
            {
                let continued = line.trim();
                if !continued.is_empty() {
                    if !value.is_empty() {
                        value.push('\n');
                    }
                    value.push_str(continued);
                }
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=').or_else(|| line.split_once(':')) else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        let value = value.trim();
        let value = match value.strip_prefix(['"', '\'']) {
            Some(rest) => rest.split(['"', '\'']).next().unwrap_or(rest).to_owned(),
            None if value.starts_with(['[', '{']) => {
                last_key = None;
                continue;
            }
            None => value.to_owned(),
        };
        if key.is_empty() {
            continue;
        }
        sections.entry(section.clone()).or_default().insert(key.to_owned(), value);
        last_key = Some(key.to_owned());
    }
    sections
}
