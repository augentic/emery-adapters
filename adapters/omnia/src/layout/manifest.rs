//! Reads a `Cargo.toml` line by line for the keys the layout's rules ask of it.

// What a manifest spells: each table header, and each key beneath one, at
// its line, with the comments dropped. A value is read as its line spells
// it, since every entry the layout writes stands on one line.
pub struct Toml<'a> {
    tables: Vec<(usize, &'a str)>,
    keys: Vec<Key<'a>>,
}

// A `name = value` line: its table, empty at the top level, its name as
// written, and its value.
struct Key<'a> {
    line: usize,
    table: &'a str,
    name: &'a str,
    value: &'a str,
}

// A `<stem> = { path = "crates/<dir>" }` entry of the root manifest.
pub struct PathEntry<'a> {
    pub line: usize,
    pub stem: &'a str,
    pub dir: &'a str,
}

impl<'a> Toml<'a> {
    pub fn read(text: &'a str) -> Self {
        let mut toml = Self {
            tables: Vec::new(),
            keys: Vec::new(),
        };
        let mut table = "";
        for (index, line) in text.lines().enumerate() {
            let line = uncommented(line).trim();
            if line.starts_with('[') {
                table = line.trim_start_matches('[').trim_end_matches(']').trim();
                toml.tables.push((index + 1, table));
            } else if let Some((name, value)) = line.split_once('=') {
                toml.keys.push(Key {
                    line: index + 1,
                    table,
                    name: name.trim(),
                    value: value.trim(),
                });
            }
        }
        toml
    }

    // The crates `[dependencies]` names, each at the first line naming it:
    // a dotted key (`uuid.workspace = true`), an inline table
    // (`uuid = { .. }`), or a `[dependencies.uuid]` table of its own.
    pub fn dependencies(&self) -> Vec<(usize, &'a str)> {
        let tables = self.tables.iter().filter_map(|(line, table)| {
            table.strip_prefix("dependencies.").map(|name| (*line, unquoted(name)))
        });
        let keys = self
            .keys
            .iter()
            .filter(|key| key.table == "dependencies")
            .map(|key| (key.line, unquoted(key.name.split('.').next().unwrap_or_default())));
        let mut named: Vec<(usize, &'a str)> =
            tables.chain(keys).filter(|(_, name)| !name.is_empty()).collect();
        named.sort_by_key(|(line, _)| *line);
        let mut found: Vec<(usize, &'a str)> = Vec::new();
        for (line, name) in named {
            if !found.iter().any(|(_, seen)| *seen == name) {
                found.push((line, name));
            }
        }
        found
    }

    // Every path entry, wherever it stands, since the template keeps one by
    // its shape alone.
    pub fn path_entries(&self) -> impl Iterator<Item = PathEntry<'a>> + '_ {
        self.keys.iter().filter_map(|key| {
            let dir = key.value.strip_prefix("{ path = \"crates/")?.strip_suffix("\" }")?;
            (is_stem(key.name) && is_stem(dir)).then_some(PathEntry {
                line: key.line,
                stem: key.name,
                dir,
            })
        })
    }

    // Whether the manifest inherits the workspace's lints, as `[lints]`
    // holding `workspace = true` or a top-level `lints.workspace = true`.
    pub fn inherits_workspace_lints(&self) -> bool {
        self.keys.iter().any(|key| {
            key.value == "true"
                && matches!((key.table, key.name), ("lints", "workspace") | ("", "lints.workspace"))
        })
    }

    // Whether `[workspace.dependencies]` pins a crate named `name`.
    pub fn pins(&self, name: &str) -> bool {
        self.keys.iter().any(|key| key.table == "workspace.dependencies" && key.name == name)
    }
}

// `line` up to a `#` outside its quotes.
fn uncommented(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (at, ch) in line.char_indices() {
        match quote {
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
