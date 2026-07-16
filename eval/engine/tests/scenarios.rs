//! The committed scenario set stays loadable against the linked
//! first-party catalog. Generic runner gates (config refusals,
//! artifact gates, run dirs, outcomes) live in the shared harness's
//! own suite.

use std::fs;
use std::path::{Path, PathBuf};

use engine::{FirstParty, SCENARIOS};
use harness::scenario::{self, Scenarios};

#[test]
fn wiring() {
    let scenarios = Scenarios {
        dir: SCENARIOS.into(),
    };
    let mut seen = 0;
    for dir in scenario_dirs(&scenarios.dir) {
        let id = dir.strip_prefix(&scenarios.dir).expect("scenario under the scenarios root");
        let config = scenario::load::<FirstParty>(&scenarios, &dir)
            .unwrap_or_else(|err| panic!("{}: {err:#}", id.display()));

        let inputs: Vec<PathBuf> = fs::read_dir(dir.join("inputs"))
            .unwrap_or_else(|err| panic!("{}: reading inputs/: {err}", id.display()))
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        assert!(!inputs.is_empty(), "{}: no `inputs/*.md`", id.display());
        assert!(!config.slice.trim().is_empty(), "{}: empty slice name", id.display());
        seen += 1;
    }
    assert!(seen >= 6, "expected the committed scenario set, found {seen}");
}

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
