//! Checks for empty cache misses on constant `StateStore` keys and single-field `data` response wrappers.

use super::{Module, Tree, is_ident, is_ident_char, spells, strip_vis};

const NOTHING: &[&str] = &[
    "Vec::new()",
    "HashMap::new()",
    "BTreeMap::new()",
    "HashSet::new()",
    "BTreeSet::new()",
    "vec![]",
    "Default::default()",
];

const DEFAULTED: &[&str] = &[
    ".unwrap_or_default()",
    ".unwrap_or_else(Vec::new)",
    ".unwrap_or_else(HashMap::new)",
    ".unwrap_or_else(Default::default)",
    ".unwrap_or(Vec::new())",
];

const MISS_WINDOW: usize = 6;

impl Tree<'_> {
    pub(super) fn empty_misses(&self) -> Option<String> {
        let mut lines: Vec<String> = self.modules.iter().flat_map(Module::empty_misses).collect();
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        lines.dedup();
        Some(format!(
            "a table read from the store under one key answers empty on a miss, so the table is \
             never loaded: on `Ok(None)` the function fetches the table from its upstream, as the \
             design spells the call, stores it under the same key with a TTL, and answers what it \
             fetched, never an empty collection; repair each read named through `edit_files`, \
             laying the fetch in the same function:\n{}",
            lines.join("\n")
        ))
    }

    pub(super) fn response_wrappers(&self) -> Option<String> {
        let mut lines = Vec::new();
        for module in &self.modules {
            if module.is_common() {
                continue;
            }
            for (number, name) in module.response_wrappers() {
                lines.push(format!(
                    "{}:{number}: response wrapper struct `{name}` wraps a single `data` field",
                    module.path
                ));
            }
        }
        if lines.is_empty() {
            return None;
        }
        lines.sort();
        Some(format!(
            "a struct wraps an HTTP response in a single `data` field, which is the source's \
             HTTP client property (`response.data`) and never a field on the wire: remove each \
             wrapper struct named through `edit_files` and deserialize `response.body()` \
             directly into the body's own shape, a top-level `Vec<T>` for a JSON array or a \
             struct with the body's fields at its root:\n{}",
            lines.join("\n")
        ))
    }
}

impl Module {
    fn empty_misses(&self) -> Vec<String> {
        let all: Vec<&str> = self.code.lines().collect();
        let mut lines = Vec::new();
        for (index, line) in all.iter().enumerate() {
            if !constant_keyed(line) {
                continue;
            }
            let end = function_end(&all, index);
            for at in answering(&all, index, end) {
                if let Some((spelling, what)) = empty(all[at]) {
                    lines.push(format!(
                        "{}:{}: {what} (`{spelling}`) after the read at line {}",
                        self.path,
                        at + 1,
                        index + 1
                    ));
                }
            }
        }
        lines
    }

    fn response_wrappers(&self) -> Vec<(usize, &str)> {
        let lines: Vec<&str> = self.code.lines().collect();
        let mut found = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let Some(name) = opens_struct(line) else { continue };
            if !derives_deserialize(&lines, index) {
                continue;
            }
            let Some(end) = lines[index + 1..]
                .iter()
                .position(|line| line.trim_start().starts_with('}'))
                .map(|offset| index + 1 + offset)
            else {
                continue;
            };
            let fields: Vec<&str> =
                lines[index + 1..end].iter().filter_map(|line| field_name(line)).collect();
            if fields == ["data"] {
                found.push((index + 1, name));
            }
        }
        found
    }
}

fn empty(line: &str) -> Option<(String, &'static str)> {
    let text = line.trim();
    let answer = match text.split_once("=>") {
        Some((pattern, body)) => spells(pattern, "None").then(|| body.trim().trim_end_matches(',')),
        None => Some(text),
    };
    let nothing = NOTHING.iter().find_map(|nothing| {
        let returned = format!("return Ok({nothing})");
        if line.contains(&returned) {
            return Some(returned);
        }
        let answered = answer?;
        let bare = answered.strip_prefix("Ok(").and_then(|rest| rest.strip_suffix(')'));
        (answered == *nothing || bare == Some(*nothing))
            .then(|| text.trim_end_matches(',').to_owned())
    });
    nothing.map(|spelling| (spelling, "a miss answered as no entries")).or_else(|| {
        DEFAULTED
            .iter()
            .find(|defaulted| line.contains(*defaulted))
            .map(|defaulted| ((*defaulted).to_owned(), "a miss defaulted"))
    })
}

// Finds the closing `}` at the same indentation as the enclosing `fn`.
fn function_end(lines: &[&str], index: usize) -> usize {
    let Some(head) = lines[..=index].iter().rposition(|line| opens_fn(line)) else {
        return lines.len().min(index + MISS_WINDOW);
    };
    let indent = lines[head].len() - lines[head].trim_start().len();
    lines[index + 1..]
        .iter()
        .position(|line| line.len() - line.trim_start().len() == indent && line.trim() == "}")
        .map_or(lines.len(), |offset| index + 1 + offset)
}

fn opens_fn(line: &str) -> bool {
    let mut rest = strip_vis(line);
    for qualifier in ["const ", "async ", "unsafe "] {
        rest = rest.strip_prefix(qualifier).unwrap_or(rest);
    }
    rest.starts_with("fn ")
}

// Collects lines after the read that handle a miss, skipping `Some(..)` hit branches.
fn answering(lines: &[&str], index: usize, end: usize) -> Vec<usize> {
    let indent = |line: &str| line.len() - line.trim_start().len();
    let mut answering = Vec::new();
    let mut hit: Option<usize> = None;
    let mut let_else: Option<usize> = None;
    for (at, line) in lines.iter().enumerate().take(end).skip(index) {
        let text = line.trim_start();
        if text.is_empty() {
            continue;
        }
        if let Some(head) = hit {
            if indent(line) > head || text == "{" {
                continue;
            }
            hit = None;
            if text.starts_with('}') {
                continue;
            }
        }
        if let_else.is_some_and(|head| indent(line) == head && text.starts_with('}')) {
            break;
        }
        let arm =
            (text.starts_with("Some(") || text.starts_with("Ok(Some(")) && text.contains("=>");
        if arm || text.starts_with("if let Some(") {
            hit = Some(indent(line));
            continue;
        }
        answering.push(at);
        if text.starts_with("let Some(") {
            if text.ends_with("};") {
                break;
            }
            let_else = Some(indent(line));
        }
    }
    answering
}

fn constant_keyed(line: &str) -> bool {
    let Some(at) = line.find("StateStore::get(") else { return false };
    let args = &line[at + "StateStore::get(".len()..];
    let Some((_, key)) = args.split_once(',') else { return false };
    let key = key.split(')').next().unwrap_or_default().trim().trim_start_matches('&').trim();
    let name = key.rsplit("::").next().unwrap_or(key);
    !name.is_empty()
        && name.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        && name.chars().any(|ch| ch.is_ascii_uppercase())
}

fn opens_struct(line: &str) -> Option<&str> {
    let rest = strip_vis(line).strip_prefix("struct ")?;
    let name = rest.split(|ch: char| !is_ident_char(ch)).next()?;
    (!name.is_empty() && line.trim_end().ends_with('{')).then_some(name)
}

fn derives_deserialize(lines: &[&str], index: usize) -> bool {
    for line in lines[..index].iter().rev() {
        let text = line.trim();
        if text.is_empty() {
            continue;
        }
        if text.ends_with('}') || text.ends_with(';') || text.ends_with('{') {
            return false;
        }
        if spells(text, "Deserialize") {
            return true;
        }
    }
    false
}

fn field_name(line: &str) -> Option<&str> {
    let rest = line.trim_start();
    if rest.is_empty() || rest.starts_with('#') {
        return None;
    }
    let (name, _) = strip_vis(rest).split_once(':')?;
    let name = name.trim();
    is_ident(name).then_some(name)
}
