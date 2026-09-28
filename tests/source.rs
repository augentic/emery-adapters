//! Verifies every shipped adapter through the component interface.
//!
//! Each adapter runs under the omnia runtime against a strict model script.
//! Assertions use adapter-owned source data rather than SDK prompt wording.
//! Shared SDK and component-boundary behaviour is covered by `probe.rs`.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use omnia_test::Seen;
use omnia_test::host::{Scratch, ScriptedModel, scratch};

// Every `sources/*` component must have a matching test here.
test_programs::foreach_adapter!();

const BRIEF: &str = "Let users reset passwords by email.";

// The prompts as this build compiled them in.
mod prompt {
    use emery_sdk::{Doc, body};

    pub fn extract(docs: &[Doc]) -> &'static str {
        prompt(docs, "extract.md")
    }

    pub fn survey(docs: &[Doc]) -> &'static str {
        prompt(docs, "survey.md")
    }

    fn prompt(docs: &[Doc], path: &str) -> &'static str {
        body(docs, path).unwrap_or_else(|| panic!("`{path}` is in the adapter's `PROSE`"))
    }
}

// Unanchored, so one answer serves a workspace seam and the inline value alike;
// what the SDK holds a `path` to is its own suite's.
fn answer() -> String {
    serde_json::json!({
        "claims": [{
            "kind": "requirement",
            "id": "orders.create",
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

// A module past the SDK's inline budget on its own, so the tree is surveyed.
fn bulk(project: &Scratch, file: &str) {
    let line = "export const pad = 0;\n";
    let count = usize::try_from(emery_sdk::INLINE_BYTES).expect("fits") / line.len() + 1;
    project.write(file, line.repeat(count));
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
        system(request, prompt);
    }
    seen[..seams].iter().map(|request| request.messages[0].clone()).collect()
}

// What the SDK appends after the prompt is the SDK's fact, asserted over the
// `gated` probe.
fn system(request: &Seen, prompt: &str) {
    let system = request.system.as_deref().expect("a system prompt");
    assert!(system.starts_with(prompt), "the compiled-in prompt leads the system");
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

// The line is the typescript adapter's own, so it is what the call is told.
fn surface(turn: &str) -> (&str, &str, &str) {
    turn.lines()
        .find_map(|line| {
            let rest = line.strip_prefix("Surface `")?.strip_suffix("`.")?;
            let (name, rest) = rest.split_once("` — entry `")?;
            let (entry, stem) = rest.split_once("` — stem `")?;
            Some((name, entry, stem))
        })
        .expect("the turn names its surface, entry, and stem")
}

// Every module is laid out in the turn, and nothing else is.
fn laid(turn: &str, modules: &[&str], refused: &[&str]) {
    for module in modules {
        assert!(turn.contains(&format!("### `{module}` (")), "`{module}` is laid out: {turn}");
    }
    for file in refused {
        assert!(!turn.contains(file), "`{file}` is not a module: {turn}");
    }
}

// A tree of one directory cuts no finer than itself.
#[tokio::test]
async fn documentation() {
    let project = scratch();
    project.write("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n");

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 1).await;

    prompted(&model, prompt::extract(documentation::PROSE), 1);
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

    let turns = prompted(&model, prompt::extract(documentation::PROSE), 3);
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

    let turns = prompted(&model, prompt::extract(documentation::PROSE), 4);
    partitioned(&turns, &[&own, &v1, &v2, &["guide/intro.md", "guide/setup.md"]]);

    // flat: nothing cuts, so the directory stays one seam whatever its size
    let flat: Vec<String> = (0..20).map(|i| format!("api/endpoint-{i:02}.md")).collect();
    let flat: Vec<&str> = flat.iter().map(String::as_str).collect();
    let project = scratch();
    tree(&project, &flat);
    tree(&project, &["guide/intro.md", "guide/setup.md"]);

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 2).await;

    let turns = prompted(&model, prompt::extract(documentation::PROSE), 2);
    partitioned(&turns, &[&flat, &["guide/intro.md", "guide/setup.md"]]);

    // a remainder of one joins the first subdirectory's seam
    let project = scratch();
    tree(&project, &["api/misc/glossary.md"]);
    tree(&project, &v1);
    tree(&project, &v2);
    tree(&project, &["guide/intro.md", "guide/setup.md"]);

    let model = extract(test_programs::ADAPTER_DOCUMENTATION, &project, 3).await;

    let turns = prompted(&model, prompt::extract(documentation::PROSE), 3);
    let joined: Vec<&str> = v1.iter().copied().chain(["api/misc/glossary.md"]).collect();
    partitioned(&turns, &[&joined, &v2, &["guide/intro.md", "guide/setup.md"]]);
}

// The engine's own output and the dot entries an editor or `git` leaves
// beside the brief are not files of the tree, so the tree is still one file.
#[tokio::test]
async fn intent() {
    let project = scratch();
    project.write("brief/intent.md", BRIEF);
    project.write("brief/.gitkeep", "");
    project.write(".DS_Store", "");
    project.write("spec.md", "# Spec");
    project.write("design.md", "# Design");
    project.write(".omnia/store.json", "{}");

    let model = extract(test_programs::ADAPTER_INTENT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(intent::PROSE), 1);
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

// A tree whose modules fit within the SDK's inline budget is one extract with
// every module laid into the turn and no survey turn spent; the inline value
// spends none either.
#[tokio::test]
async fn typescript() {
    const MODULES: [&str; 5] =
        ["src/index.ts", "src/routes.ts", "src/orders.ts", "src/jobs.ts", "src/db.ts"];
    let project = scratch();
    tree(&project, &MODULES);
    project.write("src/routes.ts", "export const routes = [];\n");

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert!(!turns[0].contains("Surface `"), "no surface was surveyed: {}", turns[0]);
    laid(&turns[0], &MODULES, &[]);
    assert!(turns[0].contains("1|export const routes = [];"), "numbered: {}", turns[0]);
}

// A tree past the budget is surveyed, and a call mines a surface and never a
// file, so two surfaces entering at one module are two extracts, each told its
// surface, entry, and stem over the whole lent tree.
#[tokio::test]
async fn typescript_surfaces() {
    const MODULES: [&str; 5] =
        ["src/index.ts", "src/routes.ts", "src/orders.ts", "src/jobs.ts", "src/db.ts"];
    let project = scratch();
    tree(&project, &MODULES);
    bulk(&project, "src/db.ts");
    let inventory = r#"{"surfaces":[
        {"name":"POST /orders","entry":"src/routes.ts","stem":"orders"},
        {"name":"GET /orders/:id","entry":"src/routes.ts","stem":"orders"},
        {"name":"nightly reconciliation job","entry":"src/jobs.ts","stem":"orders"}
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
        Some(prompt::survey(typescript::PROSE)),
        "the compiled-in survey prompt is the system"
    );
    for request in &seen[1..] {
        system(request, prompt::extract(typescript::PROSE));
    }
    let mut surfaces: Vec<_> =
        seen[1..4].iter().map(|request| surface(&request.messages[0])).collect();
    surfaces.sort_unstable();
    assert_eq!(
        surfaces,
        [
            ("GET /orders/:id", "src/routes.ts", "orders"),
            ("POST /orders", "src/routes.ts", "orders"),
            ("nightly reconciliation job", "src/jobs.ts", "orders"),
        ]
    );
    for request in &seen[1..4] {
        let turn = &request.messages[0];
        assert!(!turn.contains("laid out"), "a surface reads the lent tree: {turn}");
        assert!(!turn.contains("- `src/db.ts`"), "a surface lists no module: {turn}");
    }
}

// The surface's stem holds the call's ids: an id under another stem is the
// SDK's finding, and the corrected answer is the seam's.
#[tokio::test]
async fn typescript_stem() {
    let project = scratch();
    tree(&project, &["src/index.ts", "src/jobs.ts"]);
    bulk(&project, "src/index.ts");
    let inventory =
        r#"{"surfaces":[{"name":"nightly job","entry":"src/jobs.ts","stem":"reconciliation"}]}"#;
    let strayed = answer();
    let corrected = strayed.replace("orders.create", "reconciliation.nightly");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([
            inventory,
            strayed.as_str(),
            corrected.as_str(),
            strayed.as_str(),
        ]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "one survey, two rounds for the surface, one inline");
    let exchanges = model.exchanges();
    let correction = exchanges[1].outcome.as_ref().expect_err("the stray stem is refused");
    assert!(
        correction.contains("id `orders.create` leads with `orders`, not a stem of this seam"),
        "{correction}"
    );
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[3].outcome, Ok(String::new()), "the inline value is held to no stem");
}

// Every refused entry is present in the tree; the calls are cut from the
// inventory finally accepted, never the refused one.
#[tokio::test]
async fn typescript_non_production() {
    const REFUSED: [&str; 9] = [
        "services/mail.test.ts",
        "services/mail.spec.ts",
        "services/types.d.ts",
        "node_modules/left-pad/index.js",
        "dist/bundle.js",
        "test/cucumber/steps/orders.ts",
        "tests/orders.e2e.ts",
        ".git/HEAD",
        "routes/README.md",
    ];
    let project = scratch();
    tree(
        &project,
        &["routes/orders.ts", "routes/users.ts", "services/index.ts", "services/mail.ts"],
    );
    bulk(&project, "services/mail.ts");
    tree(&project, &REFUSED);
    let surfaces: Vec<_> = REFUSED
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            serde_json::json!({ "name": format!("surface {index}"), "entry": entry, "stem": "x" })
        })
        .collect();
    let rejected = serde_json::json!({ "surfaces": surfaces }).to_string();
    let accepted =
        r#"{"surfaces":[{"name":"POST /orders","entry":"routes/orders.ts","stem":"orders"}]}"#;
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
    assert_eq!(surface(&seen[2].messages[0]), ("POST /orders", "routes/orders.ts", "orders"));
    let turn = &seen[2].messages[0];
    for entry in REFUSED {
        assert!(!turn.contains(entry), "`{entry}` is no module of the call: {turn}");
    }
}

// *.config.* files are production modules: two modules are still one small
// tree, mined whole with both laid out.
#[tokio::test]
async fn typescript_config_module() {
    let project = scratch();
    tree(&project, &["vite.config.ts", "src/index.ts"]);

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    laid(&turns[0], &["vite.config.ts", "src/index.ts"], &[]);
}

// A tree of one production module cannot be cut, so no survey turn is spent
// and the one extract mines the tree whole, however large the module.
#[tokio::test]
async fn typescript_one_module() {
    const REFUSED: [&str; 7] = [
        "src/orders.test.ts",
        "src/types.d.ts",
        "dist/bundle.js",
        "node_modules/left-pad/index.js",
        "tests/orders.e2e.ts",
        ".git/HEAD",
        "README.md",
    ];
    let project = scratch();
    tree(&project, &REFUSED);
    bulk(&project, "src/orders.ts");

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert!(!turns[0].contains("Surface `"), "no surface was surveyed: {}", turns[0]);
    assert!(turns[0].contains("- `src/orders.ts`"), "the one module is listed: {}", turns[0]);
    for file in REFUSED {
        assert!(!turns[0].contains(file), "`{file}` is no module: {}", turns[0]);
    }
}

// A tree with no production module is refused before the model is reached.
#[tokio::test]
async fn typescript_no_module() {
    let project = scratch();
    tree(
        &project,
        &[
            "src/orders.test.ts",
            "src/types.d.ts",
            "dist/bundle.js",
            "node_modules/left-pad/index.js",
            "README.md",
        ],
    );

    let model =
        refused(test_programs::ADAPTER_TYPESCRIPT, &project, None, ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "no turn is spent on a tree with no module");
}

// Mining a tree no caller reaches would raise what no caller observes into
// requirements; the empty inventory passes the check, so the refusal is the adapter's.
#[tokio::test]
async fn typescript_no_surface() {
    let project = scratch();
    tree(&project, &["src/lib/db.ts", "src/lib/logger.ts", "src/lib/format.ts"]);
    bulk(&project, "src/lib/format.ts");

    let model = refused(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        None,
        ScriptedModel::answering([r#"{"surfaces":[]}"#]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "the one survey turn, and no extract");
    assert_eq!(seen[0].system.as_deref(), Some(prompt::survey(typescript::PROSE)));
    let exchanges = model.exchanges();
    assert_eq!(exchanges.len(), 1, "the inventory was offered to the check once");
    assert_eq!(exchanges[0].outcome, Ok(String::new()), "an empty inventory is a valid answer");
}
