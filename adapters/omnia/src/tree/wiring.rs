//! Holds the route and topic tables, the two router lists, the two root test
//! rungs, the configuration catalog, and every stem's handlers to the guest's
//! wiring.

use std::collections::BTreeSet;

use emery_sdk::Error;

use super::{
    CATALOG, Module, Tree, is_ident, is_ident_char, pub_consts, read, sources, spells, template,
    uncommented,
};

// A table of `common` with the list and the rung its constants are spelled
// into, in full under `prefix`.
struct Table {
    path: &'static str,
    prefix: &'static str,
    list: &'static str,
    rung: &'static str,
    entries: &'static str,
}

const HTTP: Table = Table {
    path: "crates/common/src/routes/http.rs",
    prefix: "routes::http::",
    list: "src/routes.rs",
    rung: "tests/routes.rs",
    entries: "ROUTES",
};

const TOPIC: Table = Table {
    path: "crates/common/src/routes/topic.rs",
    prefix: "routes::topic::",
    list: "src/messaging.rs",
    rung: "tests/messaging.rs",
    entries: "TOPICS",
};

impl Tree<'_> {
    // Names every constant of the two tables its lists disagree with, and
    // every list statement naming no constant: a route's constant stands in
    // the route list and the route rung; a consumed topic's in the topic list
    // and the messaging rung; a topic a module alone names is published and
    // stands in neither.
    pub(super) fn unwired_tables(&self) -> Result<Option<String>, Error> {
        let mut lines = Vec::new();

        // the path table: every constant in the route list and the route rung
        let routes = self.code(HTTP.list)?;
        let rung = self.code(HTTP.rung)?;
        for (number, name) in self.constants(HTTP.path)? {
            let qualified = format!("{}{name}", HTTP.prefix);
            if !spells(&routes, &qualified) {
                lines.push(format!(
                    "{}:{number}: route `{name}` is named by no statement of `{}`",
                    HTTP.path, HTTP.list
                ));
            }
            if !spells(&rung, &qualified) {
                lines.push(format!(
                    "{}:{number}: route `{name}` is named by no entry of `{}` in `{}`",
                    HTTP.path, HTTP.entries, HTTP.rung
                ));
            }
        }

        // the topic table: a consumed constant in the topic list and the
        // messaging rung, a published one in a stem's module and neither
        let topics = self.code(TOPIC.list)?;
        let rung = self.code(TOPIC.rung)?;
        for (number, name) in self.constants(TOPIC.path)? {
            let qualified = format!("{}{name}", TOPIC.prefix);
            let finding = match (spells(&topics, &qualified), spells(&rung, &qualified)) {
                (true, true) => continue,
                (true, false) => format!(
                    "topic `{name}` is consumed by `{}` and named by no entry of `{}` in `{}`",
                    TOPIC.list, TOPIC.entries, TOPIC.rung
                ),
                (false, true) => format!(
                    "topic `{name}` is named by an entry of `{}` in `{}` and consumed by no \
                     statement of `{}`",
                    TOPIC.entries, TOPIC.rung, TOPIC.list
                ),
                (false, false)
                    if self
                        .modules
                        .iter()
                        .any(|module| !module.is_common() && spells(&module.code, &name)) =>
                {
                    continue;
                }
                (false, false) => format!(
                    "topic `{name}` is consumed by no statement of `{}` and published by no module",
                    TOPIC.list
                ),
            };
            lines.push(format!("{}:{number}: {finding}", TOPIC.path));
        }

        // every statement of a list over a constant of its table
        for (table, text) in [(&HTTP, &routes), (&TOPIC, &topics)] {
            for (at, statement) in calls(text, ".route(") {
                if !statement.contains(table.prefix) {
                    let number = text[..at].matches('\n').count() + 1;
                    lines.push(format!(
                        "{}:{number}: a statement names no constant of `{}`",
                        table.list, table.path
                    ));
                }
            }
        }

        if lines.is_empty() {
            return Ok(None);
        }
        lines.sort();
        Ok(Some(format!(
            "a table and its lists disagree: a route's constant is named by one statement of \
             `src/routes.rs` and one entry of `ROUTES` in `tests/routes.rs`, a consumed topic's \
             by one statement of `src/messaging.rs` and one entry of `TOPICS` in \
             `tests/messaging.rs`, and a topic only published by the module that publishes it and \
             neither list; add the missing line at the end of its list through `edit_files`, \
             remove an entry nothing consumes, and rewrite no other line:\n{}",
            lines.join("\n")
        )))
    }

    // Names every handler under a stem's `src/` — a `pub async fn` whose
    // signature takes a `Context` — that no `.route(..)` statement of the two
    // lists reaches: handed to the route function or consumer the statement
    // calls, or to `handle_with` by a route function of a stem the statement
    // calls with no argument.
    pub(super) fn unlisted_handlers(&self) -> Result<Option<String>, Error> {
        let lists = [self.code(HTTP.list)?, self.code(TOPIC.list)?];

        // what the lists hand, then what a route function they call hands
        let mut reached: BTreeSet<String> = lists
            .iter()
            .flat_map(|text| calls(text, ".route("))
            .filter_map(|(_, statement)| handed(statement).map(str::to_owned))
            .collect();
        for module in &self.modules {
            for (route, handler) in module.route_fns() {
                if reached.contains(&route) {
                    reached.insert(handler);
                }
            }
        }

        let mut lines = Vec::new();
        for module in &self.modules {
            for (number, name) in module.handlers() {
                let full = format!("{}::{name}", module.name);
                if !reached.contains(&full) {
                    lines.push(format!(
                        "{}:{number}: handler `{name}` is reached by no list statement",
                        module.path
                    ));
                }
            }
        }
        if lines.is_empty() {
            return Ok(None);
        }
        lines.sort();
        Ok(Some(format!(
            "a handler is reached by no `.route(..)` statement of `src/routes.rs` or \
             `src/messaging.rs`, so the host never runs it; list it under the route or topic its \
             requirement states, or, where it is a helper and no handler, hand it the provider \
             rather than the `Context`:\n{}",
            lines.join("\n")
        )))
    }

    // Names every `pub const` of the catalog beyond the template's own that no
    // module outside `common` and no root source spells, each
    // `<path>:<line>: <what>`: a key nothing reads is a setting the deployment
    // supplies and the program never consults.
    pub(super) fn unread_settings(&self) -> Result<Option<String>, Error> {
        let declared = self.constants(CATALOG)?;
        if declared.is_empty() {
            return Ok(None);
        }
        let layout: BTreeSet<&str> = pub_consts(template(CATALOG)?).map(|(_, name)| name).collect();
        let mut root_sources = Vec::new();
        for file in sources(&self.root.join("src"))? {
            root_sources.push(uncommented(&read(&file)?));
        }
        let mut lines = Vec::new();
        for (number, name) in declared {
            let read_by_module = self
                .modules
                .iter()
                .any(|module| !module.is_common() && spells(&module.code, &name));
            let read_by_root = root_sources.iter().any(|text| spells(text, &name));
            if layout.contains(name.as_str()) || read_by_module || read_by_root {
                continue;
            }
            lines.push(format!("{CATALOG}:{number}: setting `{name}` is read by no module"));
        }
        if lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(format!(
            "a setting of the catalog is read by no module, so a value the deployment supplies \
             under it changes nothing: a key is declared by the slice whose handler reads it \
             through `Config::get` where the behaviour runs, so read each setting named where \
             the requirement that states it is implemented, or, where the setting configured the \
             source's broker, vault, or agent alone and the translation lays nothing for it, \
             remove its constant and its doc comment from the catalog through \
             `edit_files`:\n{}",
            lines.join("\n")
        )))
    }
}

impl Module {
    // Every route function `self.code` declares with the handler it hands
    // `handle_with` as the call's second argument, each qualified as
    // `(<fn>, <handler>)`.
    fn route_fns(&self) -> Vec<(String, String)> {
        calls(&self.code, "handle_with(")
            .into_iter()
            .filter_map(|(at, inner)| {
                let route = enclosing_pub_fn(&self.code[..at])?;
                let handler = qualify(&self.name, arguments(inner).get(1)?)?;
                Some((format!("{}::{route}", self.name), handler))
            })
            .collect()
    }

    // Every handler `self.code` declares, each its 1-based line and name: a
    // `pub async fn` whose signature, read to its body, takes a `Context<`.
    fn handlers(&self) -> Vec<(usize, &str)> {
        let mut found = Vec::new();
        let mut offset = 0;
        for (index, raw) in self.code.split_inclusive('\n').enumerate() {
            if let Some(rest) = raw.trim_start().strip_prefix("pub async fn ") {
                let name = rest.split(|ch: char| !is_ident_char(ch)).next().unwrap_or_default();
                let head = &self.code[offset..];
                let signature = head.find('{').map_or(head, |body| &head[..body]);
                if !name.is_empty() && signature.contains("Context<") {
                    found.push((index + 1, name));
                }
            }
            offset += raw.len();
        }
        found
    }
}

// Every call of `callee` in `text` — the spelling with its opening
// parenthesis, `.route(` — as the offset it starts at and its argument text,
// parentheses balanced, so a call spread over lines is one.
fn calls<'t>(text: &'t str, callee: &str) -> Vec<(usize, &'t str)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(callee) {
        let start = from + at;
        let open = start + callee.len();
        let mut depth = 1usize;
        let mut end = None;
        for (offset, ch) in text[open..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else { break };
        found.push((start, &text[open..end]));
        from = end;
    }
    found
}

// The path a `.route(..)` statement hands, read from the call its second
// argument makes: the call's first argument where it has one,
// `get(<handler>)`, `consume(<handler>)`, `consume_with(<handler>,
// <decode>)`; the callee itself where it has none, a route function of a
// stem, `orders::placing::place_order_route()`. A statement in another
// shape hands none.
fn handed(statement: &str) -> Option<&str> {
    let call = arguments(statement).get(1)?.trim();
    let open = call.find('(')?;
    let close = call.rfind(')')?;
    let inner = call.get(open + 1..close)?;
    match arguments(inner).first().map(|argument| argument.trim()) {
        Some("") | None => qualified(&call[..open]),
        Some(handler) => qualified(handler),
    }
}

// The name of the last `pub fn` declared in `head`, the one a call there
// sits in.
fn enclosing_pub_fn(head: &str) -> Option<&str> {
    head.lines().rev().find_map(|line| {
        let rest = line.trim_start().strip_prefix("pub fn ")?;
        let name = rest.split(|ch: char| !is_ident_char(ch)).next()?;
        (!name.is_empty()).then_some(name)
    })
}

// `text` split at the commas no parenthesis, bracket, or brace encloses.
fn arguments(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (at, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&text[start..at]);
                start = at + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

// A `::`-separated path of two or more identifiers,
// `orders::placing::place_order`; a text in another shape is none.
fn qualified(path: &str) -> Option<&str> {
    let path = path.trim();
    let mut segments = path.split("::");
    let first = segments.next()?;
    let second = segments.next()?;
    (is_ident(first) && is_ident(second) && segments.all(is_ident)).then_some(path)
}

// `path` qualified under `module` (`orders::placing`): a bare identifier
// sits in `module`, a `crate::` path in its stem, and a qualified path
// stands as it is.
fn qualify(module: &str, path: &str) -> Option<String> {
    let path = path.trim();
    if is_ident(path) {
        return Some(format!("{module}::{path}"));
    }
    if let Some(rest) = path.strip_prefix("crate::") {
        let stem = module.split("::").next()?;
        let full = format!("{stem}::{rest}");
        return qualified(&full).map(str::to_owned);
    }
    qualified(path).map(str::to_owned)
}
