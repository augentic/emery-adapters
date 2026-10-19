//! Reads the typed route a requirement carries, and the stem its resource names.

use emery_sdk::Route;
use emery_sdk::survey::route::Spelling;
use emery_sdk::target::Requirement;

// The methods `omnia_sdk::api::http` has a route function for, each with
// that function.
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
pub(super) fn of(requirement: &Requirement) -> Option<&Route> {
    requirement.route.as_ref().filter(|route| METHODS.iter().any(|(name, _)| *name == route.method))
}

pub(super) fn function(method: &str) -> &'static str {
    METHODS.iter().find(|(name, _)| *name == method).map_or("get", |(_, function)| function)
}

// The stem the route's first resource segment names, `health` for `GET
// /api/health/live`; none for a route of parameters and noise alone.
pub(super) fn resource(route: &Route) -> Option<String> {
    TYPED.stem(&route.path)
}
