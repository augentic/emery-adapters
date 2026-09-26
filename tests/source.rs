//! Verifies every shipped adapter through the component interface.
//!
//! Each adapter runs under the omnia runtime against a strict model script.
//! The scenarios verify metadata, survey-selected seam counts and content,
//! embedded extraction prompts, and caller-visible refusals.
//!
//! Assertions use adapter-owned source data rather than SDK prompt wording.
//! Shared SDK and component-boundary behaviour is covered by `probe.rs`.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use omnia_test::host::{Scratch, ScriptedModel, scratch};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

const BRIEF: &str = "Let users reset passwords by email.";

// The prompts as this build compiled them in.
mod prompt {
    pub const DOCUMENTATION: &str = include_str!("../sources/documentation/prose/extract.md");
    pub const INTENT: &str = include_str!("../sources/intent/prose/extract.md");
    pub const TYPESCRIPT: &str = include_str!("../sources/typescript/prose/extract.md");
    pub const TYPESCRIPT_SURVEY: &str = include_str!("../sources/typescript/prose/survey.md");
}

fn answer() -> String {
    serde_json::json!({
        "claims": [{
            "kind": "requirement",
            "id": "orders.create",
            "path": "docs/orders.md#L3",
            "statement": "POST /orders creates an order."
        }]
    })
    .to_string()
}

fn tree(project: &Scratch, files: &[&str]) {
    for file in files {
        project.write(file, "");
    }
}

// One answer per workspace seam, and one for the inline value.
async fn extract(component: &str, project: &Scratch, seams: usize) -> ScriptedModel {
    let answer = answer();
    let answers = std::iter::repeat_n(answer.as_str(), seams + 1);
    support::run(component, project, &[], ScriptedModel::answering(answers)).await
}

async fn refused(
    component: &str, project: &Scratch, inline: Option<&str>, model: ScriptedModel,
) -> ScriptedModel {
    let mut args = vec!["refused", "bad_request"];
    args.extend(inline);
    support::run(component, project, &args, model).await
}

// Returns the workspace seams' turns.
fn prompted(model: &ScriptedModel, prompt: &str, seams: usize) -> Vec<String> {
    let seen = model.seen();
    assert_eq!(
        seen.len(),
        seams + 1,
        "metadata opens no completion; each seam opens one, the inline value one more"
    );
    for request in &seen {
        assert_eq!(request.system.as_deref(), Some(prompt), "the compiled-in prompt is the system");
    }
    seen[..seams].iter().map(|request| request.messages[0].clone()).collect()
}

// Every group appears together in exactly one turn.
fn partitioned(turns: &[String], groups: &[&[&str]]) {
    assert_eq!(turns.len(), groups.len(), "one turn per seam");
    for group in groups {
        let naming: Vec<&String> =
            turns.iter().filter(|turn| group.iter().any(|file| turn.contains(file))).collect();
        assert_eq!(naming.len(), 1, "{group:?} is one seam's alone, got {naming:?}");
        assert!(
            group.iter().all(|file| naming[0].contains(file)),
            "{group:?} is listed together: {}",
            naming[0]
        );
    }
}

// The two lines are the typescript adapter's own, so they are what the call is told.
fn surface(turn: &str) -> (&str, &str) {
    let name = turn
        .lines()
        .find_map(|line| line.strip_prefix("- surface: "))
        .expect("the turn names its surface");
    let entry = turn
        .lines()
        .find_map(|line| line.strip_prefix("- entry: `")?.strip_suffix('`'))
        .expect("the turn names its entry");
    (name, entry)
}

// A tree of one directory cuts no finer than itself.
#[tokio::test]
async fn documentation() {
    let project = scratch();
    project.write("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n");

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 1).await;

    prompted(&model, prompt::DOCUMENTATION, 1);
}

// The cut is the first path segment: `guide/advanced/` has two documents of its
// own but is no seam, and the root's own document folds in with a directory of one.
#[tokio::test]
async fn documentation_directories() {
    let project = scratch();
    tree(
        &project,
        &[
            "README.md",
            "api/orders.md",
            "api/users.md",
            "guide/advanced/setup.md",
            "guide/advanced/topics.md",
            "guide/intro.md",
            "notes/todo.md",
            ".github/workflows/ci.yml",
            "guide/.draft.md",
        ],
    );

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 3).await;

    let turns = prompted(&model, prompt::DOCUMENTATION, 3);
    partitioned(
        &turns,
        &[
            &["README.md", "notes/todo.md"],
            &["api/orders.md", "api/users.md"],
            &["guide/advanced/setup.md", "guide/advanced/topics.md", "guide/intro.md"],
        ],
    );
    for turn in &turns {
        assert!(
            !turn.contains(".github") && !turn.contains(".draft.md"),
            "a dot entry is in no seam: {turn}"
        );
    }
}

// A directory of more than sixteen documents is cut once more by its
// subdirectories, one level only and only where it can.
#[tokio::test]
async fn documentation_large_directory() {
    let v1: Vec<String> = (0..10).map(|i| format!("api/v1/endpoint-{i:02}.md")).collect();
    let v2: Vec<String> = (0..8).map(|i| format!("api/v2/endpoint-{i:02}.md")).collect();
    let v1: Vec<&str> = v1.iter().map(String::as_str).collect();
    let v2: Vec<&str> = v2.iter().map(String::as_str).collect();

    // nested: the directory's own documents and the folded subdirectory of one are a seam
    let project = scratch();
    let own = ["api/README.md", "api/CHANGELOG.md", "api/misc/glossary.md"];
    tree(&project, &own);
    tree(&project, &v1);
    tree(&project, &v2);
    tree(&project, &["guide/intro.md", "guide/setup.md"]);

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 4).await;

    let turns = prompted(&model, prompt::DOCUMENTATION, 4);
    partitioned(&turns, &[&own, &v1, &v2, &["guide/intro.md", "guide/setup.md"]]);

    // flat: nothing cuts, so the directory stays one seam whatever its size
    let flat: Vec<String> = (0..20).map(|i| format!("api/endpoint-{i:02}.md")).collect();
    let flat: Vec<&str> = flat.iter().map(String::as_str).collect();
    let project = scratch();
    tree(&project, &flat);
    tree(&project, &["guide/intro.md", "guide/setup.md"]);

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 2).await;

    let turns = prompted(&model, prompt::DOCUMENTATION, 2);
    partitioned(&turns, &[&flat, &["guide/intro.md", "guide/setup.md"]]);

    // a remainder of one joins the first subdirectory's seam
    let project = scratch();
    tree(&project, &["api/misc/glossary.md"]);
    tree(&project, &v1);
    tree(&project, &v2);
    tree(&project, &["guide/intro.md", "guide/setup.md"]);

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 3).await;

    let turns = prompted(&model, prompt::DOCUMENTATION, 3);
    let joined: Vec<&str> = v1.iter().copied().chain(["api/misc/glossary.md"]).collect();
    partitioned(&turns, &[&joined, &v2, &["guide/intro.md", "guide/setup.md"]]);
}

// The engine's own output beside the brief is not a file of the tree, so the
// tree is still one file.
#[tokio::test]
async fn intent() {
    let project = scratch();
    project.write("brief/intent.md", BRIEF);
    project.write("spec.md", "# Spec");
    project.write("design.md", "# Design");
    project.write(".omnia/store.json", "{}");

    let model = extract(test_programs::ADAPTER_INTENT, &project, 1).await;

    let turns = prompted(&model, prompt::INTENT, 1);
    assert!(turns[0].contains(BRIEF), "the brief read through the mount is the seam: {}", turns[0]);
    assert!(!turns[0].contains("# Spec"), "the projection is not the brief: {}", turns[0]);
}

#[tokio::test]
async fn intent_not_one_file() {
    let empty = scratch();
    let model =
        refused(test_programs::ADAPTER_INTENT, &empty, None, ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "no turn is spent on an empty tree");

    let several = scratch();
    several.write("one.md", "first");
    several.write("two.md", "second");
    let model =
        refused(test_programs::ADAPTER_INTENT, &several, None, ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "no turn is spent on a tree of several files");
}

// An intent source is never legitimately empty.
#[tokio::test]
async fn intent_empty_brief() {
    let project = scratch();
    project.write("intent.md", "\n\t \n");
    let model =
        refused(test_programs::ADAPTER_INTENT, &project, None, ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "no turn is spent on a blank file");

    let none = scratch();
    let model =
        refused(test_programs::ADAPTER_INTENT, &none, Some("  \n"), ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "no turn is spent on a blank value");
}

// A call mines a surface and never a file, so two surfaces entering at one
// module are two extracts; the inline value spends no survey turn.
#[tokio::test]
async fn typescript() {
    let project = scratch();
    tree(&project, &["src/index.ts", "src/routes.ts", "src/orders.ts", "src/jobs.ts", "src/db.ts"]);
    let inventory = r#"{"surfaces":[
        {"name":"POST /orders","entry":"src/routes.ts"},
        {"name":"GET /orders/:id","entry":"src/routes.ts"},
        {"name":"nightly reconciliation job","entry":"src/jobs.ts"}
    ]}"#;
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([inventory, &answer, &answer, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 5, "one survey, one extract per surface, one for the inline value");
    assert_eq!(
        seen[0].system.as_deref(),
        Some(prompt::TYPESCRIPT_SURVEY),
        "the compiled-in survey prompt is the system"
    );
    for request in &seen[1..] {
        assert_eq!(
            request.system.as_deref(),
            Some(prompt::TYPESCRIPT),
            "the compiled-in prompt is the system"
        );
    }
    let mut surfaces: Vec<_> =
        seen[1..4].iter().map(|request| surface(&request.messages[0])).collect();
    surfaces.sort_unstable();
    assert_eq!(
        surfaces,
        [
            ("GET /orders/:id", "src/routes.ts"),
            ("POST /orders", "src/routes.ts"),
            ("nightly reconciliation job", "src/jobs.ts"),
        ]
    );
}

// Every refused entry is present in the tree; the calls are cut from the
// inventory finally accepted, never the refused one.
#[tokio::test]
async fn typescript_non_production() {
    const REFUSED: [&str; 8] = [
        "services/mail.test.ts",
        "services/mail.spec.ts",
        "services/types.d.ts",
        "node_modules/left-pad/index.js",
        "dist/bundle.js",
        "tests/orders.e2e.ts",
        ".git/HEAD",
        "routes/README.md",
    ];
    let project = scratch();
    tree(
        &project,
        &["routes/orders.ts", "routes/users.ts", "services/index.ts", "services/mail.ts"],
    );
    tree(&project, &REFUSED);
    let surfaces: Vec<_> = REFUSED
        .iter()
        .enumerate()
        .map(|(index, entry)| serde_json::json!({ "name": format!("surface {index}"), "entry": entry }))
        .collect();
    let rejected = serde_json::json!({ "surfaces": surfaces }).to_string();
    let accepted = r#"{"surfaces":[{"name":"POST /orders","entry":"routes/orders.ts"}]}"#;
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([rejected.as_str(), accepted, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "two survey rounds, one extract for the one surface, one inline");
    let exchanges = model.exchanges();
    assert_eq!(exchanges[0].tool, "check");
    let correction = exchanges[0].outcome.as_ref().expect_err("no entry is production source");
    for entry in REFUSED {
        assert!(correction.contains(entry), "`{entry}` is a finding: {correction}");
    }
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the production module is accepted");
    assert_eq!(surface(&seen[2].messages[0]), ("POST /orders", "routes/orders.ts"));
}

// Mining a tree no caller reaches would raise what no caller observes into
// requirements; the empty inventory passes the check, so the refusal is the adapter's.
#[tokio::test]
async fn typescript_no_surface() {
    let project = scratch();
    tree(&project, &["src/lib/db.ts", "src/lib/logger.ts", "src/lib/format.ts"]);

    let model = refused(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        None,
        ScriptedModel::answering([r#"{"surfaces":[]}"#]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "the one survey turn, and no extract");
    assert_eq!(seen[0].system.as_deref(), Some(prompt::TYPESCRIPT_SURVEY));
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 1, "the inventory was offered to the check once");
    assert_eq!(exchanges[0].outcome, Ok(String::new()), "an empty inventory is a valid answer");
}
