//! Reads a `Cargo.toml` line by line for the keys the layout's rules ask of it.

// What a manifest spells: each table header, and each key beneath one, at
// its line, with the comments dropped. A value is read as its line spells
// it, since every entry the layout writes stands on one line.
pub struct Toml {
    tables: Vec<(usize, String)>,
    keys: Vec<Key>,
}

impl Toml {
    pub fn read(text: &str) -> Self {
        let mut toml = Self {
            tables: Vec::new(),
            keys: Vec::new(),
        };
        let mut table = String::new();
        for (index, line) in text.lines().enumerate() {
            let line = uncommented(line).trim();
            if line.starts_with('[') {
                line.trim_start_matches('[').trim_end_matches(']').trim().clone_into(&mut table);
                toml.tables.push((index + 1, table.clone()));
            } else if let Some((name, value)) = line.split_once('=') {
                toml.keys.push(Key {
                    line: index + 1,
                    table: table.clone(),
                    name: name.trim().to_owned(),
                    value: value.trim().to_owned(),
                });
            }
        }
        toml
    }

    // The crates `[dependencies]` names, each at the first line naming it:
    // a dotted key (`uuid.workspace = true`), an inline table
    // (`uuid = { .. }`), or a `[dependencies.uuid]` table of its own.
    pub fn dependencies(&self) -> Vec<Dependency> {
        let tables = self.tables.iter().filter_map(|(line, table)| {
            table.strip_prefix("dependencies.").map(|name| (*line, unquoted(name).to_owned()))
        });
        let keys = self.keys.iter().filter(|key| key.table == "dependencies").map(|key| {
            let head = key.name.split('.').next().unwrap_or_default();
            (key.line, unquoted(head).to_owned())
        });
        let mut named: Vec<(usize, String)> =
            tables.chain(keys).filter(|(_, package)| !package.is_empty()).collect();
        named.sort_by_key(|(line, _)| *line);
        let mut found: Vec<Dependency> = Vec::new();
        for (line, package) in named {
            if !found.iter().any(|seen| seen.package == package) {
                found.push(Dependency { line, package });
            }
        }
        found
    }

    // Every `<package> = { path = "crates/<dir>" }` entry, wherever it
    // stands, since the template keeps one by its shape alone.
    pub fn path_entries(&self) -> Vec<PathEntry> {
        self.keys
            .iter()
            .filter_map(|key| {
                let dir = key.value.strip_prefix("{ path = \"crates/")?.strip_suffix("\" }")?;
                (is_stem(&key.name) && is_stem(dir)).then(|| PathEntry {
                    line: key.line,
                    package: key.name.clone(),
                    dir: dir.to_owned(),
                })
            })
            .collect()
    }

    // Every `<package> = { path = "../<dir>" }` entry of a stem manifest's
    // `[dependencies]`, a path dependency on another stem's crate.
    pub fn sibling_entries(&self) -> Vec<PathEntry> {
        self.keys
            .iter()
            .filter(|key| key.table == "dependencies")
            .filter_map(|key| {
                let dir = key.value.strip_prefix("{ path = \"../")?.strip_suffix("\" }")?;
                (is_stem(&key.name) && is_stem(dir)).then(|| PathEntry {
                    line: key.line,
                    package: key.name.clone(),
                    dir: dir.to_owned(),
                })
            })
            .collect()
    }

    // Whether `[workspace.dependencies]` pins a crate named `name`.
    pub fn pins(&self, name: &str) -> bool {
        self.keys.iter().any(|key| key.table == "workspace.dependencies" && key.name == name)
    }

    // Each non-blank entry inside `[dependencies]`, with the crate it names.
    pub fn dependency_entries(text: &str) -> Vec<DependencyEntry> {
        let mut entries = Vec::new();
        let mut in_dependencies = false;
        for (index, line) in text.lines().enumerate() {
            let trimmed = uncommented(line).trim();
            if trimmed.starts_with('[') {
                let table = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
                in_dependencies = table == "dependencies";
            } else if in_dependencies && !trimmed.is_empty() {
                let key = trimmed.split_once('=').map_or(trimmed, |(key, _)| key);
                let head = key.split('.').next().unwrap_or(key);
                entries.push(DependencyEntry {
                    line: index + 1,
                    package: unquoted(head).to_owned(),
                    entry: trimmed.to_owned(),
                });
            }
        }
        entries
    }

    // The lines of a stem manifest outside `[dependencies]`: every line
    // through `[dependencies]` itself, plus any line from the next `[table]`
    // header onward.
    pub fn lines_outside_dependencies(text: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut in_dependencies = false;
        for line in text.lines() {
            let trimmed = uncommented(line).trim();
            if trimmed.starts_with('[') {
                let table = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
                in_dependencies = table == "dependencies";
                out.push(line);
            } else if !in_dependencies {
                out.push(line);
            }
        }
        out
    }
}

struct Key {
    line: usize,
    table: String,
    name: String,
    value: String,
}

// A crate `[dependencies]` names, at the first line naming it.
pub struct Dependency {
    pub line: usize,
    pub package: String,
}

// One non-blank line inside `[dependencies]`, with the crate it names.
pub struct DependencyEntry {
    pub line: usize,
    pub package: String,
    pub entry: String,
}

// A `<package> = { path = "crates/<dir>" }` entry of the root manifest, or a
// `<package> = { path = "../<dir>" }` entry of a stem's.
pub struct PathEntry {
    pub line: usize,
    pub package: String,
    pub dir: String,
}

// `line` up to a `#` outside its quotes; a `\"` in a basic string closes
// none, while a literal string escapes nothing.
fn uncommented(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (at, ch) in line.char_indices() {
        match quote {
            Some(_) if escaped => escaped = false,
            Some('"') if ch == '\\' => escaped = true,
            Some(open) if ch == open => quote = None,
            None if matches!(ch, '"' | '\'') => quote = Some(ch),
            None if ch == '#' => return &line[..at],
            _ => {}
        }
    }
    line
}

fn unquoted(key: &str) -> &str {
    key.trim().trim_matches(|ch| ch == '"' || ch == '\'')
}

// The shape of a stem's name in a manifest, as the template keeps an entry
// by.
fn is_stem(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}
