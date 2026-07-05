//! Workspace-wide unit-test ratchet for the specify-adapters crates.
//!
//! Run with `cargo test --test rust_quality` (or via `cargo make test`).
//! Counts `#[test]` / `#[tokio::test]` declarations under each adapter's
//! `src/` trees (the guest shim and its sub-crates) and fails when the
//! live count drifts from the committed budget in
//! `rust_quality_budget.toml`. Mirrors the engine gate and enforces the
//! integration-first posture in TESTING.md.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// Per-adapter src unit-test budget file, relative to this crate's root.
const BUDGET_FILE: &str = "rust_quality_budget.toml";

/// Count `#[test]` / `#[tokio::test]` declarations in each adapter's
/// `src/` trees, keyed by adapter directory name. Integration tests
/// under any `tests/` tree are excluded by construction — only `src/`
/// files are scoped.
fn count_src_unit_tests(root: &Path) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    count_walk(root, root, &mut counts);
    counts
}

fn count_walk(root: &Path, dir: &Path, counts: &mut BTreeMap<String, usize>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            count_walk(root, &path, counts);
            continue;
        }
        if path.extension().is_some_and(|e| e == "rs") {
            count_src_file(root, &path, counts);
        }
    }
}

fn count_src_file(root: &Path, path: &Path, counts: &mut BTreeMap<String, usize>) {
    let rel = path.strip_prefix(root).unwrap_or(path).display().to_string().replace('\\', "/");
    let Some(scope) = adapter_scope(&rel) else {
        return;
    };
    let Ok(content) = fs::read_to_string(path) else {
        return;
    };
    let n = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("#[test]") || trimmed.starts_with("#[tokio::test")
        })
        .count();
    if n > 0 {
        *counts.entry(scope).or_default() += n;
    }
}

/// Scope key for an adapter `src/` Rust file, or `None` when the file
/// is elsewhere. Matches the guest shim (`{targets,sources}/<name>/src/**`)
/// and every sub-crate (`{targets,sources}/<name>/crates/*/src/**`), keying
/// both to `<name>`.
fn adapter_scope(rel: &str) -> Option<String> {
    let mut parts = rel.split('/');
    let axis = parts.next()?;
    if axis != "targets" && axis != "sources" {
        return None;
    }
    let name = parts.next()?;
    match parts.next()? {
        "src" => Some(name.to_owned()),
        "crates" => {
            let _crate_dir = parts.next()?;
            (parts.next() == Some("src")).then(|| name.to_owned())
        }
        _ => None,
    }
}

/// Workspace root: this crate lives at `<root>/tools/rust-quality`.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("tools/rust-quality is two levels below the workspace root")
        .to_path_buf()
}

/// Read the ratchet budget. A minimal `key = <int>` reader (skips blank
/// lines, `#` comments, and the `[adapter]` header) keeps this dev-gate
/// dependency-free rather than pulling in a TOML parser.
fn load_budget() -> BTreeMap<String, usize> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(BUDGET_FILE);
    let content =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut budget = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"').to_owned();
        let count = value
            .trim()
            .parse()
            .unwrap_or_else(|e| panic!("budget for `{key}` is not a usize: {e}"));
        budget.insert(key, count);
    }
    budget
}

/// Strict ratchet: the committed budget must equal the live src unit-test
/// count per adapter. Above budget means a new unit test was added —
/// exercise the behavior through the public surface (tests/cli.rs or
/// tests/engine.rs) instead; below budget means a reduction landed and the
/// number must be lowered to lock it in. Either way the budget edit is the
/// reviewable signal that catches a unit test being added.
#[test]
fn unit_test_budget_holds() {
    let counts = count_src_unit_tests(&workspace_root());
    let budget = load_budget();

    let scopes: BTreeSet<&String> = counts.keys().chain(budget.keys()).collect();
    let mut failures = String::new();
    for scope in scopes {
        let current = counts.get(scope).copied().unwrap_or(0);
        let allowed = budget.get(scope).copied().unwrap_or(0);
        if current > allowed {
            writeln!(
                failures,
                "[{scope}] {current} src unit tests > budget {allowed}: do not add src unit tests — exercise the behavior through the public surface in tests/, or justify and raise the budget in review (TESTING.md)"
            )
            .expect("write to String");
        } else if current < allowed {
            writeln!(
                failures,
                "[{scope}] {current} src unit tests < budget {allowed}: ratchet down — set `{scope} = {current}` in tools/rust-quality/{BUDGET_FILE}"
            )
            .expect("write to String");
        }
    }
    assert!(failures.is_empty(), "unit-test ratchet failed:\n{failures}");
}

#[test]
fn counts_src_unit_tests_by_scope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    let shim = root.join("targets/demo/src/lib.rs");
    fs::create_dir_all(shim.parent().expect("parent")).expect("mkdir");
    fs::write(&shim, "#[test]\nfn a() {}\n").expect("write");
    let core = root.join("targets/demo/crates/core/src/foo.rs");
    fs::create_dir_all(core.parent().expect("parent")).expect("mkdir");
    fs::write(&core, "#[test]\nfn b() {}\n#[tokio::test]\nasync fn c() {}\n").expect("write");
    // Integration tests under tests/ must never be counted.
    let it = root.join("targets/demo/crates/core/tests/it.rs");
    fs::create_dir_all(it.parent().expect("parent")).expect("mkdir");
    fs::write(&it, "#[test]\nfn d() {}\n").expect("write");

    let counts = count_src_unit_tests(root);
    assert_eq!(counts.get("demo").copied(), Some(3), "shim + core src tests share one scope");
    assert_eq!(counts.len(), 1, "integration tests under tests/ are excluded");
}
