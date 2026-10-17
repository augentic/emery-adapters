//! Holds every constant-keyed `StateStore::get` to a cache-aside load: a
//! table read under one key fetches and stores the table on `Ok(None)` and
//! never answers the miss as an empty collection.

use emery_sdk::Error;

use super::{Tree, code, read, spells, strip_vis};

// The empty collections a miss is answered as, each spelled whole: behind a
// `return Ok(..)`, as the body of an arm naming `None`, bare or in `Ok(..)`,
// or as the tail expression once the hit has returned above it.
const NOTHING: &[&str] = &[
    "Vec::new()",
    "HashMap::new()",
    "BTreeMap::new()",
    "HashSet::new()",
    "BTreeSet::new()",
    "vec![]",
    "Default::default()",
];

// How a miss is defaulted on its way to the answer, anywhere in a line.
const DEFAULTED: &[&str] = &[
    ".unwrap_or_default()",
    ".unwrap_or_else(Vec::new)",
    ".unwrap_or_else(HashMap::new)",
    ".unwrap_or_else(Default::default)",
    ".unwrap_or(Vec::new())",
];

// The lines a constant-keyed `StateStore::get` is read with where the
// function it sits in cannot be found: the `let .. else` block, or the
// chain the read ends in.
const MISS_WINDOW: usize = 6;

impl Tree<'_> {
    // One finding naming every `StateStore::get` under a constant key whose
    // miss is answered empty within the function that reads, its hit arms
    // set aside, each `<path>:<line>: <what>` at the line that answers it: a
    // table read under one key is loaded on a miss, so a read answering an
    // empty collection there has laid the cache and lost the table. A read
    // under a key built from the input is a lookup, whose miss is its own,
    // and no finding.
    pub(super) fn misses(&self) -> Result<Vec<String>, Error> {
        let mut lines = Vec::new();
        for file in self.modules()? {
            let text = code(&read(&file)?);
            let path = self.relative(&file);
            let all: Vec<&str> = text.lines().collect();
            for (index, line) in all.iter().enumerate() {
                if !constant_keyed(line) {
                    continue;
                }
                let end = function_end(&all, index);
                for at in answering(&all, index, end) {
                    if let Some((spelling, what)) = empty(all[at]) {
                        lines.push(format!(
                            "{path}:{}: {what} (`{spelling}`) after the read at line {}",
                            at + 1,
                            index + 1
                        ));
                    }
                }
            }
        }
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        lines.sort();
        lines.dedup();
        Ok(vec![format!(
            "a table read from the store under one key answers empty on a miss, so the table is \
             never loaded: on `Ok(None)` the function fetches the table from its upstream, as the \
             design spells the call, stores it under the same key with a TTL, and answers what it \
             fetched, never an empty collection; repair each read named through `edit_files`, \
             laying the fetch in the same function:\n{}",
            lines.join("\n")
        )])
    }
}

// The spelling by which `line` answers a miss empty, and what it is: one of
// `NOTHING` behind a `return Ok(..)` anywhere in the line; one the line
// answers with, bare or in `Ok(..)` — the body of an arm whose pattern
// names `None`, `Ok(None) | Err(_)` among them, or the line whole as a tail,
// since `Ok(Vec::new())` inside a longer line is an argument rather than
// the answer; else one of `DEFAULTED`.
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

// The index past the last line of the function `lines[index]` sits in: the
// closing brace at the indentation of the nearest `fn` head above, as
// `rustfmt` lays it; `MISS_WINDOW` lines where no head is above it.
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

// Whether `line` is a function's head: `fn` behind any visibility — `pub`,
// `pub(crate)`, `pub(super)`, `pub(in ..)` — and `const`, `async`, or
// `unsafe`, as `rustfmt` lays one.
fn opens_fn(line: &str) -> bool {
    let mut rest = strip_vis(line);
    for qualifier in ["const ", "async ", "unsafe "] {
        rest = rest.strip_prefix(qualifier).unwrap_or(rest);
    }
    rest.starts_with("fn ")
}

// The lines from the read at `index` to `end` that can answer its miss:
// every line but a hit arm's — a `Some(..) =>` arm, the body of an `if let
// Some(..)`, and what follows a `let .. else` once its block closes — since
// a default there decodes what the store held rather than answering the
// miss. A hit arm runs over the lines indented past its head and the brace
// that closes it, as `rustfmt` lays them.
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

// Whether `line` reads the store under a constant key: `StateStore::get(`
// whose second argument is a constant name, `KEY` or `config::KEY`, rather
// than a key built from the input.
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
