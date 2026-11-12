//! Reads the typed trigger a requirement carries: the route the SDK serves,
//! the message it consumes, and the stem a route's resource names.

use emery_sdk::survey::route::Spelling;
use emery_sdk::target::Requirement;
use emery_sdk::{Route, Trigger};

use crate::layout;

// The methods `axum::routing` has a method router for, each with that
// function.
const METHODS: [(&str, &str); 5] =
    [("DELETE", "delete"), ("GET", "get"), ("PATCH", "patch"), ("POST", "post"), ("PUT", "put")];

// A typed route spells every parameter `{name}`, so a segment holding a
// brace is the one pattern.
const TYPED: Spelling = Spelling {
    pattern: &['{'],
    param_name: |segment| segment.trim_start_matches('{').trim_end_matches('}'),
};

// The route the requirement carries, where the SDK has a route function
// for its method; a `HEAD` or `OPTIONS` route is none, so the requirement
// scaffolds a function.
pub fn route(requirement: &Requirement) -> Option<&Route> {
    match &requirement.trigger {
        Some(Trigger::Http(route))
            if METHODS.iter().any(|(name, _)| *name == route.method)
                && Route::parse(&route.to_string()).is_ok_and(|parsed| parsed == *route) =>
        {
            Some(route)
        }
        _ => None,
    }
}

// The message the requirement consumes, where its trigger is a topic token;
// a name outside the charset scaffolds a function.
pub fn message(requirement: &Requirement) -> Option<&str> {
    match &requirement.trigger {
        Some(Trigger::Message(name)) if is_topic(name) => Some(name),
        _ => None,
    }
}

fn is_topic(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':' | '~' | '%' | '@')
        })
}

// The method router function for `method`, `get` for one it has none for.
pub fn method_fn(method: &str) -> &'static str {
    METHODS.iter().find(|(name, _)| *name == method).map_or("get", |(_, function)| function)
}

// Whether the method carries a request body the handler reads as JSON.
pub fn carries_body(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH")
}

// The stem the route's first resource segment names, `health` for `GET
// /api/health/live`; none for a route of parameters and noise alone.
pub fn resource(route: &Route) -> Option<String> {
    TYPED.stem(&route.path)
}

// The route and resource stem a `start` requirement relays to when its
// trigger is a supported route naming a non-`start` resource.
pub fn relaid(requirement: &Requirement) -> Option<(&Route, String)> {
    let route = route(requirement)?;
    let stem = resource(route).filter(|stem| !layout::is_start_stem(stem))?;
    Some((route, stem))
}

// The stem a requirement lands under: its own stem, or for `start` the
// resource stem of its route; `None` when `start` carries no route.
pub fn stem(requirement: &Requirement) -> Option<String> {
    let own = requirement.stem();
    if !layout::is_start_stem(own) {
        return Some(own.to_owned());
    }
    relaid(requirement).map(|(_, stem)| stem)
}
