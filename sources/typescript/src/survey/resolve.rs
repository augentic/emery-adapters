//! Locates modules: the manifest's entries and where each import leads.
//!
//! A relative specifier is probed the way the compiler probes it: as
//! written, then with each source extension, then as a directory's `index`.
//! The `baseUrl` and `paths` of `tsconfig.json` resolve the aliases a bare
//! specifier may spell, and a bare specifier no mapping answers is a
//! package. A relative or aliased specifier the tree does not answer is
//! `Unresolved`, never dropped, so the survey can say what it could not
//! follow and widen what it lays. Each module's imports are settled once the
//! tree is read and carried on the module.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use emery_sdk::serde_json::{self, Value};
use emery_sdk::survey::resolve::{Target, normalize};

use super::parse::Module;
use super::{EXTENSIONS, unique};

// The build outputs a manifest may point at in place of their sources.
const OUTPUT_DIRS: &[&str] = &["dist", "build", "lib", "out"];

#[derive(Debug)]
pub struct Resolver {
    modules: BTreeSet<String>,
    data: BTreeSet<String>,
    // Root-relative, `""` for the root itself.
    base_url: Option<String>,
    // Each `paths` pattern with its targets, relative to `base_url` or the
    // root.
    paths: Vec<(String, Vec<String>)>,
}

impl Resolver {
    // One relative `extends` of `tsconfig.json` is followed where it can be
    // read.
    pub fn new(
        modules: impl IntoIterator<Item = String>, data: impl IntoIterator<Item = String>,
        root: &Path,
    ) -> Self {
        let mut resolver = Self {
            modules: modules.into_iter().collect(),
            data: data.into_iter().collect(),
            base_url: None,
            paths: Vec::new(),
        };
        let Some(config) = tsconfig(root, "tsconfig.json", true) else { return resolver };
        let options = config.get("compilerOptions");
        resolver.base_url = options
            .and_then(|o| o.get("baseUrl"))
            .and_then(Value::as_str)
            .and_then(|base| normalize("", base));
        if let Some(paths) = options.and_then(|o| o.get("paths")).and_then(Value::as_object) {
            for (pattern, targets) in paths {
                let targets: Vec<String> = targets
                    .as_array()
                    .map(|targets| {
                        targets.iter().filter_map(Value::as_str).map(str::to_owned).collect()
                    })
                    .unwrap_or_default();
                resolver.paths.push((pattern.clone(), targets));
            }
        }
        resolver
    }

    pub fn settle(&self, module: &mut Module) {
        let from = module.path.clone();
        for import in &mut module.imports {
            import.target = self.resolve(&from, &import.specifier);
        }
        for reexport in &mut module.reexports {
            reexport.target = self.resolve(&from, &reexport.specifier);
        }
    }

    // `None` only for an absolute specifier, which names nothing of the tree.
    // A bare specifier matching a `paths` pattern other than the catch-all
    // `*` is aliased, so one the mapping does not answer is `Unresolved`
    // rather than a package.
    pub fn resolve(&self, from: &str, specifier: &str) -> Option<Target> {
        if specifier.starts_with('/') {
            return None;
        }
        let unresolved = || Some(Target::Unresolved(specifier.to_owned()));
        if matches!(specifier, "." | "..")
            || specifier.starts_with("./")
            || specifier.starts_with("../")
        {
            let dir = from.rsplit_once('/').map_or("", |(dir, _)| dir);
            let Some(candidate) = normalize(dir, specifier) else { return unresolved() };
            return self.file(&candidate).or_else(unresolved);
        }

        let mut aliased = false;
        for (pattern, targets) in &self.paths {
            let Some(rest) = alias(pattern, specifier) else { continue };
            aliased |= pattern != "*";
            for target in targets {
                let candidate = target.replacen('*', rest, 1);
                let base = self.base_url.as_deref().unwrap_or("");
                if let Some(found) = normalize(base, &candidate).and_then(|c| self.file(&c)) {
                    return Some(found);
                }
            }
        }
        if let Some(base) = &self.base_url
            && let Some(found) = normalize(base, specifier).and_then(|c| self.file(&c))
        {
            return Some(found);
        }
        if aliased {
            return unresolved();
        }

        Some(Target::Package(specifier.to_owned()))
    }

    // `src/server.ts` as written, or `dist/server.js` mapped back to its
    // source.
    pub fn entry(&self, hint: &str) -> Option<String> {
        let hint = normalize("", hint.trim())?;
        if let Some(found) = self.probe(&hint) {
            return Some(found);
        }
        let (first, rest) = hint.split_once('/')?;
        if !OUTPUT_DIRS.contains(&first) {
            return None;
        }
        self.probe(&format!("src/{rest}")).or_else(|| self.probe(rest))
    }

    pub fn first(&self, candidates: &[&str]) -> Option<String> {
        candidates.iter().find_map(|candidate| self.probe(candidate))
    }

    // A module first, else a data file as written or with the `.json` a
    // `require` may leave off.
    fn file(&self, candidate: &str) -> Option<Target> {
        if let Some(module) = self.probe(candidate) {
            return Some(Target::Module(module));
        }
        [candidate.to_owned(), format!("{candidate}.json")]
            .into_iter()
            .find(|path| self.data.contains(path))
            .map(Target::Data)
    }

    // As written, with a source extension in place of the one it has or
    // lacks, or as a directory's `index`.
    fn probe(&self, candidate: &str) -> Option<String> {
        if self.modules.contains(candidate) {
            return Some(candidate.to_owned());
        }
        let (dir, file) = candidate.rsplit_once('/').map_or(("", candidate), |(d, f)| (d, f));
        let stem = match file.rsplit_once('.') {
            Some((stem, extension)) if EXTENSIONS.contains(&extension) => stem,
            _ => file,
        };
        let prefix = if dir.is_empty() { String::new() } else { format!("{dir}/") };
        for extension in EXTENSIONS {
            let path = format!("{prefix}{stem}.{extension}");
            if self.modules.contains(&path) {
                return Some(path);
            }
        }
        for extension in EXTENSIONS {
            let path = format!("{candidate}/index.{extension}");
            if self.modules.contains(&path) {
                return Some(path);
            }
        }
        None
    }
}

// What the pattern's `*` stood for, or the empty string for an exact match.
fn alias<'s>(pattern: &str, specifier: &'s str) -> Option<&'s str> {
    match pattern.split_once('*') {
        Some((prefix, suffix)) => {
            specifier.strip_prefix(prefix).and_then(|rest| rest.strip_suffix(suffix))
        }
        None => (pattern == specifier).then_some(""),
    }
}

// Comments and trailing commas removed. When `follow`, the file's own
// `compilerOptions` are laid over the ones a readable relative `extends`
// supplies; the base itself is read without following. Anything unreadable
// is ignored.
fn tsconfig(root: &Path, path: &str, follow: bool) -> Option<Value> {
    let text = std::fs::read_to_string(root.join(path)).ok()?;
    let mut config: Value = serde_json::from_str(&strip_jsonc(&text)).ok()?;
    if follow
        && let Some(base) = config.get("extends").and_then(Value::as_str)
        && (base.starts_with("./") || base.starts_with("../"))
    {
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        let mut base_path = normalize(dir, base)?;
        if Path::new(&base_path).extension().is_none_or(|ext| !ext.eq_ignore_ascii_case("json")) {
            base_path.push_str(".json");
        }
        if let Some(parent) = tsconfig(root, &base_path, false) {
            let mut options = parent
                .get("compilerOptions")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            if let Some(own) = config.get("compilerOptions").and_then(Value::as_object) {
                options.extend(own.clone());
            }
            if let Some(object) = config.as_object_mut() {
                object.insert("compilerOptions".to_owned(), Value::Object(options));
            }
        }
    }
    Some(config)
}

// Removes `//` and `/* */` comments outside strings and the trailing commas
// JSON forbids but `tsconfig.json` allows.
fn strip_jsonc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for next in chars.by_ref() {
                    if last == '*' && next == '/' {
                        break;
                    }
                    last = next;
                }
            }
            ',' => {
                let closes =
                    matches!(chars.clone().find(|next| !next.is_whitespace()), Some('}' | ']'));
                if !closes {
                    out.push(c);
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[derive(Debug, Default)]
pub struct Manifest {
    pub name: Option<String>,
    pub main: Option<String>,
    pub bin: Option<String>,
    pub scripts: BTreeMap<String, String>,
}

impl Manifest {
    // An absent or unreadable manifest is empty.
    pub fn read(root: &Path) -> Self {
        let Some(value) = std::fs::read_to_string(root.join("package.json"))
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            return Self::default();
        };
        let bin = match value.get("bin") {
            Some(Value::String(path)) => Some(path.clone()),
            Some(Value::Object(paths)) => paths.values().find_map(Value::as_str).map(str::to_owned),
            _ => None,
        };
        let scripts = value
            .get("scripts")
            .and_then(Value::as_object)
            .map(|scripts| {
                scripts
                    .iter()
                    .filter_map(|(name, command)| {
                        Some((name.clone(), command.as_str()?.to_owned()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            name: value.get("name").and_then(Value::as_str).map(str::to_owned),
            main: value.get("main").and_then(Value::as_str).map(str::to_owned),
            bin,
            scripts,
        }
    }

    // In manifest order, once each: `main`, `bin`, then the sources the
    // `start` and `dev` scripts run.
    pub fn entries(&self, resolver: &Resolver) -> Vec<String> {
        let scripts = ["start", "dev"].into_iter().flat_map(|script| {
            self.scripts.get(script).into_iter().flat_map(|command| {
                command.split_whitespace().filter(|word| {
                    !word.starts_with('-')
                        && word.rsplit_once('.').is_some_and(|(_, ext)| EXTENSIONS.contains(&ext))
                })
            })
        });
        let hints = self.main.as_deref().into_iter().chain(self.bin.as_deref()).chain(scripts);
        unique(hints.filter_map(|hint| resolver.entry(hint)))
    }
}
