//! Checks that stem routers, topic constants, and stem dependencies are each
//! used by the workspace, that a stem which publishes is reached, and that
//! the root manifest names no crate the tree lacks.

use emery_sdk::Error;
use omnia_template::Manifest;

use super::{
    Module, Tree, crates, is_ident, is_ident_char, is_start_stem, read, sources, spells,
    uncommented,
};

const ROUTES: &str = "src/routes.rs";
const MESSAGING: &str = "src/messaging.rs";
const TOPICS: &str = "crates/common/src/topic.rs";
// The lines every stem's manifest carries whether or not each module reaches
// the crate, so none is a finding.
const LAYOUT_DEPENDENCIES: [&str; 6] =
    ["axum", "common", "omnia-sdk", "serde", "serde_json", "tracing"];

impl Tree<'_> {
    // The template keeps a path entry by its shape alone, so one left behind
    // after its crate went is no drift and compiles nothing.
    pub(super) fn dangling_entries(&self) -> Result<Option<String>, Error> {
        let file = self.root.join(Manifest::PATH);
        if !file.is_file() {
            return Ok(None);
        }
        let manifest = read(&file)?;
        let lines: Vec<String> = manifest
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                let (stem, dir) = path_entry(line)?;
                let held = self.root.join("crates").join(dir).join("Cargo.toml").is_file();
                (!held).then(|| {
                    format!(
                        "{}:{}: `{stem}` names `crates/{dir}`, which the tree lacks",
                        Manifest::PATH,
                        index + 1
                    )
                })
            })
            .collect();
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "the root manifest's path list names a crate the tree holds for each stem, and no \
             other: an entry whose `crates/<dir>/Cargo.toml` is missing wires nothing and stands \
             by its shape alone; remove it and the stem's `<stem>.workspace = true` line through \
             `edit_files`:\n{}",
            lines.join("\n")
        )))
    }

    pub(super) fn unused_dependencies(&self) -> Result<Option<String>, Error> {
        let mut lines = Vec::new();
        for stem in crates(self.root)? {
            if stem == "common" || is_start_stem(&stem) {
                continue;
            }
            let path = format!("crates/{stem}/Cargo.toml");
            let file = self.root.join(&path);
            if !file.is_file() {
                continue;
            }
            let manifest = read(&file)?;
            let code = self.stem_code(&stem)?;
            for (number, name) in dependencies(&manifest) {
                let ident = name.replace('-', "_");
                if LAYOUT_DEPENDENCIES.contains(&name.as_str())
                    || code.iter().any(|text| spells(text, &ident))
                {
                    continue;
                }
                lines.push(format!(
                    "{path}:{number}: dependency `{name}` is used by no file of `crates/{stem}/`"
                ));
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "a stem's manifest names the crates its files use and nothing more, since a \
             dependency no module or test spells is compiled for nothing and hides the use the \
             slice meant to write: use the crate where the slice meant to, or remove its line \
             through `edit_files`:\n{}",
            lines.join("\n")
        )))
    }

    // Every source of the stem the check reads a dependency's use in: its
    // modules, read once with the tree, and its tests, which nothing else
    // reads.
    fn stem_code(&self, stem: &str) -> Result<Vec<String>, Error> {
        let prefix = format!("crates/{stem}/");
        let mut code: Vec<String> = self
            .modules
            .iter()
            .filter(|module| module.path.starts_with(&prefix))
            .map(|module| module.code.clone())
            .collect();
        for file in sources(&self.root.join("crates").join(stem).join("tests"))? {
            code.push(uncommented(&read(&file)?));
        }
        Ok(code)
    }

    pub(super) fn stems_merged(&self) -> Result<Option<String>, Error> {
        let routes = self.code(ROUTES)?;
        let messaging = self.code(MESSAGING)?;
        let mut lines = Vec::new();
        for module in &self.modules {
            let Some(stem) = module.entry_of() else { continue };
            let ident = crate::scaffold::package(stem).replace('-', "_");
            if let Some(number) = module.exposes("router")
                && !spells(&routes, &format!("{ident}::router"))
            {
                lines.push(format!(
                    "{}:{number}: `{ident}::router()` is merged by no statement of `{ROUTES}`",
                    module.path
                ));
            }
            if let Some(number) = module.exposes("topics")
                && !spells(&messaging, &format!("{ident}::topics"))
            {
                lines.push(format!(
                    "{}:{number}: `{ident}::topics(..)` is registered by no statement of \
                     `{MESSAGING}`",
                    module.path
                ));
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        lines.sort();
        Ok(Some(format!(
            "a stem's `router()` and `topics(..)` are reached through the root lists alone, and \
             a stem the lists never merge compiles clean and serves nothing: add the stem's \
             statement at the end of its list through `edit_files`, `let router = \
             router.merge(<stem>::router());` in `{ROUTES}` before `router.with_state(..)` and \
             `let router = <stem>::topics(router, &env);` in `{MESSAGING}` before its \
             trace:\n{}",
            lines.join("\n")
        )))
    }

    pub(super) fn unnamed_topics(&self) -> Result<Option<String>, Error> {
        let mut lines = Vec::new();
        for (number, name) in self.constants(TOPICS)? {
            let named = self
                .modules
                .iter()
                .any(|module| !module.is_common() && spells(&module.code, &name));
            if !named {
                lines.push(format!("{TOPICS}:{number}: topic `{name}` is named by no module"));
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "a topic of the shared table is named by no module, so nothing publishes or \
             consumes it: a constant is added by the slice whose part publishes or consumes the \
             topic, as `common::topic::<NAME>` qualified through `config::topic_for`, so name \
             each topic in the part that publishes or consumes it, or remove its constant and \
             its doc comment through `edit_files`:\n{}",
            lines.join("\n")
        )))
    }

    // A stem is run through its `routes()` and `topics(..)` or through
    // another stem's call, so a publishing part in a stem neither reaches is
    // never run; the root lists name every stem and tell nothing.
    pub(super) fn unreached_publishers(&self) -> Result<Option<String>, Error> {
        let mut lines = Vec::new();
        for stem in crates(self.root)? {
            if stem == "common" || is_start_stem(&stem) {
                continue;
            }
            let prefix = format!("crates/{stem}/");
            let (own, others): (Vec<&Module>, Vec<&Module>) =
                self.modules.iter().partition(|module| module.path.starts_with(&prefix));
            let registers = own
                .iter()
                .any(|module| module.entry_of() == Some(stem.as_str()) && module.registers());
            let ident = crate::scaffold::package(&stem).replace('-', "_");
            let reached =
                others.iter().any(|module| !module.is_common() && reaches(&module.code, &ident));
            if registers || reached {
                continue;
            }
            for module in own {
                if let Some(number) = module.publishes() {
                    lines.push(format!(
                        "{}:{number}: `Publish` in a stem no route or topic reaches",
                        module.path
                    ));
                }
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "the host runs a stem through its `routes()` and `topics(..)` alone, so a part that \
             publishes in a stem no route or topic reaches, and no other stem calls, is never \
             run: the trigger is the inbound stream the design's `## APIs and integrations` \
             lists for the stem — its bullet spells the topic and names the requirement its \
             deliveries set off — never a requirement at the registration, which the host owns; \
             add it through `edit_files`: the topic's constant at the end of `{TOPICS}`, \
             `.route(config::topic_for(env, topic::<NAME>), messaging::consume_with(<handler>, \
             <decode>))` in the part's `topics(..)` with the handler calling the earlier parts' \
             functions, and `<part>::topics,` at the end of the entry's array:\n{}",
            lines.join("\n")
        )))
    }
}

impl Module {
    // The stem whose `src/lib.rs` this module is, `crates/<stem>/src/lib.rs`.
    fn entry_of(&self) -> Option<&str> {
        let stem = self.path.strip_prefix("crates/")?.strip_suffix("/src/lib.rs")?;
        (!stem.is_empty() && !stem.contains('/') && !self.is_common()).then_some(stem)
    }

    // Whether the entry names a part's `routes()` or `topics`, as an array
    // entry or a call; `common`'s own folds are not a part's.
    fn registers(&self) -> bool {
        ["::routes()", "::topics"].into_iter().any(|needle| {
            let mut from = 0;
            while let Some(at) = self.code[from..].find(needle) {
                let start = from + at;
                let end = start + needle.len();
                let ident = self.code[..start].trim_end_matches(is_ident_char);
                let ident = &self.code[ident.len()..start];
                let after = self.code[end..].chars().next().is_none_or(|ch| !is_ident_char(ch));
                if is_ident(ident) && ident != "common" && after {
                    return true;
                }
                from = end;
            }
            false
        })
    }

    fn publishes(&self) -> Option<usize> {
        self.code.lines().position(|line| spells(line, "Publish")).map(|index| index + 1)
    }

    // The line declaring `pub fn <name>`, the `pub` bare: a `pub(crate)` one
    // nothing calls is the compiler's to refuse.
    fn exposes(&self, name: &str) -> Option<usize> {
        self.code
            .lines()
            .position(|line| {
                line.trim_start()
                    .strip_prefix("pub fn ")
                    .and_then(|rest| rest.strip_prefix(name))
                    .is_some_and(|after| !after.starts_with(is_ident_char))
            })
            .map(|index| index + 1)
    }
}

// Whether `code` names an item of the crate `ident`, as `<ident>::<item>`.
fn reaches(code: &str, ident: &str) -> bool {
    let needle = format!("{ident}::");
    let mut from = 0;
    while let Some(at) = code[from..].find(&needle) {
        let start = from + at;
        if code[..start].chars().next_back().is_none_or(|ch| !is_ident_char(ch) && ch != ':') {
            return true;
        }
        from = start + needle.len();
    }
    false
}

// The crates a manifest names under `[dependencies]`, each at the first line
// naming it: a dotted key (`uuid.workspace = true`), an inline table
// (`uuid = { .. }`), or a `[dependencies.uuid]` table of its own.
fn dependencies(manifest: &str) -> Vec<(usize, String)> {
    let mut found: Vec<(usize, String)> = Vec::new();
    let mut in_table = false;
    for (index, line) in manifest.lines().enumerate() {
        let line = line.split('#').next().unwrap_or_default().trim();
        let name = if let Some(header) = line.strip_prefix('[') {
            let header = header.strip_suffix(']').map_or("", str::trim);
            in_table = header == "dependencies";
            let Some(name) = header.strip_prefix("dependencies.") else { continue };
            unquoted(name)
        } else if in_table {
            let Some((key, _)) = line.split_once('=') else { continue };
            unquoted(key.split('.').next().unwrap_or_default())
        } else {
            continue;
        };
        if !name.is_empty() && !found.iter().any(|(_, seen)| seen == name) {
            found.push((index + 1, name.to_owned()));
        }
    }
    found
}

fn unquoted(key: &str) -> &str {
    key.trim().trim_matches(|ch| ch == '"' || ch == '\'')
}

// The stem and the directory of a line shaped `<stem> = { path = "crates/<dir>" }`,
// the shape the template keeps an entry by.
fn path_entry(line: &str) -> Option<(&str, &str)> {
    let (stem, rest) = line.trim().split_once(" = { path = \"crates/")?;
    let dir = rest.strip_suffix("\" }")?;
    (is_stem(stem) && is_stem(dir)).then_some((stem, dir))
}

fn is_stem(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}
