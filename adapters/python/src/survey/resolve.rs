//! Resolves Python module imports and manifest script entries.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::survey::code::{self, Imported};
use emery_sdk::survey::resolve::{Target, normalize};
use emery_sdk::unique;

use super::parse::Module;

#[derive(Debug)]
pub struct Resolver {
    modules: BTreeSet<String>,
    tests: BTreeSet<String>,
    data: BTreeSet<String>,
    roots: Vec<String>,
    owned: BTreeSet<String>,
}

impl Resolver {
    pub fn new(parsed: &code::Parsed<Module>, owned: Option<&str>) -> Self {
        let modules: BTreeSet<String> = parsed.modules.keys().cloned().collect();
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
            let file = path.strip_prefix("src/").unwrap_or(path);
            if !file.contains('/')
                && let Some(stem) = file.strip_suffix(".py")
            {
                owned.insert(stem.to_owned());
            }
        }

        Self {
            modules,
            tests: parsed.tests.iter().cloned().collect(),
            data: parsed.data.iter().cloned().collect(),
            roots,
            owned,
        }
    }

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
        let code::Module { imports, dynamic, .. } = &mut **module;
        for load in dynamic {
            load.scope = load.specifier.as_deref().and_then(|spelled| self.scope(imports, spelled));
        }
    }

    // An import of a namespace package binds no module and returns `None`.
    fn resolve(&self, from: &str, specifier: &str, name: Option<&str>) -> Option<Target> {
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
            return self
                .module_at(&base, name)
                .map(Target::Module)
                .or_else(|| self.test_at(&base, name).map(Target::Skipped))
                .or_else(unresolved);
        }

        let path = specifier.replace('.', "/");
        for root in &self.roots {
            if let Some(found) = self.module_at(&join(root, &path), name) {
                return Some(Target::Module(found));
            }
        }
        for root in &self.roots {
            if let Some(found) = self.test_at(&join(root, &path), name) {
                return Some(Target::Skipped(found));
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

    fn module_at(&self, base: &str, name: Option<&str>) -> Option<String> {
        if let Some(name) = name
            && let Some(found) = file_or_init(&self.modules, &join(base, name))
        {
            return Some(found);
        }
        file_or_init(&self.modules, base)
    }

    fn test_at(&self, base: &str, name: Option<&str>) -> Option<String> {
        if let Some(name) = name
            && let Some(found) = file_or_init(&self.tests, &join(base, name))
        {
            return Some(found);
        }
        file_or_init(&self.tests, base)
    }

    fn scope(&self, imports: &[code::Import], spelled: &str) -> Option<String> {
        let path = spelled.replace('.', "/");
        if let Some(base) =
            self.roots.iter().map(|root| join(root, &path)).find(|base| self.holds_package(base))
        {
            return Some(base);
        }
        let target = imports.iter().find(|import| import.local == spelled)?.target.as_ref()?;
        target.module()?.strip_suffix("/__init__.py").map(str::to_owned)
    }

    fn holds_package(&self, base: &str) -> bool {
        if base.is_empty() {
            return false;
        }
        let prefix = format!("{base}/");
        self.modules.iter().any(|path| path.starts_with(&prefix))
    }
}

fn file_or_init(among: &BTreeSet<String>, base: &str) -> Option<String> {
    if base.is_empty() {
        return None;
    }
    [format!("{base}.py"), format!("{base}/__init__.py")]
        .into_iter()
        .find(|path| among.contains(path))
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
    pub scripts: BTreeMap<String, String>,
}

impl Manifest {
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

    pub fn summary(&self, resolver: &Resolver) -> code::Manifest {
        let mut says: Vec<String> = Vec::new();
        if let Some(name) = &self.name {
            says.push(format!("names the package `{name}`"));
        }
        if !self.scripts.is_empty() {
            let scripts: Vec<String> = self
                .scripts
                .iter()
                .map(|(name, target)| format!("`{name}` runs `{target}`"))
                .collect();
            says.push(format!("installs the console scripts {}", scripts.join(", ")));
        }
        code::Manifest {
            name: self.name.clone(),
            says,
            entries: self.targets(resolver).into_iter().map(|(module, _)| module).collect(),
        }
    }

    pub fn targets(&self, resolver: &Resolver) -> Vec<(String, Option<String>)> {
        unique(self.scripts.values().filter_map(|target| {
            let module = resolver.entry(target)?;
            let function = target.split_once(':').map(|(_, function)| function.trim().to_owned());
            Some((module, function))
        }))
    }
}

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
        let value = if let Some(rest) = value.strip_prefix('"') {
            rest.split('"').next().unwrap_or(rest).to_owned()
        } else if let Some(rest) = value.strip_prefix('\'') {
            rest.split('\'').next().unwrap_or(rest).to_owned()
        } else if value.starts_with(['[', '{']) {
            last_key = None;
            continue;
        } else {
            value.split('#').next().unwrap_or(value).trim_end().to_owned()
        };
        if key.is_empty() {
            continue;
        }
        sections.entry(section.clone()).or_default().insert(key.to_owned(), value);
        last_key = Some(key.to_owned());
    }
    sections
}
