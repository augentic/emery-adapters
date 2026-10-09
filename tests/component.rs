//! Verifies every shipped component through its world under the omnia runtime.
//!
//! One scenario per component, over what a native call cannot show: the
//! export, the lift and lower of the input and the answer, the metadata, the
//! lend resolved against the mount, and the parser running as a guest. What
//! each adapter decides is `source.rs`'s and `target.rs`'s, natively, except
//! `documentation`'s, asserted here whole: its `model-survey` arm is a Cargo
//! feature, which the workspace's `--all-features` turns on in the native
//! rlib, while the component build keeps the crate's default, the control.
//! The boundary's own rules are `probe.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use emery_sdk::survey::{Inventory, Surface};
use emery_sdk::{Doc, body};
use omnia_test::Seen;
use omnia_test::host::{Scratch, ScriptedModel, scratch};

// Every `sources/*` and `targets/*` component must have a matching test here.
test_programs::foreach_adapter!();
test_programs::foreach_target!();

const BRIEF: &str = "Let users reset passwords by email.";

// One requirement, unanchored, so one answer serves a workspace seam and the
// inline value alike.
const CLAIM: &str =
    r#"{"claims":[{"kind":"requirement","id":"orders.create","statement":"Creates an order."}]}"#;

const REPORT: &str =
    r#"{"covered": ["REQ-001"], "written": ["src/orders/index.ts", "src/index.ts"]}"#;
const PASSED: &str = r#"{"passed": true, "failures": []}"#;

// The survey answer a surveying adapter's first turn is scripted with: one
// surface at the anchor that registers it, nothing unreached.
fn inventory(name: &str, anchor: &str, stem: &str) -> String {
    let inventory = Inventory {
        surfaces: vec![Surface {
            name: name.to_owned(),
            anchor: anchor.to_owned(),
            stem: stem.to_owned(),
        }],
        unreached: Vec::new(),
    };
    serde_json::to_string(&inventory).expect("the SDK's inventory serialises")
}

// A run over `project` and the inline value: `turns` workspace turns, the
// mount lent to each, then the inline value's under this build's
// `extract.md`, lending nothing.
async fn extracted(
    component: &str, docs: &[Doc], project: &Scratch, model: ScriptedModel, turns: usize,
) -> Vec<Seen> {
    let model = support::run(component, project, &[], model).await;
    let seen = model.seen();
    assert_eq!(seen.len(), turns + 1, "the workspace turns, then the inline value's");
    let lent = model.lent();
    assert!(lent[..turns].iter().all(Option::is_some), "the mount is lent to every workspace turn");
    assert!(lent[turns].is_none(), "the inline value lends nothing");
    led(&seen[turns], docs, "extract.md");
    seen
}

fn led(request: &Seen, docs: &[Doc], prompt: &str) {
    let body = body(docs, prompt).unwrap_or_else(|| panic!("`{prompt}` is in the adapter's PROSE"));
    let system = request.system.as_deref().expect("a system prompt");
    assert!(system.starts_with(body), "the compiled-in `{prompt}` leads the system");
}

fn tree(project: &Scratch, files: &[&str]) {
    for file in files {
        project.write(file, "");
    }
}

// A `documentation` run over `project`: one answer per seam and one for the
// inline value, answering the workspace seams' turns under `extract.md`.
async fn documented(project: &Scratch, seams: usize) -> Vec<String> {
    let answers = std::iter::repeat_n(CLAIM, seams + 1);
    let seen = extracted(
        test_programs::ADAPTER_DOCUMENTATION,
        documentation::PROSE,
        project,
        ScriptedModel::answering(answers),
        seams,
    )
    .await;
    for request in &seen[..seams] {
        led(request, documentation::PROSE, "extract.md");
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

// A tree of one directory cuts no finer than itself.
#[tokio::test]
async fn documentation() {
    let project = scratch();
    project.write("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n");

    let turns = documented(&project, 1).await;

    assert_eq!(turns.len(), 1, "the whole tree is the one seam");
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

    let turns = documented(&project, 3).await;

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

    let turns = documented(&project, 4).await;

    partitioned(&turns, &[&own, &v1, &v2, &["guide/intro.md", "guide/setup.md"]]);

    // flat: nothing cuts, so the directory stays one seam whatever its size
    let flat: Vec<String> = (0..20).map(|i| format!("api/endpoint-{i:02}.md")).collect();
    let flat: Vec<&str> = flat.iter().map(String::as_str).collect();
    let project = scratch();
    tree(&project, &flat);
    tree(&project, &["guide/intro.md", "guide/setup.md"]);

    let turns = documented(&project, 2).await;

    partitioned(&turns, &[&flat, &["guide/intro.md", "guide/setup.md"]]);

    // a remainder of one joins the first subdirectory's seam
    let project = scratch();
    tree(&project, &["api/misc/glossary.md"]);
    tree(&project, &v1);
    tree(&project, &v2);
    tree(&project, &["guide/intro.md", "guide/setup.md"]);

    let turns = documented(&project, 3).await;

    let joined: Vec<&str> = v1.iter().copied().chain(["api/misc/glossary.md"]).collect();
    partitioned(&turns, &[&joined, &v2, &["guide/intro.md", "guide/setup.md"]]);
}

// The brief is read through the mount, so the file reaches the turn whole.
#[tokio::test]
async fn intent() {
    let project = scratch();
    project.write("brief/intent.md", BRIEF);

    let seen = extracted(
        test_programs::ADAPTER_INTENT,
        intent::PROSE,
        &project,
        ScriptedModel::answering([CLAIM, CLAIM]),
        1,
    )
    .await;

    assert!(seen[0].messages[0].contains(BRIEF), "the brief read through the mount is the seam");
}

// The survey turn opens first under `survey.md`, and the surface the code
// derives at the anchor the answer named reaches the seam: the parser and
// the recogniser ran as a guest over the lent tree.
#[tokio::test]
async fn typescript() {
    let project = scratch();
    project.write(
        "src/index.ts",
        "import express from \"express\";\nimport { ordersRouter } from \"./routes\";\n\nconst app = \
         express();\napp.use(\"/api\", ordersRouter());\napp.listen(3000);\n",
    );
    project.write(
        "src/routes.ts",
        "import { Router } from \"express\";\n\nexport function ordersRouter() {\n  const router = \
         Router();\n  router.post(\"/orders\", async (req, res) => {\n    res.json(req.body);\n  \
         });\n  return router;\n}\n",
    );
    let survey = inventory("POST /api/orders", "src/routes.ts#L5-L7", "orders");

    let seen = extracted(
        test_programs::ADAPTER_TYPESCRIPT,
        typescript::PROSE,
        &project,
        ScriptedModel::answering([&survey, CLAIM, CLAIM]),
        2,
    )
    .await;

    led(&seen[0], typescript::PROSE, "survey.md");
    let surface = "- Surface `POST /api/orders` — entry `src/routes.ts` — stem `orders`";
    assert!(
        seen[1].messages[0].contains(surface),
        "derived from the lent tree: {}",
        seen[1].messages[0]
    );
}

#[tokio::test]
async fn python() {
    let project = scratch();
    project.write("app/__init__.py", "");
    project.write(
        "app/main.py",
        "from fastapi import FastAPI\n\nfrom .routers import orders\n\napp = FastAPI()\n\
         app.include_router(orders.router, prefix=\"/api\")\n",
    );
    project.write("app/routers/__init__.py", "");
    project.write(
        "app/routers/orders.py",
        "from fastapi import APIRouter\n\nrouter = APIRouter(prefix=\"/orders\")\n\n\n\
         @router.post(\"\")\nasync def create(body: dict) -> dict:\n    return body\n",
    );
    let survey = inventory("POST /api/orders", "app/routers/orders.py#L6-L8", "orders");

    let seen = extracted(
        test_programs::ADAPTER_PYTHON,
        python::PROSE,
        &project,
        ScriptedModel::answering([&survey, CLAIM, CLAIM]),
        2,
    )
    .await;

    led(&seen[0], python::PROSE, "survey.md");
    let surface = "- Surface `POST /api/orders` — entry `app/routers/orders.py` — stem `orders`";
    assert!(
        seen[1].messages[0].contains(surface),
        "derived from the lent tree: {}",
        seen[1].messages[0]
    );
}

// The metadata lowers the merge rules; the build writes the mount through
// `write_files`, and its report and the verdict lift back to the driver.
#[tokio::test]
async fn typescript_target() {
    support::build(
        test_programs::TARGET_TYPESCRIPT_TARGET,
        &scratch(),
        &["rules", "package-lock.json=ours", "src/index.ts=union", "**/index.ts=union"],
        ScriptedModel::default(),
    )
    .await;

    let project = scratch();
    let written = serde_json::json!({ "files": [
        { "path": "src/orders/index.ts", "content": "export function createOrder() {}\n" },
        { "path": "src/index.ts", "content": "export * as orders from \"./orders/index.ts\";\n" },
    ]})
    .to_string();
    let model = ScriptedModel::answering([REPORT, PASSED]).calling(0, [("write_files", written)]);

    let model = support::build(test_programs::TARGET_TYPESCRIPT_TARGET, &project, &[], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "the build opens one completion, the verify one");
    led(&seen[0], typescript_target::PROSE, "build.md");
    led(&seen[1], typescript_target::PROSE, "verify.md");
    assert!(seen.iter().all(|request| request.workspace.is_some()), "the mount is lent to both");
    for path in ["src/orders/index.ts", "src/index.ts"] {
        assert!(project.read(path).is_some(), "the tree holds `{path}` written through the lend");
    }
}
