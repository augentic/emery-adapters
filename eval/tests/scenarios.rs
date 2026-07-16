//! Model-free wiring smoke over the prompt scenarios: every scenario
//! directory parses, routes to a linked adapter, and carries inputs —
//! the checks `specify-dev eval scenario` performs before it would
//! spend a model request.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    adapter: String,
    operation: String,
    slice: String,
    #[serde(default)]
    expect: Vec<String>,
}

#[test]
fn wiring() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    let linked: Vec<String> = eval::catalog::entries().iter().map(|entry| entry.id()).collect();

    let mut seen = 0;
    for scenario in scenario_dirs(&root) {
        let id = scenario
            .strip_prefix(&root)
            .expect("scenario under the scenarios root")
            .display()
            .to_string();
        let body = fs::read_to_string(scenario.join("scenario.toml"))
            .unwrap_or_else(|err| panic!("{id}: reading scenario.toml: {err}"));
        let config: Config = toml::from_str(&body)
            .unwrap_or_else(|err| panic!("{id}: parsing scenario.toml: {err}"));

        assert!(
            linked.contains(&config.adapter),
            "{id}: adapter `{}` is not linked into the native shim",
            config.adapter
        );
        assert!(
            ["build", "merge-preflight", "merge-postflight"].contains(&config.operation.as_str()),
            "{id}: unknown operation `{}`",
            config.operation
        );
        assert!(!config.slice.trim().is_empty(), "{id}: empty slice name");
        assert!(
            config.expect.iter().all(|rel| !rel.trim().is_empty()),
            "{id}: empty expect entry"
        );

        let inputs: Vec<PathBuf> = fs::read_dir(scenario.join("inputs"))
            .unwrap_or_else(|err| panic!("{id}: reading inputs/: {err}"))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        assert!(!inputs.is_empty(), "{id}: no `inputs/*.md`");
        seen += 1;
    }
    assert!(seen >= 6, "expected the committed scenario set, found {seen}");
}

/// Every `<adapter>/<name>` directory carrying a `scenario.toml`.
fn scenario_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for adapter in fs::read_dir(root).expect("scenarios root") {
        let adapter = adapter.expect("scenarios entry").path();
        if !adapter.is_dir() {
            continue;
        }
        for scenario in fs::read_dir(&adapter).expect("adapter scenarios") {
            let scenario = scenario.expect("scenario entry").path();
            if scenario.join("scenario.toml").is_file() {
                dirs.push(scenario);
            }
        }
    }
    dirs.sort();
    dirs
}
