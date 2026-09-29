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

    fn prompt(docs: &[Doc], path: &str) -> &'static str {
        body(docs, path).unwrap_or_else(|| panic!("`{path}` is in the adapter's `PROSE`"))
    }
}

// Unanchored, so one answer serves a workspace seam and the inline value alike;
// what the SDK holds a `path` to is its own suite's.
fn answer() -> String {
    claim("orders.create")
}

// A requirement under `id`'s stem, for a seam the typescript adapter holds to it.
fn claim(id: &str) -> String {
    serde_json::json!({
        "claims": [{ "kind": "requirement", "id": id, "statement": "Creates an order." }]
    })
    .to_string()
}

// A claim no stem holds, for the seams a concurrent run consumes in any order.
fn decision() -> String {
    serde_json::json!({ "claims": [{ "kind": "decision", "statement": "Orders are rows." }] })
        .to_string()
}

fn tree(project: &Scratch, files: &[&str]) {
    for file in files {
        project.write(file, "");
    }
}

fn modules(project: &Scratch, files: &[(&str, &str)]) {
    for (file, text) in files {
        project.write(file, *text);
    }
}

// `head`, then padding past the SDK's inline budget, so the tree is cut by stem.
fn bulk(project: &Scratch, file: &str, head: &str) {
    let line = "// padding\n";
    let count = usize::try_from(emery_sdk::INLINE_BYTES).expect("fits") / line.len() + 1;
    project.write(file, format!("{head}{}", line.repeat(count)));
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

// The surface lines are the typescript adapter's own, so they are what the
// call is told: each surface's name, entry, and stem, in the turn's order.
fn surfaces(turn: &str) -> Vec<(&str, &str, &str)> {
    turn.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("- Surface `")?;
            let (name, rest) = rest.split_once("` — entry `")?;
            let (entry, rest) = rest.split_once("` — stem `")?;
            let (stem, _) = rest.split_once("`:")?;
            Some((name, entry, stem))
        })
        .collect()
}

// The turn that names `surface`, among a run's workspace turns.
fn turn_for<'t>(turns: &'t [String], surface: &str) -> &'t String {
    let naming: Vec<&String> =
        turns.iter().filter(|turn| turn.contains(&format!("- Surface `{surface}`"))).collect();
    assert_eq!(naming.len(), 1, "`{surface}` is one seam's alone, got {naming:?}");
    naming[0]
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

// An Express service the way one is written: the entry mounts a router,
// the router registers two routes — one handled in place, one by a function
// passed by name — and the routes reach a service and its store.
const APP: [(&str, &str); 4] = [
    (
        "src/index.ts",
        "import express from \"express\";\nimport { ordersRouter } from \"./routes\";\n\nconst app = \
         express();\napp.use(\"/api\", ordersRouter());\napp.listen(3000);\n",
    ),
    (
        "src/routes.ts",
        "import { Router } from \"express\";\nimport { createOrder, findOrder } from \
         \"./orders\";\n\nexport function ordersRouter() {\n  const router = Router();\n  \
         router.post(\"/orders\", async (req, res) => {\n    res.json(await \
         createOrder(req.body));\n  });\n  router.get(\"/orders/:id\", findOrder);\n  return \
         router;\n}\n",
    ),
    (
        "src/orders.ts",
        "import { pool } from \"./db\";\n\nexport async function createOrder(input: unknown) {\n  \
         return { id: pool, input };\n}\n\nexport function findOrder(req: { params: { id: string } \
         }, res: { json(body: unknown): void }) {\n  res.json({ id: req.params.id, pool });\n}\n",
    ),
    ("src/db.ts", "export const pool = \"o-1\";\n"),
];

// A tree whose modules fit within the SDK's inline budget is one extract over
// every module, laid into the turn, told every surface the code registers and
// held to every stem; the conventional entry is the bootstrap when no manifest
// names one. Each surface line carries the id its requirements lead with —
// the stem alone for a stem's one surface; under a shared stem, the handler's
// name when one is passed, else the verb and the path past the resource — and
// the modules it reaches beyond its entry. The inline value spends one turn
// more.
#[tokio::test]
async fn typescript() {
    let project = scratch();
    modules(&project, &APP);

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/index.ts", "start"),
            ("POST /api/orders", "src/routes.ts", "orders"),
            ("GET /api/orders/:id", "src/routes.ts", "orders"),
        ]
    );
    for note in [
        "- Surface `start` — entry `src/index.ts` — stem `start`: the process bootstrap: what runs \
         before each handler is registered, and at shutdown; id `start`; reaches `src/routes.ts`.",
        "; through `express`; id `orders.post`; reaches `src/orders.ts`, `src/db.ts`.",
        "; through `express`; id `orders.find-order`; reaches `src/orders.ts`, `src/db.ts`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    laid(&turns[0], &["src/index.ts", "src/routes.ts", "src/orders.ts", "src/db.ts"], &[]);
    assert!(turns[0].contains("1|import express from \"express\";"), "numbered: {}", turns[0]);
}

// The seam's stems hold the call's ids: an id under another stem is the SDK's
// finding, naming the stems the adapter chose, and the corrected answer is the
// seam's; the inline value is held to none.
#[tokio::test]
async fn typescript_stem() {
    let project = scratch();
    modules(&project, &APP);
    let strayed = claim("reconciliation.nightly");
    let corrected = claim("orders.create");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([strayed.as_str(), corrected.as_str(), strayed.as_str()]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 3, "two rounds for the one seam, one for the inline value");
    let exchanges = model.exchanges();
    let correction = exchanges[0].outcome.as_ref().expect_err("the stray stem is refused");
    assert!(
        correction.contains("`reconciliation.nightly`")
            && correction.contains("`orders`")
            && correction.contains("`start`"),
        "the finding names the id and the seam's stems: {correction}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the inline value is held to no stem");
}

// A tree past the budget is one extract per stem over the modules its
// surfaces reach: the routes' seam reaches the service and the store from the
// router, never the entry; the bootstrap's reaches the router it mounts and
// stops there — and each surface line says so. A module past the budget is
// listed, the rest laid.
#[tokio::test]
async fn typescript_closure() {
    let project = scratch();
    modules(&project, &APP);
    bulk(&project, "src/db.ts", "export const pool = \"o-1\";\n");
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&neutral, &neutral, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 2);
    let start = turn_for(&turns, "start");
    assert_eq!(surfaces(start), [("start", "src/index.ts", "start")]);
    assert!(start.contains("; id `start`; reaches `src/routes.ts`."), "{start}");
    laid(start, &["src/index.ts", "src/routes.ts"], &["src/orders.ts", "src/db.ts"]);
    let orders = turn_for(&turns, "POST /api/orders");
    assert_eq!(
        surfaces(orders),
        [
            ("POST /api/orders", "src/routes.ts", "orders"),
            ("GET /api/orders/:id", "src/routes.ts", "orders"),
        ]
    );
    assert!(
        orders.contains("; id `orders.post`; reaches `src/orders.ts`, `src/db.ts`."),
        "{orders}"
    );
    laid(orders, &["src/routes.ts", "src/orders.ts"], &["src/index.ts"]);
    assert!(orders.contains("- `src/db.ts`"), "the store past the budget is listed: {orders}");
}

// The bootstrap constructs a class whose constructor builds a service and
// whose method registers the handler: the entry is reached and not
// followed, but what loading it constructs runs before any handler, so the
// service reaches the `start` turn as well as the handler's.
#[tokio::test]
async fn typescript_constructed() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"feed\",\"main\":\"src/start.ts\"}\n"),
            (
                "src/start.ts",
                "import { Main } from \"./main\";\n\nconst main = new Main();\nmain.start();\n",
            ),
            (
                "src/main.ts",
                "import { Consumer } from \"kafkajs\";\nimport { Stops } from \"./stops\";\n\nexport \
                 class Main {\n  private stops: Stops;\n\n  constructor() {\n    this.stops = new \
                 Stops();\n  }\n\n  start() {\n    const consumer = new Consumer();\n    \
                 consumer.on(\"message\", (raw: string) => this.stops.lookup(raw));\n  }\n}\n",
            ),
        ],
    );
    bulk(
        &project,
        "src/stops.ts",
        "export class Stops {\n  constructor() {\n    setTimeout(() => this.fetch(), 1000);\n  }\n\n  \
         fetch() {}\n\n  lookup(code: string) {\n    return code;\n  }\n}\n",
    );
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&neutral, &neutral, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 2);
    let start = turn_for(&turns, "start");
    assert_eq!(surfaces(start), [("start", "src/start.ts", "start")]);
    laid(start, &["src/start.ts", "src/main.ts"], &[]);
    assert!(
        start.contains("- `src/stops.ts`"),
        "the service the entry constructs is the bootstrap's to reach: {start}"
    );
    let message = turn_for(&turns, "Consumer.on(\"message\")");
    assert_eq!(surfaces(message), [("Consumer.on(\"message\")", "src/main.ts", "message")]);
    laid(message, &["src/main.ts"], &["src/start.ts"]);
    assert!(message.contains("- `src/stops.ts`"), "the handler reaches the service too: {message}");
}

// The manifest's `start` script names the bootstrap over the conventional
// entry, which here only re-exports; a router mounted with no prefix keeps
// its own path.
#[tokio::test]
async fn typescript_bootstrap() {
    let project = scratch();
    modules(
        &project,
        &[
            (
                "package.json",
                "{\"name\":\"shop\",\"scripts\":{\"start\":\"ts-node src/server.ts\"}}\n",
            ),
            ("src/index.ts", "export { ordersRouter } from \"./routes\";\n"),
            (
                "src/server.ts",
                "import express from \"express\";\nimport { ordersRouter } from \"./routes\";\n\n\
                 const app = express();\napp.use(ordersRouter());\napp.listen(3000);\n",
            ),
            (
                "src/routes.ts",
                "import { Router } from \"express\";\n\nexport function ordersRouter() {\n  const \
                 router = Router();\n  router.get(\"/orders/:id\", (req, res) => {\n    res.json({ \
                 id: req.params.id });\n  });\n  return router;\n}\n",
            ),
        ],
    );

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("start", "src/server.ts", "start"), ("GET /orders/:id", "src/routes.ts", "orders")]
    );
}

// A handler handed to what a package provides is a surface — a command, a
// worker — named by the call that registers it and stemmed by its literal;
// a hook on the worker folds into the worker's surface; a handler handed to
// the tree's own class (`bus.on`) is not a surface, nor is a call with no
// handler (`program.parse`). Under a stem two surfaces share, a handler
// passed by name tells its surface apart and the registering method tells
// the other.
#[tokio::test]
async fn typescript_callbacks() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"jobs\",\"main\":\"src/cli.ts\"}\n"),
            (
                "src/cli.ts",
                "import { Command } from \"commander\";\nimport { listInvoices, runImport } from \
                 \"./import\";\nimport { startWorker } from \"./worker\";\n\nconst program = new \
                 Command();\nprogram.command(\"import <file>\").description(\"Import a \
                 CSV\").action(runImport);\nprogram.command(\"invoices\").action(listInvoices);\n\
                 program.command(\"serve\").action(async () => {\n  await startWorker();\n});\n\
                 program.parse();\n",
            ),
            (
                "src/import.ts",
                "export async function runImport(file: string) {\n  console.log(file);\n}\n\nexport \
                 async function listInvoices() {\n  console.log(\"invoices\");\n}\n",
            ),
            (
                "src/worker.ts",
                "import { Worker } from \"bullmq\";\nimport { Bus } from \"./bus\";\n\nexport async \
                 function startWorker() {\n  const worker = new Worker(\"invoices\", async (job) => \
                 {\n    console.log(job.id);\n  });\n  worker.on(\"failed\", (job, error) => {\n    \
                 console.error(job?.id, error);\n  });\n  const bus = new Bus();\n  bus.on(\"paid\", \
                 (id: string) => {\n    console.log(id);\n  });\n  return worker;\n}\n",
            ),
            (
                "src/bus.ts",
                "export class Bus {\n  on(event: string, handler: (id: string) => void) {\n    \
                 handler(event);\n  }\n}\n",
            ),
        ],
    );
    let answer = claim("invoices.failed");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&answer, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/cli.ts", "start"),
            ("Command.command(\"import <file>\")", "src/cli.ts", "import"),
            ("Command.command(\"invoices\")", "src/cli.ts", "invoices"),
            ("Command.command(\"serve\")", "src/cli.ts", "serve"),
            ("Worker(\"invoices\")", "src/worker.ts", "invoices"),
        ]
    );
    for note in [
        "through `commander`; id `import`; reaches `src/import.ts`, `src/worker.ts`, `src/bus.ts`.",
        "through `commander`; id `invoices.list-invoices`; reaches",
        "through `commander`; id `serve`; reaches `src/worker.ts`, `src/import.ts`, `src/bus.ts`.",
        "hook `worker.on(\"failed\")` L8–L10; id `invoices.worker`; reaches `src/bus.ts`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    assert!(!turns[0].contains("hook `bus.on"), "the tree's own class hooks nothing: {}", turns[0]);
}

// A method under a package's decorator is a surface: a verb decorator maps a
// route under the class decorator's prefix, a shaping decorator marks none.
#[tokio::test]
async fn typescript_decorated() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"nest\",\"main\":\"src/main.ts\"}\n"),
            (
                "src/main.ts",
                "import { NestFactory } from \"@nestjs/core\";\nimport { OrdersController } from \
                 \"./orders.controller\";\n\nasync function bootstrap() {\n  const app = await \
                 NestFactory.create(OrdersController);\n  await app.listen(3000);\n}\nbootstrap();\n",
            ),
            (
                "src/orders.controller.ts",
                "import { Body, Controller, Get, HttpCode, Post } from \"@nestjs/common\";\nimport { \
                 OrdersService } from \"./orders.service\";\n\n@Controller(\"orders\")\nexport class \
                 OrdersController {\n  constructor(private readonly orders: OrdersService) {}\n\n  \
                 @Get(\":id\")\n  @HttpCode(200)\n  find(id: string) {\n    return \
                 this.orders.find(id);\n  }\n\n  @Post()\n  create(@Body() input: unknown) {\n    \
                 return this.orders.create(input);\n  }\n}\n",
            ),
            (
                "src/orders.service.ts",
                "export class OrdersService {\n  find(id: string) {\n    return { id };\n  }\n  \
                 create(input: unknown) {\n    return { input };\n  }\n}\n",
            ),
        ],
    );

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/main.ts", "start"),
            ("GET /orders/:id", "src/orders.controller.ts", "orders"),
            ("POST /orders", "src/orders.controller.ts", "orders"),
        ]
    );
    assert!(
        turns[0].contains("method `OrdersController.find` L8–L12"),
        "the method under two decorators is one surface: {}",
        turns[0]
    );
    for note in [
        "through `@nestjs/common`; id `orders.find`; reaches `src/orders.service.ts`.",
        "through `@nestjs/common`; id `orders.create`; reaches `src/orders.service.ts`.",
    ] {
        assert!(turns[0].contains(note), "the method tells the surface: {note}: {}", turns[0]);
    }
}

// A library's entry only declares and exports, so the tree has no bootstrap
// and what the entry exports — a function, a class, through a re-export —
// is its surfaces; a type and an error class are none. A class carries an id
// per public method beside its own.
#[tokio::test]
async fn typescript_exports() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"money\",\"main\":\"dist/index.js\"}\n"),
            (
                "src/index.ts",
                "export { Money } from \"./money\";\nexport { parse, ParseError } from \
                 \"./parse\";\nexport type { Currency } from \"./money\";\n",
            ),
            (
                "src/money.ts",
                "export type Currency = \"GBP\" | \"EUR\";\n\nexport class Money {\n  \
                 constructor(readonly amount: number, readonly currency: Currency) {}\n\n  add(other: \
                 Money) {\n    return new Money(this.amount + other.amount, this.currency);\n  }\n\n  \
                 private check() {}\n}\n",
            ),
            (
                "src/parse.ts",
                "import { Money } from \"./money\";\n\nexport class ParseError extends Error \
                 {}\n\nexport function parse(text: string) {\n  return new Money(Number(text), \
                 \"GBP\");\n}\n",
            ),
        ],
    );
    let answer = claim("parse.text");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&answer, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("Money", "src/money.ts", "money"), ("parse", "src/parse.ts", "parse")]
    );
    assert!(
        turns[0].contains(
            "methods `add` L6–L8; ids `money`, `money.add`; reaches nothing beyond its entry."
        ),
        "public methods only, each an id: {}",
        turns[0]
    );
    assert!(turns[0].contains("; id `parse`; reaches `src/money.ts`."), "{}", turns[0]);
}

// The `type` claims are the declarations the parser read — every exported
// interface, alias, enum, and class of the modules the seams reach, anchored
// at its lines; one a module keeps to itself is none — and a `type` the model
// answers gives way to them; an inline value's declarations are copied too,
// with no file to anchor in.
#[tokio::test]
async fn typescript_types() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"money\",\"main\":\"dist/index.js\"}\n"),
            ("src/index.ts", "export { Money, parse } from \"./money\";\n"),
            (
                "src/money.ts",
                "export type Currency = \"GBP\" | \"EUR\";\n\nexport enum Rounding {\n  Bankers,\n  \
                 HalfUp,\n}\n\ninterface Parts {\n  amount: number;\n  currency: Currency;\n}\n\n\
                 type Adjust = (money: Money) => Money;\n\nexport class Money {\n  \
                 constructor(readonly parts: Parts) {}\n\n  add(other: Money): Money {\n    return \
                 new Money({ ...this.parts, amount: this.parts.amount + other.parts.amount });\n  \
                 }\n}\n\nclass Ledger {\n  entries: Money[] = [];\n}\n\nexport function \
                 parse(text: string, adjust?: Adjust): Money {\n  const money = new Money({ amount: \
                 Number(text), currency: \"GBP\" });\n  return adjust ? adjust(money) : money;\n}\n",
            ),
        ],
    );
    let answered = serde_json::json!({
        "claims": [
            { "kind": "requirement", "id": "parse.text", "statement": "Parses an amount." },
            { "kind": "type", "name": "Phantom", "signature": "interface Phantom {}" }
        ]
    })
    .to_string();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &["types", "Currency", "Rounding", "Money"],
        ScriptedModel::answering([&answered, &answered]),
    )
    .await;

    assert_eq!(model.seen().len(), 2, "one seam over the tree, one over the value");
}

// The turn lists what the modules spell as values of their own — an
// environment read with its default, or with whatever the code does to it,
// a named constant at module level or inside a function, a pattern, a
// definition handed literals, listed by its head at its range when it runs
// over several lines, a class field — at its anchor, and the packages they
// import with the names bound to them; a construction, a reference to a
// value spelled elsewhere, and a function's working local are none.
#[tokio::test]
async fn typescript_boundaries() {
    let project = scratch();
    modules(&project, &APP);
    modules(
        &project,
        &[
            (
                "src/config.ts",
                "import { define } from \"schema\";\n\nexport const PORT = Number(process.env.PORT ?? \
                 3000);\nexport const MAX_LINES = 50;\nexport const SKU = \
                 /^[A-Z]{2,4}-\\d{3,6}$/;\nexport const API_TOKENS = (process.env.API_TOKENS ?? \
                 \"\").split(\",\").filter((token) => token.length > 0);\nexport const ORDER = \
                 define({\n  id: \"string\",\n  lines: \"number\",\n});\n\nexport class Limits {\n  \
                 static pageSize = Number(process.env.PAGE_SIZE ?? 20);\n}\n\nexport function \
                 listen(app: { listen(port: number): void }) {\n  app.listen(Number(process.env.PORT \
                 ?? 3000));\n}\n\nexport async function settle(sleep: (ms: number) => \
                 Promise<void>) {\n  const FIVE_SEC_DELAY = 5 * 1000;\n  const attempts = 0;\n  \
                 await sleep(FIVE_SEC_DELAY);\n  return attempts;\n}\n",
            ),
            (
                "src/index.ts",
                "import express from \"express\";\nimport { ordersRouter } from \"./routes\";\n\
                 import { PORT } from \"./config\";\n\nconst app = express();\nconst port = \
                 PORT;\napp.use(\"/api\", ordersRouter());\napp.listen(port);\n",
            ),
        ],
    );

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    for line in [
        "- `src/config.ts#L3` — `PORT = Number(process.env.PORT ?? 3000)`",
        "- `src/config.ts#L4` — `MAX_LINES = 50`",
        "- `src/config.ts#L5` — `SKU = /^[A-Z]{2,4}-\\d{3,6}$/`",
        "- `src/config.ts#L6` — `API_TOKENS = (process.env.API_TOKENS ?? \"\").split(\",\").filter((token) \
         => token.length > 0)`",
        "- `src/config.ts#L7-L10` — `ORDER = define({…`",
        "- `src/config.ts#L13` — `Limits.pageSize = Number(process.env.PAGE_SIZE ?? 20)`",
        "- `src/config.ts#L17` — `process.env.PORT` in `app.listen(Number(process.env.PORT ?? 3000));`",
        "- `src/config.ts#L21` — `FIVE_SEC_DELAY = 5 * 1000`",
        "- `express` — `express` (default), `Router` — in `src/index.ts`, `src/routes.ts`",
    ] {
        assert!(turns[0].contains(line), "{line} is in the brief: {}", turns[0]);
    }
    for absent in ["`app = express()`", "`port = PORT`", "`attempts = 0`"] {
        assert!(!turns[0].contains(absent), "{absent} is no boundary: {}", turns[0]);
    }
    assert!(
        !turns[0].contains("`process.env.API_TOKENS` in"),
        "a read a listed binding holds is not listed twice: {}",
        turns[0]
    );
}

// The turn lists the calls the modules make through a package, grouped by
// callee at their sites — a store's query wherever the repository makes it
// — and none of what is structure: the registration of a route, the mount,
// the listen, the construction bound at module level.
#[tokio::test]
async fn typescript_calls() {
    let project = scratch();
    modules(&project, &APP);
    modules(
        &project,
        &[
            (
                "src/db.ts",
                "import { Pool } from \"pg\";\n\nexport const pool = new Pool({ connectionString: \
                 process.env.DATABASE_URL });\n\nexport function replica() {\n  return new Pool({ \
                 connectionString: process.env.REPLICA_URL, max: 2 });\n}\n",
            ),
            (
                "src/orders.ts",
                "import { pool } from \"./db\";\n\nexport async function createOrder(input: unknown) \
                 {\n  const { rows } = await pool.query(\"INSERT INTO orders (input) VALUES ($1) \
                 RETURNING id\", [input]);\n  return rows[0];\n}\n\nexport async function findOrder(req: \
                 { params: { id: string } }, res: { json(body: unknown): void }) {\n  const { rows } = \
                 await pool.query(\"SELECT * FROM orders WHERE id = $1\", [req.params.id]);\n  \
                 res.json(rows[0]);\n}\n",
            ),
        ],
    );

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert!(
        turns[0].contains("- `pg:Pool.query` in `src/orders.ts` at L4, L9"),
        "the store's calls are one callee at its sites: {}",
        turns[0]
    );
    assert!(
        !turns[0].contains("`express:") && !turns[0].contains("`pg:Pool` in"),
        "registrations, mounts, and the constructions are no calls: {}",
        turns[0]
    );
}

// A `tsconfig` `paths` alias resolves like a relative import: the route is
// mounted through it, and its seam reaches the service it names through it.
// The config `extends` a base with no `compilerOptions`, which leaves the
// child's own in place.
#[tokio::test]
async fn typescript_paths() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"shop\",\"main\":\"src/server.ts\"}\n"),
            ("tsconfig.base.json", "{ \"include\": [\"src\"] }\n"),
            (
                "tsconfig.json",
                "{\n  \"extends\": \"./tsconfig.base.json\",\n  // aliases\n  \"compilerOptions\": \
                 {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@app/*\": [\"src/*\"], },\n  },\n}\n",
            ),
            (
                "src/server.ts",
                "import express from \"express\";\nimport { ordersRouter } from \
                 \"@app/routes/orders\";\n\nconst app = express();\napp.use(\"/api\", \
                 ordersRouter());\napp.listen(3000);\n",
            ),
            (
                "src/routes/orders.ts",
                "import { Router } from \"express\";\nimport { findOrder } from \
                 \"@app/services/orders\";\n\nexport function ordersRouter() {\n  const router = \
                 Router();\n  router.get(\"/orders/:id\", (req, res) => {\n    \
                 res.json(findOrder(req.params.id));\n  });\n  return router;\n}\n",
            ),
        ],
    );
    bulk(
        &project,
        "src/services/orders.ts",
        "export function findOrder(id: string) {\n  return { id };\n}\n",
    );
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&neutral, &neutral, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 2);
    let orders = turn_for(&turns, "GET /api/orders/:id");
    assert_eq!(surfaces(orders), [("GET /api/orders/:id", "src/routes/orders.ts", "orders")]);
    laid(orders, &["src/routes/orders.ts"], &["src/server.ts"]);
    assert!(orders.contains("- `src/services/orders.ts`"), "reached by alias: {orders}");
    assert!(!turn_for(&turns, "start").contains("services/orders.ts"), "{turns:?}");
}

// Tests, declarations, dependencies, build output, and dot entries are not
// modules: what they register is no surface, and — a test stating nothing
// included — they are neither laid nor listed.
#[tokio::test]
async fn typescript_non_production() {
    const REFUSED: [&str; 9] = [
        "src/orders.test.ts",
        "src/orders.spec.ts",
        "src/types.d.ts",
        "node_modules/left-pad/index.js",
        "dist/bundle.js",
        "test/cucumber/steps/orders.ts",
        "tests/orders.e2e.ts",
        ".git/HEAD",
        "README.md",
    ];
    let project = scratch();
    modules(&project, &APP);
    for file in REFUSED {
        project.write(
            file,
            "import express from \"express\";\nconst app = express();\napp.get(\"/refused\", (req, \
             res) => res.end());\n",
        );
    }

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert_eq!(surfaces(&turns[0]).len(), 3, "the app's three surfaces alone: {}", turns[0]);
    assert!(!turns[0].contains("/refused"), "nothing refused registers: {}", turns[0]);
    laid(&turns[0], &["src/index.ts"], &REFUSED);
}

// An Express service whose one handler decides: a guard, a switch, a throw,
// a catch, a timer, and a conditional it returns, over a named constant and
// a store.
const DECIDING: [(&str, &str); 3] = [
    (
        "src/index.ts",
        "import express from \"express\";\nimport { createOrder } from \"./orders\";\n\nconst app \
         = express();\napp.post(\"/orders\", createOrder);\napp.listen(3000);\n",
    ),
    (
        "src/orders.ts",
        "import { pool } from \"./db\";\n\nconst RETRY_MS = 500;\n\nexport async function \
         createOrder(req: { body: { sku?: string; qty: number } }, res: { status(code: number): { \
         json(body: unknown): void } }) {\n  if (!req.body.sku) {\n    res.status(400).json({ \
         error: \"sku-required\" });\n    return;\n  }\n  switch (req.body.qty) {\n    case 0:\n      \
         throw new Error(\"empty\");\n    default:\n      break;\n  }\n  try {\n    \
         res.status(201).json({ id: pool, sku: req.body.sku });\n  } catch (error) {\n    \
         setTimeout(() => createOrder(req, res), RETRY_MS);\n  }\n  return req.body.qty > 1 ? \
         \"bulk\" : \"single\";\n}\n",
    ),
    ("src/db.ts", "export const pool = \"o-1\";\n"),
];

// The points where the code decides — a guard, a switch, a throw, a catch,
// a timer, a conditional — are listed at their lines with their text and
// the function they run in, so a requirement anchors where a behaviour
// starts.
#[tokio::test]
async fn typescript_decisions() {
    let project = scratch();
    modules(&project, &DECIDING);

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    let sections: Vec<&str> = turns[0].split("\n\n").collect();
    let at = sections
        .iter()
        .position(|section| section.starts_with("Decision points"))
        .unwrap_or_else(|| panic!("the section is rendered: {}", turns[0]));
    let listed: Vec<&str> = sections[at + 1].lines().collect();
    assert_eq!(
        listed,
        [
            "- `src/orders.ts#L6-L9` — `if (!req.body.sku)` in `createOrder`",
            "- `src/orders.ts#L10-L15` — `switch (req.body.qty)` in `createOrder`",
            "- `src/orders.ts#L12` — `throw new Error(\"empty\");` in `createOrder`",
            "- `src/orders.ts#L18-L20` — `catch (error)` in `createOrder`",
            "- `src/orders.ts#L19` — `setTimeout(() => createOrder(req, res), RETRY_MS)` in \
             `createOrder`",
            "- `src/orders.ts#L21` — `req.body.qty > 1 ? …` in `createOrder`",
        ],
        "each decision at its lines, and the bootstrap decides nothing"
    );
}

// The seam's anchors hold every `requirement` to where the code's behaviour
// starts or its result is decided — a decision, a `return`, a call through
// a package, a boundary, a surface's lines: one anchored at a line that only
// imports is the SDK's finding, a `criterion` there is not, the corrected
// answer is accepted, and the inline value is held to no anchor.
#[tokio::test]
async fn typescript_anchors() {
    let project = scratch();
    modules(&project, &DECIDING);
    let guarded = serde_json::json!({
        "kind": "requirement", "id": "orders.sku-required",
        "statement": "An order without a sku is refused with 400.", "path": "src/orders.ts#L6-L9"
    });
    let returned = serde_json::json!({
        "kind": "requirement", "id": "orders.size",
        "statement": "An order of more than one unit is bulk.", "path": "src/orders.ts#L21"
    });
    let listening = serde_json::json!({
        "kind": "requirement", "id": "start.listen",
        "statement": "The app listens on port 3000.", "path": "src/index.ts#L6"
    });
    let wired = serde_json::json!({
        "kind": "requirement", "id": "orders.pool",
        "statement": "Orders use the pool.", "path": "src/orders.ts#L1"
    });
    let retry = serde_json::json!({
        "kind": "criterion", "id": "orders.retry-ms",
        "criterion": "RETRY_MS is 500 ms.", "path": "src/orders.ts#L1"
    });
    let strayed =
        serde_json::json!({ "claims": [&guarded, &wired, &listening, &retry, &returned] })
            .to_string();
    let corrected =
        serde_json::json!({ "claims": [&guarded, &listening, &retry, &returned] }).to_string();
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([strayed.as_str(), corrected.as_str(), inline.as_str()]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 3, "two rounds for the one seam, one for the inline value");
    let exchanges = model.exchanges();
    let correction = exchanges[0].outcome.as_ref().expect_err("the wired anchor is refused");
    assert!(
        correction.contains("claim 1: path `src/orders.ts#L1`"),
        "the finding names the claim at the import: {correction}"
    );
    for held in ["claim 0:", "claim 2:", "claim 3:", "claim 4:"] {
        assert!(!correction.contains(held), "{held} is at an anchor or not held: {correction}");
    }
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the inline value is held to no anchor");
}

// The tree's own tests are read for what they state: each running case
// under its suites, each scenario under its feature, listed at its line in
// the seam whose modules the test imports — a feature through the step
// modules beside it, a test importing no module of the tree in every seam
// — and the test file itself follows the seam's modules, laid or listed as
// they are; a skipped case states nothing.
#[tokio::test]
async fn typescript_stated() {
    let project = scratch();
    modules(&project, &APP);
    bulk(&project, "src/db.ts", "export const pool = \"o-1\";\n");
    modules(
        &project,
        &[
            (
                "test/orders.test.ts",
                "import { createOrder } from \"../src/orders\";\n\ndescribe(\"orders\", () => {\n  \
                 it(\"creates an order from the body\", async () => {\n    expect(await \
                 createOrder({ sku: \"a\" })).toBeDefined();\n  });\n  it.skip(\"rejects an empty \
                 body\", () => {});\n});\n",
            ),
            (
                "test/health.test.ts",
                "describe(\"health\", () => {\n  it(\"answers ok\", () => {});\n});\n",
            ),
            (
                "test/cucumber/features/orders.feature",
                "Feature: Orders\n  Scenario: An order is created\n    Given a body\n",
            ),
            (
                "test/cucumber/steps/orders.steps.ts",
                "import { pool } from \"../../../src/db\";\n\nexport function given() {\n  return \
                 pool;\n}\n",
            ),
        ],
    );
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&neutral, &neutral, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 2);
    let orders = turn_for(&turns, "POST /api/orders");
    assert!(
        orders.contains("- `test/orders.test.ts#L4` — orders › creates an order from the body"),
        "{orders}"
    );
    assert!(!orders.contains("rejects an empty body"), "a skipped case states nothing: {orders}");
    assert!(
        orders.contains(
            "- `test/cucumber/features/orders.feature#L2` — Orders › An order is created"
        ),
        "the feature follows the step module's imports: {orders}"
    );
    assert!(orders.contains("- `test/health.test.ts#L2` — health › answers ok"), "{orders}");
    assert!(
        orders.contains("- `test/orders.test.ts`"),
        "listed after the store past the budget: {orders}"
    );
    assert!(!orders.contains("orders.steps.ts"), "a step module states nothing: {orders}");
    let start = turn_for(&turns, "start");
    assert!(start.contains("- `test/health.test.ts#L2` — health › answers ok"), "{start}");
    assert!(
        !start.contains("test/orders.test.ts"),
        "the orders test imports no module of `start`: {start}"
    );
    assert!(!start.contains("orders.feature"), "{start}");
    laid(start, &["src/index.ts", "src/routes.ts", "test/health.test.ts"], &[]);
}

// *.config.* files are production modules: laid beside the app's own.
#[tokio::test]
async fn typescript_config_module() {
    let project = scratch();
    modules(&project, &APP);
    project.write(
        "vite.config.ts",
        "import { defineConfig } from \"vite\";\n\nexport default defineConfig({ plugins: [] });\n",
    );

    let model = extract(test_programs::ADAPTER_TYPESCRIPT, &project, 1).await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    laid(&turns[0], &["vite.config.ts", "src/index.ts"], &[]);
}

// A tree of one production module cuts no finer than itself: one extract
// over the module, listed when it is past the budget, its exports the
// surfaces.
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
    bulk(
        &project,
        "src/orders.ts",
        "export class OrderService {\n  create(input: unknown) {\n    return { input };\n  }\n}\n",
    );
    let answer = claim("order-service.create");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&answer, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert_eq!(surfaces(&turns[0]), [("OrderService", "src/orders.ts", "order-service")]);
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

// A tree with no bootstrap, no registration, no decorator, and no entry
// export is cut mechanically, never refused: within the budget, one extract
// over every module, told no surface was found and held to the one stem the
// manifest's package name gives.
#[tokio::test]
async fn typescript_no_surface() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"@acme/shared-helpers\"}\n"),
            ("src/lib/db.ts", "export const pool = 1;\n"),
            ("src/lib/config.ts", "export const retries = 3;\nexport type Mode = \"fast\";\n"),
        ],
    );
    let answer = claim("shared-helpers.pool");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&answer, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 1);
    assert!(surfaces(&turns[0]).is_empty(), "no surface is named: {}", turns[0]);
    assert!(turns[0].contains("No surface was found"), "{}", turns[0]);
    assert!(turns[0].contains("the one stem `shared-helpers`"), "{}", turns[0]);
    laid(&turns[0], &["src/lib/config.ts", "src/lib/db.ts"], &[]);
}

// Past the budget, a tree with no surface is one extract per top-level
// directory beneath `src/`, under the directory's name, the root's own
// modules joined to the first; a directory's seam lays its own modules and
// no other's.
#[tokio::test]
async fn typescript_no_surface_directories() {
    let project = scratch();
    modules(
        &project,
        &[
            ("src/index.ts", "export const version = \"1\";\n"),
            ("src/lib/config.ts", "export const retries = 3;\n"),
            ("src/util/format.ts", "export const SEPARATOR = \", \";\n"),
        ],
    );
    bulk(&project, "src/lib/db.ts", "export const pool = 1;\n");
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&neutral, &neutral, &answer]),
    )
    .await;

    let turns = prompted(&model, prompt::extract(typescript::PROSE), 2);
    let seam = |stem: &str| {
        let naming: Vec<&String> =
            turns.iter().filter(|turn| turn.contains(&format!("the stem `{stem}`"))).collect();
        assert_eq!(naming.len(), 1, "`{stem}` is one seam's alone, got {naming:?}");
        naming[0]
    };
    let lib = seam("lib");
    assert!(lib.contains("`src/lib/`, with the root's own modules"), "{lib}");
    laid(lib, &["src/index.ts", "src/lib/config.ts"], &["src/util/format.ts"]);
    assert!(lib.contains("- `src/lib/db.ts`"), "the store past the budget is listed: {lib}");
    let util = seam("util");
    laid(util, &["src/util/format.ts"], &["src/index.ts", "src/lib/config.ts", "src/lib/db.ts"]);
}
