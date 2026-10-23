//! Spells the identifiers a scaffold writes from what the slice spells.

use std::collections::BTreeSet;

use emery_sdk::words;

// Every strict and reserved keyword, so a part or a function named as one
// takes a trailing underscore rather than a raw identifier.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

pub fn snake(text: &str) -> String {
    let words = words(text);
    if words.is_empty() {
        return "unnamed".to_owned();
    }
    let mut name = words.join("_");
    if name.starts_with(|ch: char| ch.is_ascii_digit()) {
        name.insert(0, '_');
    }
    if KEYWORDS.contains(&name.as_str()) {
        name.push('_');
    }
    name
}

pub fn pascal(text: &str) -> String {
    let words = words(text);
    if words.is_empty() {
        return "Unnamed".to_owned();
    }
    let mut name: String = words
        .iter()
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect();
    if name.starts_with(|ch: char| ch.is_ascii_digit()) {
        name.insert(0, 'N');
    }
    name
}

pub fn screaming(path: &str) -> String {
    let words = words(path);
    if words.is_empty() {
        return "ROOT".to_owned();
    }
    let mut name = words.join("_").to_ascii_uppercase();
    if name.starts_with(|ch: char| ch.is_ascii_digit()) {
        name.insert(0, '_');
    }
    name
}

// The digits of a requirement id, `012` for `REQ-012`; the id's own
// alphanumerics where it has none.
pub fn number(id: &str) -> String {
    let digits: String =
        id.rsplit('-').next().unwrap_or(id).chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        id.chars().filter(char::is_ascii_alphanumeric).collect()
    } else {
        digits
    }
}

pub fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// `text` as a string literal's body: quotes and backslashes escaped.
pub fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

// One namespace of a part's identifiers: every name reserved so far, so no
// two items of the kind share one.
#[derive(Default)]
pub struct Names(BTreeSet<String>);

impl Names {
    // A namespace with `taken` reserved already: the module's own imports
    // and functions, so a later reservation never shadows one.
    pub fn taken(taken: &[&str]) -> Self {
        Self(taken.iter().map(|name| (*name).to_owned()).collect())
    }

    // Reserves `name` where nothing took it, else `name` with `suffix` after
    // it, then with a counter after both, answering the name reserved.
    pub fn reserve(&mut self, name: &str, suffix: &str) -> String {
        let mut candidate = name.to_owned();
        let mut count = 1;
        while !self.0.insert(candidate.clone()) {
            count += 1;
            candidate = if count == 2 {
                format!("{name}{suffix}")
            } else {
                format!("{name}{suffix}{count}")
            };
        }
        candidate
    }
}
