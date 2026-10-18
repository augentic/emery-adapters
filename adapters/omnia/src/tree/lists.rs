//! Checks that stem routers and topic constants are wired into the workspace.

use emery_sdk::Error;

use super::{Module, Tree, is_ident_char, spells};

const ROUTES: &str = "src/routes.rs";
const MESSAGING: &str = "src/messaging.rs";
const TOPICS: &str = "crates/common/src/topic.rs";

impl Tree<'_> {
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
