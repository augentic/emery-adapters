//! Component-identity clustering over the public [`vectis::infer::run`]
//! surface: Evidence candidate notes feed unmerged screens into the
//! cluster report without a writable project cache.

use std::path::Path;

use serde_json::Value;
use tempfile::TempDir;
use vectis::infer::{InferArgs, run};

fn write_rel(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir parent");
    std::fs::write(path, body).expect("write file");
}

const ROW_GROUP: &str = r"
version: 1
screens:
  archive:
    body:
      group:
        items:
          - checkbox: {}
          - text: {}
";

const TASK_LIST_EVIDENCE: &str = r"
authority: documentation
lead: task-list
claims:
  - kind: container
    id: task-list.body.task-row
    screen: task-list
    region: body
    parent: task-list.body
    container: group
    notes:
      candidate_component: task-row
  - kind: leaf
    id: task-list.body.task-row.checkbox
    parent: task-list.body.task-row
    leaf: checkbox
  - kind: leaf
    id: task-list.body.task-row.title
    parent: task-list.body.task-row
    leaf: text
";

fn args(root: &Path, slices: bool, cache: Option<&Path>) -> InferArgs {
    InferArgs {
        composition: root.join(".emery/specs/composition.yaml"),
        slices: slices.then(|| root.join(".emery/slices")),
        candidate_cache: cache.map(Path::to_path_buf),
        parts: None,
        min_occurrences: vectis::infer::DEFAULT_MIN_OCCURRENCES,
    }
}

fn clusters(report: &Value) -> &[Value] {
    report.get("clusters").and_then(Value::as_array).expect("clusters array").as_slice()
}

#[test]
fn evidence_notes_supply_second_screen() {
    let tmp = TempDir::new().expect("tempdir");
    write_rel(tmp.path(), ".emery/specs/composition.yaml", ROW_GROUP);
    write_rel(tmp.path(), ".emery/slices/task-list/evidence/screens.yaml", TASK_LIST_EVIDENCE);

    let report = run(&args(tmp.path(), true, None)).expect("infer");
    let clusters = clusters(&report);
    assert_eq!(clusters.len(), 1, "baseline archive + evidence task-list cluster");
    assert_eq!(clusters[0]["occurrences"], 2);
    let screens = clusters[0]["screens"].as_array().expect("screens");
    let names: Vec<&str> = screens.iter().filter_map(Value::as_str).collect();
    assert_eq!(names, ["archive", "task-list"]);
    let names = clusters[0]["evidence"]["candidate-names"].as_array().expect("candidate-names");
    assert_eq!(names, &vec![Value::String("task-row".into())]);
}

#[test]
fn baseline_only_waits_for_second_merged_screen() {
    let tmp = TempDir::new().expect("tempdir");
    write_rel(tmp.path(), ".emery/specs/composition.yaml", ROW_GROUP);

    let report = run(&args(tmp.path(), true, None)).expect("infer");
    assert!(clusters(&report).is_empty(), "one baseline screen is below the threshold");
}

#[test]
fn sidecar_overlay_still_folds() {
    let tmp = TempDir::new().expect("tempdir");
    write_rel(tmp.path(), ".emery/specs/composition.yaml", ROW_GROUP);
    write_rel(
        tmp.path(),
        ".emery/.cache/component-candidates/settings/settings/row.yaml",
        r"
candidate_component: setting-row
region: body
group:
  items:
    - checkbox: {}
    - text: {}
",
    );

    let cache = tmp.path().join(".emery/.cache/component-candidates");
    let report = run(&args(tmp.path(), false, Some(&cache))).expect("infer");
    let clusters = clusters(&report);
    assert_eq!(clusters.len(), 1, "sidecar overlay still supplies the second screen");
    assert_eq!(clusters[0]["occurrences"], 2);
}
