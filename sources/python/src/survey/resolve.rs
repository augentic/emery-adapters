//! Locates modules: the manifest's scripts and where each import leads.
//!
//! An absolute import is probed the way the interpreter probes it — beneath
//! each root the tree lays its packages under: the root itself, `src/`, and
//! the directory above every top-level package — as a module, a package's
//! `__init__`, or a submodule the imported name spells. A relative import
//! climbs from the importing module's directory by its dots. A dotted path a
//! string spells (`"orders.urls"`, `"app.main:serve"`) is probed as an
//! absolute import is. Anything the tree does not answer is a package —
//! third-party and standard library alike — unless its first segment is a
//! package of the tree's own, when it is `Unresolved`, never dropped, so the
//! survey can say what it could not follow and widen what it lays. Each
//! module's imports are settled once the tree is read and carried on the
//! module, so nothing resolves twice.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::parse::{Imported, Module};
use super::unique;

/// What an import specifier names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    /// A module of the tree, by its root-relative path.
    Module(String),
    /// A package outside the tree, by its top-level name: `fastapi` for
    /// `fastapi.routing`, `os` for `os.path`.
    Package(String),
    /// A data file of the tree — a `.json`, `.yaml`, `.toml`, `.csv`, or
    /// `.ini` a string names by path — by its root-relative path.
    Data(String),
    /// A relative import naming nothing, or an absolute one beneath a
    /// package of the tree's own whose module is absent — a file the keep
    /// left out, a module not yet written, a typo — by the specifier as
    /// written.
    Unresolved(String),
}

impl Target {
    /// The module of the tree the target names, if it names one.
    pub fn module(&self) -> Option<&str> {
        match self {
            Self::Module(path) => Some(path),
            Self::Package(_) | Self::Data(_) | Self::Unresolved(_) => None,
        }
    }
}

/// The tree's modules, its data files, and the roots its packages lie
/// beneath.
#[derive(Debug)]
pub struct Resolver {
    modules: BTreeSet<String>,
    data: BTreeSet<String>,
    // root-relative, `""` for the root itself
    roots: Vec<String>,
    // the first segment of every absolute import the tree should answer:
    // its top-level packages and modules, and the manifest's name
    owned: BTreeSet<String>,
}

impl Resolver {
    /// Indexes `modules` and the `data` files a string may name, and reads
    /// the roots the tree lays its packages under: the root, `src/`, and
    /// the directory above every top-level package — one with an
    /// `__init__.py` whose own directory has none. `owned` is the
    /// manifest's name, when it has one.
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

    /// Settles where every import and re-export of `module` leads. A
    /// string that spells a module path or a data file is kept only when
    /// the tree answers it.
    pub fn settle(&self, module: &mut Module) {
        let from = module.path.clone();
        for import in &mut module.imports {
            import.target = match &import.imported {
                Imported::Literal => self.literal(&from, &import.specifier),
                Imported::Named(name) => self.resolve(&from, &import.specifier, Some(name)),
                Imported::Module | Imported::Star => self.resolve(&from, &import.specifier, None),
            };
        }
        module
            .imports
            .retain(|import| import.imported != Imported::Literal || import.target.is_some());
        for reexport in &mut module.reexports {
            reexport.target = self.resolve(&from, &reexport.specifier, None);
        }
    }

    /// Resolves `specifier` as imported from the module at `from`: a
    /// relative one from that module's directory, an absolute one beneath
    /// each root. `name` is the name a `from` import takes, probed first as
    /// a submodule. A relative specifier the tree does not answer, or an
    /// absolute one under a package of the tree's own, is
    /// [`Target::Unresolved`]; any other absolute one is a package.
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

    // The module a string spells: the dotted path whole, or less its last
    // segment where that names an attribute (`orders.apps.OrdersConfig`);
    // else the data file it names, from the module's directory or the root,
    // or — a bare file name joined onto a path the code computes
    // (`Path(__file__).parent / "data" / "rates.json"`) — the one data file
    // of that name in the tree.
    fn literal(&self, from: &str, value: &str) -> Option<Target> {
        if let Some((path, _)) = value.rsplit_once('.')
            && !value.contains('/')
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

    /// The module a dotted path names — `ledger.cli` as a manifest script
    /// spells it, `app.main` from `app.main:serve` — beneath any root.
    pub fn entry(&self, dotted: &str) -> Option<String> {
        let dotted = dotted.split_once(':').map_or(dotted, |(module, _)| module).trim();
        let path = dotted.replace('.', "/");
        self.roots.iter().find_map(|root| self.module_at(&join(root, &path), None))
    }

    /// The first of `candidates` that is a module of the tree. A candidate
    /// led by `*/` stands for any top-level package's: `*/__main__.py` is
    /// the first `<package>/__main__.py` beneath a root.
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

    // The module at `base` — `a/b` for `a.b` — as the interpreter finds it:
    // `name` as a submodule beneath it first, then the module file, then the
    // package's `__init__`.
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

    // Whether `base` is a directory holding modules: a namespace package,
    // which an import names without naming a module.
    fn holds_package(&self, base: &str) -> bool {
        !base.is_empty() && self.modules.iter().any(|path| path.starts_with(&format!("{base}/")))
    }
}

// `path` beneath `dir`, with no leading slash for the root.
fn join(dir: &str, path: &str) -> String {
    match (dir.is_empty(), path.is_empty()) {
        (true, _) => path.to_owned(),
        (false, true) => dir.to_owned(),
        (false, false) => format!("{dir}/{path}"),
    }
}

// `path` joined beneath `dir`, `.` and `..` segments folded; `None` when it
// climbs above the root.
pub(super) fn normalize(dir: &str, path: &str) -> Option<String> {
    let mut segments: Vec<&str> = Vec::new();
    for segment in dir.split('/').chain(path.split('/')) {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    Some(segments.join("/"))
}

/// What `pyproject.toml` or `setup.cfg` says about the package and the
/// scripts it installs.
#[derive(Debug, Default)]
pub struct Manifest {
    pub name: Option<String>,
    /// Each console script by name, with the `module:function` it runs.
    pub scripts: BTreeMap<String, String>,
}

impl Manifest {
    /// Reads `pyproject.toml` under `root` — `[project]` and `[tool.poetry]`
    /// — then `setup.cfg` for what it leaves unsaid; absent or unreadable
    /// ones are empty.
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

    /// Every module the manifest's scripts run, resolved against the tree,
    /// in script order, once each — with the function each names, for the
    /// bootstrap's note.
    pub fn entries(&self, resolver: &Resolver) -> Vec<(String, Option<String>)> {
        unique(self.scripts.values().filter_map(|target| {
            let module = resolver.entry(target)?;
            let function = target.split_once(':').map(|(_, function)| function.trim().to_owned());
            Some((module, function))
        }))
    }
}

// The `[section]` tables of a TOML or INI text, each key with its string
// value: a quoted TOML string unquoted, a bare INI value as written, a
// value continued on indented lines joined with newlines. Tables, arrays,
// and anything else the hand reader does not spell are skipped, so a
// manifest it misreads is at worst a manifest it does not read.
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
                // an array or inline table, whose continuation lines are
                // nothing a key continues
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
