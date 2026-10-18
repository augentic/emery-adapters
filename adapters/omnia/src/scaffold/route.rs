//! Reads the route a requirement states: `METHOD /path` in its statement or a scenario's `when`.

use emery_sdk::target::{Requirement, Route};

// The methods `omnia_sdk::api::http` has a route function for, each with
// that function.
const METHODS: [(&str, &str); 5] =
    [("DELETE", "delete"), ("GET", "get"), ("PATCH", "patch"), ("POST", "post"), ("PUT", "put")];

// The route the requirement carries, else the first its statement spells,
// else the first a scenario's `when` spells; a method the SDK has no route
// function for is none.
pub(super) fn of(requirement: &Requirement) -> Option<Route> {
    if let Some(route) = &requirement.route {
        return normalised(&route.method, &route.path);
    }
    requirement
        .statement
        .iter()
        .chain(requirement.scenarios.iter().map(|scenario| &scenario.when))
        .find_map(|text| spelled(text))
}

pub(super) fn function(method: &str) -> &'static str {
    METHODS.iter().find(|(name, _)| *name == method).map_or("get", |(_, function)| function)
}

// The `{name}` parameters of a path, in order, a `{*rest}` wildcard by its
// name.
pub(super) fn params(path: &str) -> Vec<String> {
    path.split('/')
        .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
        .map(|name| name.trim_start_matches('*').to_owned())
        .filter(|name| !name.is_empty())
        .collect()
}

fn spelled(text: &str) -> Option<Route> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    tokens.windows(2).find_map(|pair| {
        let method = pair[0].trim_matches(|ch: char| !ch.is_ascii_alphabetic());
        let path = pair[1]
            .trim_start_matches(['`', '"', '\'', '(', '['])
            .trim_end_matches(['`', '"', '\'', ')', ']', '.', ',', ';', ':', '!', '?']);
        let path = path.split('?').next().unwrap_or(path);
        normalised(method, path)
    })
}

// `method` as `METHODS` spells it and `path` as axum does, `:id` and `<id>`
// as `{id}`; none for a method the SDK lacks or a token that is no path.
fn normalised(method: &str, path: &str) -> Option<Route> {
    let (method, _) = METHODS.iter().find(|(name, _)| name.eq_ignore_ascii_case(method))?;
    let shaped = path.starts_with('/')
        && path.chars().all(|ch| ch.is_ascii_alphanumeric() || "/_-.{}:*<>".contains(ch));
    if !shaped {
        return None;
    }
    let segments: Vec<String> = path
        .split('/')
        .map(|segment| {
            let name = segment
                .strip_prefix(':')
                .or_else(|| segment.strip_prefix('<').and_then(|rest| rest.strip_suffix('>')));
            name.map_or_else(|| segment.to_owned(), |name| format!("{{{name}}}"))
        })
        .collect();
    let mut path = segments.join("/");
    if path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    Some(Route {
        method: (*method).to_owned(),
        path,
    })
}
