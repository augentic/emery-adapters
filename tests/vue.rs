//! Experiment / non-production: the Vue target natively, against a scripted model.
//!
//! A real browser capture cannot run under the script, so each turn writes a
//! raw capture taken once of a tiny page, `tests/fixtures/vue/capture/`, its
//! stamp computed over `src/` as the turn leaves it. The slice's original
//! facts are that same capture assembled, so its score is a pass.

#![cfg(not(target_arch = "wasm32"))]

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use emery_sdk::target::{
    AttachedLayout, BuildContext, InitContext, Outcome, Report, Requirement, Slice, TargetAdapter,
    Task, Verdict, VerifyContext,
};
use emery_sdk::{Error, body};
use omnia_sdk::model::{Reply, ToolCall};
use omnia_test::guest::Scripted;
use omnia_test::host::{Scratch, scratch};
use sha2::{Digest as _, Sha256};
use web_rendered_layout::{BUNDLE_MEDIA_TYPE, Facts, FactsFolder, assemble, review};

const CAPTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/vue/capture");
const MAIN: &str = include_str!("../adapters/web/vue/seed/src/main.js");
const REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["src/Page.vue"]}"#;
const REFACTORED: &str =
    r#"{"covered": ["REQ-001"], "written": ["src/Page.vue", "src/components/ProduceSection.vue"]}"#;
const PASSED: &str = r#"{"passed": true, "failures": []}"#;
const EXTRACTED_PAGE: &str = "<template>\n  <main class=\"page\">\n    <section><h1>Fresh eggs</h1></section>\n  </main>\n</template>\n";
const REFACTORED_PAGE: &str = "<script setup>\nimport ProduceSection from \"./components/ProduceSection.vue\";\n</script>\n\n<template>\n  <main><ProduceSection /></main>\n</template>\n";
const SECTION: &str = "<template>\n  <section class=\"produce\"><h1>Fresh eggs</h1></section>\n</template>\n\n<style scoped>\n.produce { display: flex; }\n</style>\n";

#[tokio::test]
async fn vue() {
    let project = scratch();
    init(&project).await;
    let extracted = [("src/Page.vue", EXTRACTED_PAGE)];
    let refactored =
        [("src/Page.vue", REFACTORED_PAGE), ("src/components/ProduceSection.vue", SECTION)];
    let model = Scripted::answering([REPORT, REFACTORED, PASSED])
        .calling(0, [turn_files(&extracted, "layout/capture", true)])
        .calling(1, [turn_files(&refactored, "layout/capture", true)])
        .calling(2, [tool_call(&[], &refactored, "layout/verify/capture", false)]);

    let report =
        build(&project, &model, &slice(&original())).await.expect("the build passes its checks");
    let verdict = verify(&project, &model).await.expect("the verification passes its check");
    model.assert_exhausted();

    assert_eq!(report.covered, ["REQ-001"]);
    assert!(report.written.contains(&"src/components/ProduceSection.vue".to_owned()));
    assert!(
        report.written.contains(&"layout/original/index.json".to_owned()),
        "{:?}",
        report.written
    );
    assert!(verdict.passed);

    let seen = model.seen();
    assert_eq!(seen.len(), 3, "an extraction, a refactor, and a verification");
    for (request, table) in [(&seen[0], emery_vue::EXTRACT), (&seen[1], emery_vue::REFACTOR)] {
        assert_eq!(
            request.system.as_deref(),
            body(table, "build.md"),
            "each build turn opens under its own prompt"
        );
    }
    assert_eq!(seen[1].system.as_deref(), body(emery_vue::PROSE, "refactor.md"));
    assert_eq!(seen[2].system.as_deref(), body(emery_vue::PROSE, "verify.md"));
    for path in [
        "src/main.js",
        "src/Page.vue",
        "src/components/ProduceSection.vue",
        "layout/original/index.json",
        "layout/refactor/ref/320.png",
        "layout/verify/score.json",
        "layout/verify/overlay-320.svg",
    ] {
        assert!(project.read(path).is_some(), "the tree holds `{path}`");
    }
    assert!(project.read("layout/facts/index.json").is_none(), "the extraction's facts are gone");
}

// The refactor turn's tree holds the stripped facts and the extracted
// page's screenshots, and neither the original facts nor the extracted code.
// A refactor that never answers leaves the tree as the turn saw it.
#[tokio::test]
async fn vue_refactor_reads_no_structure() {
    let project = scratch();
    init(&project).await;
    let extracted = [("src/Page.vue", EXTRACTED_PAGE)];
    let model = Scripted::answering([REPORT])
        .calling(0, [turn_files(&extracted, "layout/capture", true)])
        .then(|| {
            Ok(Reply {
                answer: REPORT.to_owned(),
                usage: None,
            })
        });

    let refused = build(&project, &model, &slice(&original()))
        .await
        .expect_err("the refactor captures nothing");
    assert!(matches!(refused, Error::BadRequest { .. }), "{refused}");

    assert!(project.read("layout/facts/index.json").is_none(), "the original facts are gone");
    assert!(project.read("src/Page.vue").is_none(), "the extracted code is gone");
    assert!(
        project.read("layout/original/index.json").is_none(),
        "the original is not yet written"
    );
    assert!(
        project.read("layout/refactor/ref/320.png").is_some(),
        "the extracted page's screenshot stands"
    );
    let stripped = FactsFolder::read(&project.path().join("layout/refactor/facts"))
        .expect("the stripped facts");
    let index = stripped.json("index.json").expect("the index");
    for key in ["root", "containers", "ax"] {
        assert!(index.get(key).is_none(), "the stripped index carries no `{key}`");
    }
    for path in stripped.paths().filter(|path| path.starts_with("parts/")) {
        let part =
            String::from_utf8(stripped.get(path).unwrap_or_default().to_vec()).expect("UTF-8");
        for key in ["\"tag\"", "\"path\"", "\"children\""] {
            assert!(!part.contains(key), "`{path}` carries no {key}");
        }
    }
}

#[tokio::test]
async fn vue_positioned_refused() {
    let project = scratch();
    init(&project).await;
    let positioned = "<template>\n  <main style=\"position: absolute\" />\n</template>\n";
    let model = Scripted::answering([REPORT])
        .calling(0, [turn_files(&[("src/Page.vue", positioned)], "layout/capture", true)])
        .then(|| {
            Ok(Reply {
                answer: REPORT.to_owned(),
                usage: None,
            })
        });

    build(&project, &model, &slice(&original())).await.expect_err("the page is positioned");
    let first = refusals(&model).into_iter().next().expect("a candidate is refused");
    assert!(first.contains("`src/Page.vue:2` positions an element out of the flow"), "{first}");
}

// A tree whose original moved on scores below the pass mark: a passing
// verdict is refused naming the score's findings, and the verdict that
// quotes them verbatim, each naming its overlay, is the answer.
#[tokio::test]
async fn vue_verify_held_to_the_score() {
    let project = scratch();
    init(&project).await;
    let extracted = [("src/Page.vue", EXTRACTED_PAGE)];
    let refactored =
        [("src/Page.vue", REFACTORED_PAGE), ("src/components/ProduceSection.vue", SECTION)];
    let model = Scripted::answering([REPORT, REFACTORED])
        .calling(0, [turn_files(&extracted, "layout/capture", true)])
        .calling(1, [turn_files(&refactored, "layout/capture", true)]);
    build(&project, &model, &slice(&original())).await.expect("the build passes its checks");

    let moved = renamed(&original(), "Fresh eggs", "Fresh honey");
    let original_dir = project.path().join("layout/original");
    fs::remove_dir_all(&original_dir).expect("the original is replaced");
    moved.write(&original_dir).expect("the moved original is written");
    let expected = {
        let render = Facts::read(&assemble(Path::new(CAPTURE)).expect("the fixture assembles"))
            .expect("facts");
        let reviewed = review(&Facts::read(&moved).expect("facts"), &render).expect("a review");
        reviewed.findings(|width| format!("layout/verify/overlay-{}.svg", width.0))
    };
    assert!(!expected.is_empty(), "the moved original fails the score");
    let failed = serde_json::json!({ "passed": false, "failures": expected }).to_string();
    let model = Scripted::answering([PASSED, failed.as_str()])
        .calling(0, [tool_call(&[], &refactored, "layout/verify/capture", false)]);

    let verdict = verify(&project, &model).await.expect("the verification answers");
    model.assert_exhausted();

    assert!(!verdict.passed);
    assert_eq!(verdict.failures, expected);
    let [refused] = refusals(&model).try_into().expect("one candidate is refused");
    assert!(refused.contains("so the verdict is `passed: false`"), "{refused}");
    assert!(expected.iter().any(|line| line.starts_with("overlay at ")), "{expected:?}");
    for line in expected.iter().filter(|line| line.starts_with("overlay at ")) {
        let path = line.rsplit(": ").next().unwrap_or_default();
        assert!(
            project.read(path).is_some(),
            "the finding names the overlay `{path}` the check drew"
        );
    }
}

async fn init(project: &Scratch) {
    let ctx = InitContext {
        adapter_id: "vue",
        workspace: root(project),
    };
    match emery_vue::Adapter::run(Task::<Scripted>::Init(ctx)).await.expect("init") {
        Outcome::Initialised => {}
        outcome => panic!("an init answers an initialised tree, not {outcome}"),
    }
}

async fn build(project: &Scratch, model: &Scripted, slice: &Slice) -> Result<Report, Error> {
    let ctx = BuildContext {
        adapter_id: "vue",
        slice,
        workspace: root(project),
        grants: emery_vue::Adapter::metadata().grants,
        model,
        scaffold: None,
    };
    match emery_vue::Adapter::run(Task::Build(ctx)).await? {
        Outcome::Built(report) => Ok(report),
        outcome => panic!("a build answers a report, not {outcome}"),
    }
}

async fn verify(project: &Scratch, model: &Scripted) -> Result<Verdict, Error> {
    let ctx = VerifyContext {
        adapter_id: "vue",
        workspace: root(project),
        grants: emery_vue::Adapter::metadata().grants,
        model,
    };
    match emery_vue::Adapter::run(Task::Verify(ctx)).await? {
        Outcome::Verified(verdict) => Ok(verdict),
        outcome => panic!("a verify answers a verdict, not {outcome}"),
    }
}

// The slice of one requirement carrying `original` as its one layout.
fn slice(original: &FactsFolder) -> Slice {
    let body = original.to_bundle();
    Slice {
        id: "SLICE-001".to_owned(),
        name: "farm-page".to_owned(),
        base: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        requirements: vec![Requirement {
            id: "REQ-001".to_owned(),
            subject: "page.render".to_owned(),
            statement: vec!["The page renders as recorded.".to_owned()],
            scenarios: Vec::new(),
            trigger: None,
        }],
        types: Vec::new(),
        brief: Vec::new(),
        spec: "# Specification\n\n## Requirement: page.render\n\nID: REQ-001\n".to_owned(),
        design: "# Design\n".to_owned(),
        plan: "## Slice: farm-page\n\nID: SLICE-001\nRequirements: REQ-001\n".to_owned(),
        layouts: vec![AttachedLayout {
            digest: format!("sha256:{}", hex(&Sha256::digest(&body))),
            media_type: BUNDLE_MEDIA_TYPE.to_owned(),
            body,
        }],
    }
}

fn original() -> FactsFolder {
    assemble(Path::new(CAPTURE)).expect("the fixture capture assembles")
}

// `folder` with every part's `from` text put as `to`.
fn renamed(folder: &FactsFolder, from: &str, to: &str) -> FactsFolder {
    let mut moved = folder.clone();
    for path in folder.paths().filter(|path| path.starts_with("parts/")) {
        let part = String::from_utf8(folder.get(path).unwrap_or_default().to_vec()).expect("UTF-8");
        moved.insert(path, part.replace(from, to).into_bytes()).expect("the part is replaced");
    }
    moved
}

// One `write_files` call writing `sources` under `src/`, the fixture capture
// under `capture` stamped over `src/` as the call leaves it, and with
// `shots` a screenshot beside it.
fn turn_files(sources: &[(&str, &str)], capture: &str, shots: bool) -> ToolCall {
    tool_call(sources, sources, capture, shots)
}

// One `write_files` call writing `written` and the fixture capture under
// `capture` stamped over `stamped` and the seeded entry.
fn tool_call(
    written: &[(&str, &str)], stamped: &[(&str, &str)], capture: &str, shots: bool,
) -> ToolCall {
    let mut stamp = serde_json::Map::new();
    stamp.insert("main.js".to_owned(), hex(&Sha256::digest(MAIN.as_bytes())).into());
    for (path, content) in stamped {
        let below = path.strip_prefix("src/").expect("a source under `src/`");
        stamp.insert(below.to_owned(), hex(&Sha256::digest(content.as_bytes())).into());
    }
    let mut files: Vec<(String, String)> =
        written.iter().map(|(path, content)| ((*path).to_owned(), (*content).to_owned())).collect();
    for (below, content) in fixture_files(Path::new(CAPTURE), "") {
        let content = if below == "meta.json" {
            let mut meta: serde_json::Value =
                serde_json::from_str(&content).expect("the meta is JSON");
            meta["source"] = serde_json::json!({ "root": "src", "files": stamp });
            meta.to_string()
        } else {
            content
        };
        files.push((format!("{capture}/{below}"), content));
    }
    if shots {
        files.push(("layout/capture-shots/320.png".to_owned(), "a screenshot".to_owned()));
    }
    let files: Vec<_> = files
        .iter()
        .map(|(path, content)| serde_json::json!({ "path": path, "content": content }))
        .collect();
    ToolCall {
        id: "call-1".to_owned(),
        name: "write_files".to_owned(),
        arguments: serde_json::json!({ "files": files }).to_string(),
    }
}

fn fixture_files(dir: &Path, prefix: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .expect("the fixture is readable")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().expect("a name").to_string_lossy().into_owned();
        if path.is_dir() {
            files.extend(fixture_files(&path, &format!("{prefix}{name}/")));
        } else {
            files
                .push((format!("{prefix}{name}"), fs::read_to_string(&path).expect("a text file")));
        }
    }
    files
}

// The corrections the adapter's checks put to the model, in order.
fn refusals(model: &Scripted) -> Vec<String> {
    model
        .exchanges()
        .into_iter()
        .filter(|exchange| exchange.tool == "check")
        .filter_map(|exchange| exchange.outcome.err())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

fn root(project: &Scratch) -> &str {
    project.path().to_str().expect("a UTF-8 scratch root")
}
