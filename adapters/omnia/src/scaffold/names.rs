//! Spells the identifiers a scaffold writes from what the slice spells.

use std::collections::BTreeSet;

// Every strict and reserved keyword, so a part or a function named as one
// takes a trailing underscore rather than a raw identifier.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "union", "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

// The words of `text`, split at its separators and its camel-case
// boundaries, so `orderId` and `HTTPServer` spell `order_id` and
// `http_server`.
fn words(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    for (at, &ch) in chars.iter().enumerate() {
        if !ch.is_ascii_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        let after = at.checked_sub(1).map(|before| chars[before]);
        let boundary = ch.is_ascii_uppercase()
            && after.is_some_and(|before| {
                before.is_ascii_lowercase()
                    || before.is_ascii_digit()
                    || (before.is_ascii_uppercase()
                        && chars.get(at + 1).is_some_and(char::is_ascii_lowercase))
            });
        if boundary && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.push(ch.to_ascii_lowercase());
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

pub(super) fn snake(text: &str) -> String {
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

pub(super) fn pascal(text: &str) -> String {
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

pub(super) fn screaming(path: &str) -> String {
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
pub(super) fn number(id: &str) -> String {
    let digits: String =
        id.rsplit('-').next().unwrap_or(id).chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() {
        id.chars().filter(char::is_ascii_alphanumeric).collect()
    } else {
        digits
    }
}

pub(super) fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// `text` as a string literal's body: quotes and backslashes escaped.
pub(super) fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Default)]
pub(super) struct Names(BTreeSet<String>);

impl Names {
    // The names the module's own imports and functions take, so a claim
    // never shadows one.
    pub(super) fn reserving(taken: &[&str]) -> Self {
        Self(taken.iter().map(|name| (*name).to_owned()).collect())
    }

    // `name` where no earlier claim took it, else `name` with `suffix` after
    // it, then with a counter after both.
    pub(super) fn claim(&mut self, name: &str, suffix: &str) -> String {
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
