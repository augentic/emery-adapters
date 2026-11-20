//! Extracts settings, URL paths, and domain terms from design prose and identifiers.

use std::collections::BTreeSet;

use emery_sdk::words;

use crate::scaffold::Settings;

#[derive(Clone)]
pub struct ProseMentions {
    pub settings: BTreeSet<String>,
    pub paths: BTreeSet<UrlPath>,
    pub domain: DomainTerms,
}

impl ProseMentions {
    pub fn parse(text: &str) -> Self {
        Self {
            settings: setting_tokens(text),
            paths: UrlPath::in_prose(text),
            domain: DomainTerms::parse(text),
        }
    }

    pub fn symmetric_difference(&self, other: &Self) -> Self {
        Self {
            settings: self.settings.symmetric_difference(&other.settings).cloned().collect(),
            paths: self.paths.symmetric_difference(&other.paths).cloned().collect(),
            domain: self.domain.symmetric_difference(&other.domain),
        }
    }

    pub fn into_union(mut self, other: Self) -> Self {
        self.settings.extend(other.settings);
        self.paths.extend(other.paths);
        self.domain.extend(other.domain);
        self
    }

    pub fn has_setting_or_path(&self) -> bool {
        !self.settings.is_empty() || !self.paths.is_empty()
    }

    pub fn has_any(&self) -> bool {
        self.has_setting_or_path() || !self.domain.is_empty()
    }
}

pub fn setting_tokens(text: &str) -> BTreeSet<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|word| Settings::is_key(word))
        .map(str::to_owned)
        .collect()
}

#[derive(Clone, Default)]
pub struct DomainTerms(BTreeSet<String>);

impl DomainTerms {
    const MIN_LEN: usize = 3;
    const MIN_PLURAL_LEN: usize = 4;

    pub fn parse(text: &str) -> Self {
        let tokens: Vec<String> = text
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
        let mut terms: BTreeSet<String> =
            tokens.iter().filter(|word| word.len() >= Self::MIN_LEN).cloned().collect();
        terms.extend(tokens.windows(2).filter_map(|pair| {
            let [left, right] = pair else { return None };
            (left.len() >= Self::MIN_LEN || right.len() >= Self::MIN_LEN)
                .then(|| format!("{left}_{right}"))
        }));
        Self(terms)
    }

    pub fn symmetric_difference(&self, other: &Self) -> Self {
        Self(self.0.symmetric_difference(&other.0).cloned().collect())
    }

    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    pub fn insert_identifier(&mut self, raw: &str) {
        let segments = words(raw);
        let Some((last, head)) = segments.split_last() else {
            return;
        };
        for tail in Self::inflections(last) {
            let plain = format!("{}{tail}", head.concat());
            if plain.len() >= Self::MIN_LEN {
                self.0.insert(plain);
            }
            let snake = if head.is_empty() { tail } else { format!("{}_{tail}", head.join("_")) };
            if snake.len() >= Self::MIN_LEN {
                self.0.insert(snake);
            }
        }
    }

    fn inflections(word: &str) -> Vec<String> {
        let mut forms = vec![word.to_owned()];
        if word.len() >= Self::MIN_PLURAL_LEN
            && !word.ends_with("ss")
            && !word.ends_with("us")
            && !word.ends_with("is")
            && let Some(singular) = word.strip_suffix('s')
        {
            forms.push(singular.to_owned());
        } else if word.len() >= Self::MIN_LEN && !word.ends_with('s') {
            forms.push(format!("{word}s"));
        }
        forms
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn intersects(&self, other: &Self) -> bool {
        !self.0.is_disjoint(&other.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PathSegment {
    Literal(String),
    Param,
}

impl PathSegment {
    fn parse(raw: &str) -> Self {
        if (raw.starts_with('{') && raw.ends_with('}')) || raw.starts_with(':') {
            Self::Param
        } else {
            Self::Literal(raw.to_owned())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct UrlPath {
    segments: Vec<PathSegment>,
}

impl UrlPath {
    pub fn in_prose(text: &str) -> BTreeSet<Self> {
        text.split_whitespace().filter_map(Self::from_design_token).collect()
    }

    pub fn from_literal(literal: &str) -> Option<Self> {
        Self::from_url_or_path(literal)
    }

    pub fn starts_with(&self, prefix: &Self) -> bool {
        self.segments.starts_with(&prefix.segments)
    }

    fn from_design_token(token: &str) -> Option<Self> {
        let trimmed = token
            .trim_matches(|ch: char| {
                matches!(ch, '`' | '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>')
            })
            .trim_end_matches(['.', ',', ':', ';', '!', '?']);
        Self::from_url_or_path(trimmed)
    }

    fn from_url_or_path(text: &str) -> Option<Self> {
        if let Some(after_scheme) =
            text.strip_prefix("https://").or_else(|| text.strip_prefix("http://"))
        {
            let slash = after_scheme.find('/')?;
            return Self::parse(&after_scheme[slash..]);
        }
        Self::parse(strip_leading_placeholder(text))
    }

    fn parse(candidate: &str) -> Option<Self> {
        let path = candidate.split(['?', '#']).next().unwrap_or(candidate);
        if !path.starts_with('/') || path.starts_with("//") {
            return None;
        }
        let trimmed = path.trim_matches('/');
        if trimmed.is_empty()
            || !trimmed.chars().all(|ch| {
                ch.is_ascii_alphanumeric()
                    || matches!(ch, '/' | '-' | '_' | '{' | '}' | ':' | '*' | '.')
            })
        {
            return None;
        }
        let segments: Vec<PathSegment> = trimmed
            .split('/')
            .filter(|segment| !segment.is_empty())
            .map(PathSegment::parse)
            .collect();
        (!segments.is_empty()).then_some(Self { segments })
    }
}

fn strip_leading_placeholder(text: &str) -> &str {
    let opened = text.strip_prefix("${").or_else(|| text.strip_prefix('{'));
    if let Some((inner, rest)) = opened.and_then(|rest| rest.split_once('}'))
        && !inner.contains('/')
        && rest.starts_with('/')
    {
        return rest;
    }
    text
}
