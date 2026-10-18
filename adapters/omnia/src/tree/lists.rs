//! Checks that stem routers, topic constants, and stem dependencies are each
//! used by the workspace.

use emery_sdk::Error;

use super::{
    Module, Tree, crates, is_ident_char, is_start_stem, read, sources, spells, uncommented,
};

const ROUTES: &str = "src/routes.rs";
const MESSAGING: &str = "src/messaging.rs";
const TOPICS: &str = "crates/common/src/topic.rs";
// The lines every stem's manifest carries whether or not each module reaches
// the crate, so none is a finding.
const LAYOUT_DEPENDENCIES: [&str; 6] =
    ["axum", "common", "omnia-sdk", "serde", "serde_json", "tracing"];

impl Tree<'_> {
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
             slice meant to lay: use the crate where the slice meant to, or remove its line \
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
            let ident = stem.replace('-', "_");
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
}

impl Module {
    // The stem whose `src/lib.rs` this module is, `crates/<stem>/src/lib.rs`.
    fn entry_of(&self) -> Option<&str> {
        let stem = self.path.strip_prefix("crates/")?.strip_suffix("/src/lib.rs")?;
        (!stem.is_empty() && !stem.contains('/') && !self.is_common()).then_some(stem)
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
