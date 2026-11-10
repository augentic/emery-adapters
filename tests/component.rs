//! Exercises every shipped adapter component under the Omnia WebAssembly runtime.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use std::collections::BTreeSet;

use emery_sdk::survey::{Inventory, Surface};
use emery_sdk::target::{Grant, MergeStrategy, TargetAdapter};
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
const OMNIA_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/orders.rs", "crates/orders/tests/orders.rs"]}"#;
// The handler and test markers the omnia adapter scaffolds for the slice the
// driver hands it, and the bodies the scripted turn puts in their place.
const OMNIA_HANDLER_MARKER: &str =
    "    let _ = (&context, &input);\n    todo!(\"REQ-001 orders.create: the handler\")\n";
const OMNIA_HANDLER_FILLED: &str = "    let CreateInput {} = input;\n    let _ = \
                                    context.state().get(\"orders\").await?;\n    \
                                    Ok(Json(CreateOutput {}))\n";
const OMNIA_BOUND: &str = "where\n    P: Capabilities,\n";
const OMNIA_BOUND_NARROWED: &str = "where\n    P: StateStore,\n";
const OMNIA_SDK_USE: &str = "use omnia_sdk::{Capabilities, Result};\n";
const OMNIA_SDK_USE_NARROWED: &str = "use omnia_sdk::{Capabilities, Result, StateStore};\n";
const OMNIA_TEST_MARKER: &str = "#[ignore = \"REQ-001 An order is created: unfilled\"]\nasync fn \
                                 an_order_is_created() {\n    let request = \
                                 Request::post(\"/orders\")\n        .header(CONTENT_TYPE, \
                                 \"application/json\")\n        .body(Body::from(\"{}\"))\n        \
                                 .expect(\"request\");\n    let (status, body) = call(&provider(), \
                                 request).await;\n    let _ = (&status, &body);\n    \
                                 todo!(\"REQ-001 An order is created: arrange, act, assert\")\n}\n";
const OMNIA_TEST_FILLED: &str = "async fn an_order_is_created() {\n    assert_eq!(1 + 1, 2);\n}\n";
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

// The worker grant a target's `grants` lower through the world onto each of
// its turns, as the host records it.
fn granted<A: TargetAdapter>(seen: &[Seen]) {
    let grants = A::metadata().grants;
    assert_eq!(Grant::findings(grants), Vec::<String>::new());
    let grant = Grant::union(grants).map(SeenWorker::from);
    assert!(grant.is_some(), "the shipped target spells what its worker reaches");
    for request in seen {
        assert_eq!(request.worker, grant, "the turn carries the adapter's worker grant");
    }
}

// Every file the template scaffolds as a list is union-merged; static seed
// files are not; the glob rules are a stem's manifest and its entry.
fn omnia_union_rules_follow_template() {
    let union: Vec<&str> = emery_omnia::Adapter::metadata()
        .rules
        .iter()
        .filter(|rule| rule.strategy == MergeStrategy::Union)
        .map(|rule| rule.paths.as_ref())
        .collect();
    let listed: BTreeSet<&str> = omnia_template::ENTRIES
        .iter()
        .filter(|entry| entry.role == omnia_template::Role::Listed)
        .map(|entry| entry.path)
        .collect();

    for path in &listed {
        assert!(union.contains(path), "the listed `{path}` is merged as a union: {union:?}");
    }
    let (globs, plain): (Vec<&str>, Vec<&str>) = union.iter().partition(|rule| rule.contains('*'));
    assert_eq!(
        plain.into_iter().collect::<BTreeSet<_>>(),
        listed,
        "every union rule over one path is a file the template scaffolds as a list"
    );
    assert_eq!(
        globs,
        ["crates/*/Cargo.toml", "crates/*/src/lib.rs"],
        "the glob rules are a stem's manifest and its entry"
    );
    for entry in omnia_template::ENTRIES {
        if entry.role == omnia_template::Role::Static {
            assert!(
                !union.contains(&entry.path),
                "the static `{}` is merged under no rule",
                entry.path
            );
        }
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

// The metadata lowers the budget and the merge rules; the init scaffolds the
// root manifests in the lent tree with no turn, the build writes the mount
// through `write_files`, its turns carry the worker grant, and its report and
// the verdict lift back to the driver.
#[tokio::test]
async fn emery_node() {
    support::build(
        test_programs::TARGET_EMERY_NODE,
        &scratch(),
        &[
            "metadata",
            "budget=12",
            "package-lock.json=ours",
            "src/index.ts=union",
            "**/index.ts=union",
        ],
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

// The metadata lowers the budget and the six rules in order; the init
// scaffolds the template in the lent tree with no turn, the build scaffolds
// the slice's skeleton from the typed slice the driver lowered — the stem's
// crate and its lines in the seeded lists — before its turn, the handler from
// the typed route the driver lowered, which the turn fills with the test
// through `edit_files`, its turns carry the worker grant, and
// its report and the verdict lift back to the driver, the verify's check
// finding the static files standing as the template spells them.
#[tokio::test]
async fn emery_omnia() {
    support::build(
        test_programs::TARGET_EMERY_OMNIA,
        &scratch(),
        &[
            "metadata",
            "budget=12",
            "Cargo.lock=ours",
            "Cargo.toml=union",
            "src/lib.rs=union",
            "crates/common/src/topic.rs=union",
            "crates/*/Cargo.toml=union",
            "crates/*/src/lib.rs=union",
        ],
        ScriptedModel::default(),
    )
    .await;
    omnia_union_rules_follow_template();

    let project = scratch();
    let edits = serde_json::json!({ "edits": [
        { "path": "crates/orders/src/orders.rs", "find": OMNIA_HANDLER_MARKER, "replace": OMNIA_HANDLER_FILLED },
        { "path": "crates/orders/src/orders.rs", "find": OMNIA_BOUND, "replace": OMNIA_BOUND_NARROWED },
        { "path": "crates/orders/src/orders.rs", "find": OMNIA_SDK_USE, "replace": OMNIA_SDK_USE_NARROWED },
        { "path": "crates/orders/tests/orders.rs", "find": OMNIA_TEST_MARKER, "replace": OMNIA_TEST_FILLED },
    ]})
    .to_string();
    let model =
        ScriptedModel::answering([OMNIA_REPORT, PASSED]).calling(0, [("edit_files", edits)]);

    let model = support::build(test_programs::TARGET_EMERY_OMNIA, &project, &[], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "the init opens none, the build one completion, the verify one");
    led(&seen[0], emery_omnia::PROSE, "build.md");
    led(&seen[1], emery_omnia::PROSE, "verify.md");
    assert!(seen.iter().all(|request| request.workspace.is_some()), "the mount is lent to both");
    granted::<emery_omnia::Adapter>(&seen);
    assert!(
        seen[0].messages[0]
            .contains("What the adapter scaffolded for this slice before this turn:"),
        "the brief carries the scaffold section: {}",
        seen[0].messages[0]
    );
    for path in ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs"] {
        assert!(
            project.read(path).is_some(),
            "the tree holds `{path}`, scaffolded through the lend"
        );
    }
    let text = |path: &str| {
        String::from_utf8(project.read(path).unwrap_or_else(|| panic!("the tree holds `{path}`")))
            .expect("UTF-8")
    };
    let module = text("crates/orders/src/orders.rs");
    assert!(
        module.contains(OMNIA_HANDLER_FILLED) && !module.contains(OMNIA_HANDLER_MARKER),
        "the handler the guest scaffolded holds the body the turn filled: {module}"
    );
    for line in [
        "pub(crate) const ORDERS: &str = \"/orders\";\n",
        "    Handlers::new().route(ORDERS, post(create))\n",
        "pub(crate) async fn create<P>(\n    context: Context<P>, Json(input): \
         Json<CreateInput>,\n) -> Result<Json<CreateOutput>>\n",
    ] {
        assert!(
            module.contains(line),
            "the typed route the driver lowered reached the skeleton as `{line}`: {module}"
        );
    }
    let tests = text("crates/orders/tests/orders.rs");
    assert!(
        tests.contains(OMNIA_TEST_FILLED) && !tests.contains("#[ignore"),
        "the test the guest scaffolded is filled and run: {tests}"
    );
    assert!(
        text("src/lib.rs").contains(
            "    Handlers::from([\n        // one `<stem>::handlers(),` per stem, each added at \
             the end\n        orders::handlers(),\n    ])\n    \
             .with_state(Client::new(provider))\n"
        ),
        "the seeded root router holds the stem's entry the guest added under its comment"
    );
    let seeded = |path: &str| {
        omnia_template::entry(path).unwrap_or_else(|| panic!("`{path}` is a seed")).body.as_bytes()
    };
    for path in ["crates/common/src/lib.rs", ".gitignore"] {
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

// Two serial slices of one part built over one tree: the first scaffolds the
// part and lays its owned design type, and the second extends the held module
// and test file in place with its route, handler, and test without rewriting
// the first slice's code or creating a numbered module.
#[tokio::test]
async fn emery_omnia_serial() {
    let project = scratch();
    let first_edits = serde_json::json!({ "edits": [
        { "path": "crates/orders/src/orders.rs", "find": "REQ-001 orders.create: the handler", "replace": OMNIA_HANDLER_FILLED.trim_end() },
        { "path": "crates/orders/src/orders.rs", "find": OMNIA_BOUND, "replace": OMNIA_BOUND_NARROWED },
        { "path": "crates/orders/src/orders.rs", "find": OMNIA_SDK_USE, "replace": OMNIA_SDK_USE_NARROWED },
        { "path": "crates/orders/tests/orders.rs", "find": "REQ-001 An order is created: arrange, act, assert", "replace": OMNIA_TEST_FILLED.trim_end() },
    ]})
    .to_string();
    let second_edits = serde_json::json!({ "edits": [
        { "path": "crates/orders/src/orders.rs", "find": "REQ-002 orders.read: the handler", "replace": "    let _ = (&id, context.state().get(\"orders\").await?);\n    Ok(Json(ReadOutput {}))" },
        { "path": "crates/orders/src/orders.rs", "find": OMNIA_BOUND, "replace": OMNIA_BOUND_NARROWED },
        { "path": "crates/orders/tests/orders.rs", "find": "REQ-002 An order is read: arrange, act, assert", "replace": "async fn an_order_is_read() {\n    assert_eq!(2 + 2, 4);\n}" },
    ]})
    .to_string();
    let second_report = r#"{"covered": ["REQ-002"], "written": ["crates/orders/src/orders.rs", "crates/orders/tests/orders.rs"]}"#;
    let model = ScriptedModel::answering([OMNIA_REPORT, second_report, PASSED])
        .calling(0, [("edit_files", first_edits)])
        .calling(1, [("edit_files", second_edits)]);

    let model =
        support::build(test_programs::TARGET_EMERY_OMNIA, &project, &["serial"], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 3, "two build completions, then the verify");
    assert!(
        seen[1].messages[0].contains("Build slice 2 of 2 of part `orders` (PART-001) as SLICE-002"),
        "the second turn opens with the serial slice position: {}",
        seen[1].messages[0]
    );
    assert!(
        seen[1].messages[0].contains("- `crates/orders/src/orders.rs`: held, extended."),
        "the brief names the held module as extended: {}",
        seen[1].messages[0]
    );
    let text = |path: &str| {
        String::from_utf8(project.read(path).unwrap_or_else(|| panic!("the tree holds `{path}`")))
            .expect("UTF-8")
    };
    let module = text("crates/orders/src/orders.rs");
    for carried in [
        "pub struct Order {",
        "Handlers::new().route(ORDERS, post(create)).route(ORDERS_ID, get(read))",
        "pub(crate) async fn create<P>(",
        "pub(crate) async fn read<P>(context: Context<P>, Path(id): Path<String>) -> Result<Json<ReadOutput>>",
    ] {
        assert!(module.contains(carried), "the extended module carries `{carried}`: {module}");
    }
    let tests = text("crates/orders/tests/orders.rs");
    for carried in ["async fn an_order_is_created()", "async fn an_order_is_read()"] {
        assert!(tests.contains(carried), "the extended test file carries `{carried}`: {tests}");
    }
    assert!(!tests.contains("#[ignore"), "both scenario tests are filled: {tests}");
}
