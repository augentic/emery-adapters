//! The first-party catalog inventory gate: every first-party adapter
//! is linked exactly once on its axis, published names stay globally
//! unique across axes (the Wasm store carries no axis segment), and
//! each compiled identity carries the exact workspace package version.

use std::collections::BTreeSet;

const SOURCES: &[&str] = &["captures", "documentation", "intent", "screenshots", "typescript"];
const TARGETS: &[&str] = &["contracts", "omnia", "vectis"];

#[test]
fn inventory() {
    let catalog = lab::catalog().expect("the first-party catalog validates");

    let version = env!("CARGO_PKG_VERSION");
    let mut ids: Vec<String> = catalog.entries().iter().map(native::Entry::id).collect();
    ids.sort_unstable();
    let mut expected: Vec<String> = SOURCES
        .iter()
        .map(|name| format!("source:{name}@{version}"))
        .chain(TARGETS.iter().map(|name| format!("target:{name}@{version}")))
        .collect();
    expected.sort_unstable();
    assert_eq!(ids, expected, "every first-party adapter exactly once on its axis, exact-routed");

    // Published names stay globally unique across axes: the component
    // store carries no axis segment, so a dual-axis name would make a
    // binding ambiguous there.
    let names: BTreeSet<&str> = catalog.entries().iter().map(native::Entry::name).collect();
    assert_eq!(names.len(), catalog.entries().len(), "no name appears on both axes");
}

#[test]
fn published_versions() {
    let catalog = lab::catalog().expect("the first-party catalog validates");
    let workspace = env!("CARGO_PKG_VERSION");
    let placeholder = semver::Version::new(0, 0, 0);

    for entry in catalog.entries() {
        assert_eq!(
            entry.version(),
            workspace,
            "`{}` must publish the workspace package version",
            entry.name()
        );
        let version = semver::Version::parse(entry.version()).expect("exact SemVer");
        assert_ne!(
            version,
            placeholder,
            "`{}` must not carry the development placeholder version",
            entry.name()
        );
    }
}
