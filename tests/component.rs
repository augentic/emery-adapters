//! Verifies every shipped component through its world under the omnia runtime.
//!
//! One scenario per component, over what a native call cannot show: the
//! export, the lift and lower of the input and the answer, the metadata, the
//! lend resolved against the mount, and the parser running as a guest. What
//! each adapter decides is `source.rs`'s and `target.rs`'s, natively; the
//! boundary's own rules are `probe.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use emery_sdk::survey::{Inventory, Surface};
use emery_sdk::target::TargetAdapter;
use emery_sdk::{Doc, body};
use omnia_test::host::{Scratch, ScriptedModel, scratch};
use omnia_test::{Seen, SeenWorker};

// Every shipped adapter component must have a matching test here.
test_programs::foreach_adapter!();
test_programs::foreach_target!();

const BRIEF: &str = "Let users reset passwords by email.";

// One requirement, unanchored, so one answer serves a workspace seam and the
// inline value alike.
const CLAIM: &str =
    r#"{"claims":[{"kind":"requirement","id":"orders.create","statement":"Creates an order."}]}"#;

const REPORT: &str =
    r#"{"covered": ["REQ-001"], "written": ["src/orders/index.ts", "src/index.ts"]}"#;
const OMNIA_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/lib.rs"]}"#;
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

// The worker grant a target's `WORKER` lowers through the world onto each
// of its turns, as the host records it.
fn granted<A: TargetAdapter>(seen: &[Seen]) {
    let grant = A::WORKER.grant().map(SeenWorker::from);
    assert!(grant.is_some(), "the shipped target spells what its worker reaches");
    for request in seen {
        assert_eq!(request.worker, grant, "the turn carries the adapter's worker grant");
    }
}

// The survey turn opens first under `survey.md`, and the subject the code
// derives at the anchor the answer named reaches the seam: the outline
// reader ran as a guest over the lent tree.
#[tokio::test]
async fn emery_documentation() {
    let project = scratch();
    project.write("docs/orders.md", "# Orders\n\nPOST /orders creates an order.\n");
    let survey = inventory("Orders", "docs/orders.md#L1", "orders");

    let seen = extracted(
        test_programs::ADAPTER_EMERY_DOCUMENTATION,
        emery_documentation::PROSE,
        &project,
        ScriptedModel::answering([&survey, CLAIM, CLAIM]),
        2,
    )
    .await;

    led(&seen[0], emery_documentation::PROSE, "survey.md");
    assert!(
        seen[0].messages[0].contains("- `docs/orders.md` — 3 lines; prose alone"),
        "the outline read from the lent tree: {}",
        seen[0].messages[0]
    );
    let subject = "- `Orders` — `docs/orders.md#L1-L3` — stem `orders` — ids lead with `orders`";
    assert!(
        seen[1].messages[0].contains(subject),
        "derived from the lent tree: {}",
        seen[1].messages[0]
    );
}

// The brief is read through the mount, so the file reaches the turn whole.
#[tokio::test]
async fn emery_intent() {
    let project = scratch();
    project.write("brief/intent.md", BRIEF);

    let seen = extracted(
        test_programs::ADAPTER_EMERY_INTENT,
        emery_intent::PROSE,
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
async fn emery_typescript() {
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
        test_programs::ADAPTER_EMERY_TYPESCRIPT,
        emery_typescript::PROSE,
        &project,
        ScriptedModel::answering([&survey, CLAIM, CLAIM]),
        2,
    )
    .await;

    led(&seen[0], emery_typescript::PROSE, "survey.md");
    let surface = "- Surface `POST /api/orders` — entry `src/routes.ts` — stem `orders`";
    assert!(
        seen[1].messages[0].contains(surface),
        "derived from the lent tree: {}",
        seen[1].messages[0]
    );
}

#[tokio::test]
async fn emery_python() {
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
        test_programs::ADAPTER_EMERY_PYTHON,
        emery_python::PROSE,
        &project,
        ScriptedModel::answering([&survey, CLAIM, CLAIM]),
        2,
    )
    .await;

    led(&seen[0], emery_python::PROSE, "survey.md");
    let surface = "- Surface `POST /api/orders` — entry `app/routers/orders.py` — stem `orders`";
    assert!(
        seen[1].messages[0].contains(surface),
        "derived from the lent tree: {}",
        seen[1].messages[0]
    );
}

// The metadata lowers the merge rules; the init lays the root manifests in
// the lent tree with no turn, the build writes the mount through
// `write_files`, its turns carry the worker grant, and its report and the
// verdict lift back to the driver.
#[tokio::test]
async fn emery_node() {
    support::build(
        test_programs::TARGET_EMERY_NODE,
        &scratch(),
        &["metadata", "package-lock.json=ours", "src/index.ts=union", "**/index.ts=union"],
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

    let model = support::build(test_programs::TARGET_EMERY_NODE, &project, &[], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "the init opens none, the build one completion, the verify one");
    led(&seen[0], emery_node::PROSE, "build.md");
    led(&seen[1], emery_node::PROSE, "verify.md");
    assert!(seen.iter().all(|request| request.workspace.is_some()), "the mount is lent to both");
    granted::<emery_node::Adapter>(&seen);
    for path in ["src/orders/index.ts", "src/index.ts"] {
        assert!(project.read(path).is_some(), "the tree holds `{path}` written through the lend");
    }
    assert_eq!(
        project.read("package.json").as_deref(),
        Some(include_bytes!("../adapters/node/seed/package.json").as_slice()),
        "the root manifest reached the host's tree through the init"
    );
}

// The metadata lowers the seven rules in order; the init lays the template
// in the lent tree with no turn, the build writes a stem's list through
// `write_files`, its turns carry the worker grant, and its report and the
// verdict lift back to the driver, the verify's check finding the static
// files standing as the template spells them.
#[tokio::test]
async fn emery_omnia() {
    support::build(
        test_programs::TARGET_EMERY_OMNIA,
        &scratch(),
        &[
            "metadata",
            "Cargo.lock=ours",
            "Cargo.toml=union",
            "src/routes.rs=union",
            "src/messaging.rs=union",
            "crates/common/src/topic.rs=union",
            "crates/*/Cargo.toml=union",
            "crates/*/src/lib.rs=union",
        ],
        ScriptedModel::default(),
    )
    .await;

    let project = scratch();
    let written = serde_json::json!({ "files": [
        { "path": "crates/orders/src/lib.rs", "content": "//! The `orders` stem.\n\npub mod reading;\n" },
    ]})
    .to_string();
    let model =
        ScriptedModel::answering([OMNIA_REPORT, PASSED]).calling(0, [("write_files", written)]);

    let model = support::build(test_programs::TARGET_EMERY_OMNIA, &project, &[], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "the init opens none, the build one completion, the verify one");
    led(&seen[0], emery_omnia::PROSE, "build.md");
    led(&seen[1], emery_omnia::PROSE, "verify.md");
    assert!(seen.iter().all(|request| request.workspace.is_some()), "the mount is lent to both");
    granted::<emery_omnia::Adapter>(&seen);
    assert!(
        project.read("crates/orders/src/lib.rs").is_some(),
        "the tree holds the stem's list written through the lend"
    );
    let seeded = |path: &str| {
        omnia_template::entry(path).unwrap_or_else(|| panic!("`{path}` is a seed")).body.as_bytes()
    };
    for path in ["src/routes.rs", ".gitignore"] {
        assert_eq!(
            project.read(path).as_deref(),
            Some(seeded(path)),
            "`{path}` reached the host's tree through the init, as the template spells it"
        );
    }
    assert!(
        project.read(".github/workflows/ci.yaml").is_some(),
        "a seed under a hidden directory reached the host's tree"
    );
}
