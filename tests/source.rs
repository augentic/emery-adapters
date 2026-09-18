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

/// The prompts each component embeds, as this build compiled them in.
mod prompt {
    pub const DOCUMENTATION: &str = include_str!("../sources/documentation/prose/extract.md");
    pub const INTENT: &str = include_str!("../sources/intent/prose/extract.md");
    pub const TYPESCRIPT: &str = include_str!("../sources/typescript/prose/extract.md");
    pub const TYPESCRIPT_SURVEY: &str = include_str!("../sources/typescript/prose/survey.md");
}

/// A gate-valid answer of claims alone.
///
/// The kind of source rides `metadata`, never the document.
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

/// Writes an empty file at each of `files` under `project`, directories made on the way.
fn tree(project: &Scratch, files: &[&str]) {
    for file in files {
        project.write(file, "");
    }
}

/// Runs workspace and inline extraction against `component`.
///
/// The model supplies one answer per workspace seam and one for inline input.
async fn extract(component: &str, project: &Scratch, seams: usize) -> ScriptedModel {
    let answer = answer();
    let answers = std::iter::repeat_n(answer.as_str(), seams + 1);
    support::run(component, project, &[], ScriptedModel::answering(answers)).await
}

/// Runs the driver's `refused bad_request` mode against `component`, over the workspace or `inline`.
async fn refused(
    component: &str, project: &Scratch, inline: Option<&str>, model: ScriptedModel,
) -> ScriptedModel {
    let mut args = vec!["refused", "bad_request"];
    args.extend(inline);
    support::run(component, project, &args, model).await
}

/// Asserts the completions the host saw and returns the workspace seams' turns.
///
/// `metadata` opened none; the workspace `extract` opened one per seam and
/// the inline value's one more, each with `prompt` as its system.
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

/// Asserts that `turns` preserve the partition described by `groups`.
///
/// Every group must appear together in exactly one turn.
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

/// Returns the surface a turn tells its call to mine, as `(name, entry)`.
///
/// The two lines are the typescript adapter's own, so they are what the
/// call is told.
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

// A tree of one directory cuts no finer than itself, so it is one seam, and
// an inline value is one: two completions, both under the extraction prompt.
#[tokio::test]
async fn documentation() {
    let project = scratch();
    project.write("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n");

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 1).await;

    prompted(&model, prompt::DOCUMENTATION, 1);
}

// The cut is the first path segment. Each top-level directory of two or more
// documents is one seam listing its documents, nested ones included —
// `guide/advanced/` has two of its own but is no seam; the root's own
// document and a directory of one fold into one more, so every document is
// in exactly one seam. A dot entry — a `.github/`, an editor's draft — is
// tooling, not documentation, and is in none.
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

// A top-level directory of more than sixteen documents is cut once more, by
// its subdirectories, so one directory cannot hold the run behind its one
// turn: each subdirectory of two or more is a seam, and the directory's own
// documents with those of a subdirectory of one are one more, adjacent — four
// seams here. The cut is one level only, and a subdirectory cuts a directory
// only where it can: a flat directory of twenty stays one seam beside `guide`,
// and a remainder of one — the lone `api/misc/` document, with no document
// directly beneath `api/` — joins the first subdirectory's seam rather than
// standing alone.
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

// The one adapter that reads its source inside the guest: the tree's one
// file, nested or not, is read through the mount into the turn's seam. The
// engine's own output beside it — a projection of the last revision, its
// store — is not a file of the tree, so the tree is still one file and the
// brief is the one read.
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

// No file, or several: a typed refusal before any model call.
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

// An intent source is never legitimately empty: a blank brief, in the one
// file or inline, is a typed refusal, never an empty success.
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

// The one adapter that surveys by model: a workspace `extract` opens one
// survey completion under the embedded survey prompt, then one extract
// completion per surface the inventory names — each telling its call the
// surface and the module a caller enters it at, however many surfaces enter
// at one module, since a call mines a surface and never a file — and then
// the inline value's own, with no survey turn spent on it.
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

// Dependencies, build output, tests, declaration files, dot entries, and a
// file this adapter does not read are not production source: an inventory
// entered at any of them — present in the tree though they are — goes back
// to the model as findings naming each, and the calls are cut from the
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

// A tree the model finds no surface in is refused after the one survey turn,
// never mined whole: a source no caller reaches is incomplete input or a
// failed discovery, and mining it would raise what no caller observes into
// requirements. The empty inventory passes the survey's check, so the
// refusal is the adapter's, not a spent budget.
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
