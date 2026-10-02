//! Verifies every shipped adapter through the component interface.
//!
//! Each adapter runs under the omnia runtime against a strict model script.
//! Assertions use adapter-owned source data rather than SDK prompt wording.
//! Shared SDK and component-boundary behaviour is covered by `probe.rs`.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use std::path::Path;

use emery_sdk::survey::{Inventory, Surface};
use omnia_test::host::{Scratch, ScriptedModel, scratch};
use omnia_test::{Exchange, Seen};

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

// An adapter that has the model name its surfaces: its component, and the
// prose its turns open under.
#[derive(Clone, Copy)]
struct Surveying {
    component: &'static str,
    docs: &'static [emery_sdk::Doc],
}

const TYPESCRIPT: Surveying = Surveying {
    component: test_programs::ADAPTER_TYPESCRIPT,
    docs: typescript::PROSE,
};

const PYTHON: Surveying = Surveying {
    component: test_programs::ADAPTER_PYTHON,
    docs: python::PROSE,
};

// The survey answer a surveying workspace's first turn is scripted with:
// the SDK's inventory as the model would answer it — each surface named at
// the anchor that registers or declares it, under the stem its ids lead
// with — and the modules no surface reaches.
fn inventory(surfaces: &[(&str, &str, &str)], unreached: &[&str]) -> String {
    let inventory = Inventory {
        surfaces: surfaces
            .iter()
            .map(|&(name, anchor, stem)| Surface {
                name: name.to_owned(),
                anchor: anchor.to_owned(),
                stem: stem.to_owned(),
            })
            .collect(),
        unreached: unreached.iter().map(|&path| path.to_owned()).collect(),
    };
    serde_json::to_string(&inventory).expect("the SDK's inventory serialises")
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
    padded(project, file, head, "// padding\n");
}

// `bulk`, for a Python module.
fn py_bulk(project: &Scratch, file: &str, head: &str) {
    padded(project, file, head, "# padding\n");
}

fn padded(project: &Scratch, file: &str, head: &str, line: &str) {
    let count = usize::try_from(emery_sdk::INLINE_BYTES).expect("fits") / line.len() + 1;
    project.write(file, format!("{head}{}", line.repeat(count)));
}

// A committed fixture under `evals/cases/<name>/fixture`, copied whole into
// the scratch — less the `node_modules` and `dist` a checkout may hold — so
// the component runs over the real tree the eval's case of that name runs over.
fn fixture(project: &Scratch, name: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("evals/cases").join(name).join("fixture");
    copy_tree(&root, project, "");
}

fn copy_tree(from: &Path, project: &Scratch, under: &str) {
    let entries = std::fs::read_dir(from).unwrap_or_else(|e| panic!("{}: {e}", from.display()));
    for entry in entries {
        let entry = entry.expect("a readable directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if matches!(name.as_str(), "node_modules" | "dist") {
            continue;
        }
        let path = if under.is_empty() { name } else { format!("{under}/{name}") };
        if entry.file_type().expect("a file type").is_dir() {
            copy_tree(&entry.path(), project, &path);
        } else {
            project.write(&path, std::fs::read(entry.path()).expect("a readable fixture file"));
        }
    }
}

// One answer per workspace seam, and one for the inline value.
async fn extract(component: &str, project: &Scratch, seams: usize) -> ScriptedModel {
    let answer = answer();
    let answers = std::iter::repeat_n(answer.as_str(), seams + 1);
    support::run(component, project, &[], ScriptedModel::answering(answers)).await
}

// A surveying adapter's run over a workspace: the scripted survey answer,
// then one answer per seam, and one for the inline value.
async fn mined(
    adapter: Surveying, project: &Scratch, inventory: &str, seams: usize,
) -> ScriptedModel {
    let answer = answer();
    let answers = std::iter::once(inventory).chain(std::iter::repeat_n(answer.as_str(), seams + 1));
    support::run(adapter.component, project, &[], ScriptedModel::answering(answers)).await
}

async fn refused(
    component: &str, project: &Scratch, inline: Option<&str>, model: ScriptedModel,
) -> ScriptedModel {
    let mut args = vec!["refused", "bad_request"];
    args.extend(inline);
    support::run(component, project, &args, model).await
}

// Returns the workspace seams' turns of a run that surveys by code alone.
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

// Returns the workspace seams' turns of a surveying adapter's run, which the
// survey turn opens under `survey.md` — one per workspace, none for the
// inline value — before the seams' under `extract.md`.
fn surveyed(adapter: Surveying, model: &ScriptedModel, seams: usize) -> Vec<String> {
    let seen = model.seen();
    assert_eq!(
        seen.len(),
        seams + 2,
        "metadata opens no completion; the survey opens one, each seam one, the inline value one \
         more and no survey"
    );
    system(&seen[0], prompt::survey(adapter.docs));
    for request in &seen[1..] {
        system(request, prompt::extract(adapter.docs));
    }
    seen[1..=seams].iter().map(|request| request.messages[0].clone()).collect()
}

// The `check` rounds a surveying adapter's mining turns put, after the
// survey's — the run's first exchange, which accepted the scripted inventory
// at once.
fn checked(model: &ScriptedModel) -> Vec<Exchange> {
    let exchanges = model.exchanges();
    let (survey, mining) = exchanges.split_first().expect("the survey's check is recorded");
    assert_eq!(survey.tool, "check", "the survey's answer is checked before any seam opens");
    assert_eq!(survey.outcome, Ok(String::new()), "the inventory is accepted at once");
    mining.to_vec()
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

// The survey answer over `APP`: the two routes, each at its registration.
fn app_inventory() -> String {
    inventory(
        &[
            ("POST /api/orders", "src/routes.ts#L6-L8", "orders"),
            ("GET /api/orders/:id", "src/routes.ts#L9", "orders"),
        ],
        &[],
    )
}

// A workspace opens one survey turn before it is mined, and the inline value
// none. A tree whose modules fit within the SDK's inline budget is one extract
// over every module, laid into the turn, told every surface the survey named
// and held to every stem; the conventional entry is the bootstrap when no
// manifest names one, and the caller's `start` leads the surfaces. Each
// surface line carries what the code read at its anchor — the registration,
// its handler, the package — the id its requirements lead with (the stem
// alone for a stem's one surface; under a shared stem, the handler's name
// when one is passed, else the verb and the path past the resource) and the
// modules it reaches beyond its entry. The inline value spends one turn more.
#[tokio::test]
async fn typescript() {
    let project = scratch();
    modules(&project, &APP);

    let model = mined(TYPESCRIPT, &project, &app_inventory(), 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/index.ts", "start"),
            ("POST /api/orders", "src/routes.ts", "orders"),
            ("GET /api/orders/:id", "src/routes.ts", "orders"),
        ]
    );
    for note in [
        "- Surface `start` — entry `src/index.ts` — stem `start`: the process bootstrap, run at \
         load: what runs before each handler is registered, what it awaits before serving, and \
         at shutdown — `stop` and what a signal handler calls, wherever declared; id `start`; \
         reaches `src/routes.ts`.",
        ": registered L6–L8 in `ordersRouter`; handler L6–L8; through `express`; id `orders.post`; \
         reaches `src/orders.ts`, `src/db.ts`.",
        ": registered L9 in `ordersRouter`; through `express`; id `orders.find-order`; reaches \
         `src/orders.ts`, `src/db.ts`.",
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
    let survey = app_inventory();
    let strayed = claim("reconciliation.nightly");
    let corrected = claim("orders.create");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &strayed]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let exchanges = checked(&model);
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
    let survey = app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 2);
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
// whose method registers the handler the survey anchors: the entry is
// reached and not followed, but what loading it constructs runs before any
// handler, so the service reaches the `start` turn — and the handler's,
// which the code reads through the class the registration sits in.
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
    let survey = inventory(&[("message consumer", "src/main.ts#L13", "message")], &[]);
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 2);
    let start = turn_for(&turns, "start");
    assert_eq!(surfaces(start), [("start", "src/start.ts", "start")]);
    laid(start, &["src/start.ts", "src/main.ts"], &[]);
    assert!(
        start.contains("- `src/stops.ts`"),
        "the service the entry constructs is the bootstrap's to reach: {start}"
    );
    let message = turn_for(&turns, "message consumer");
    assert_eq!(surfaces(message), [("message consumer", "src/main.ts", "message")]);
    let note = ": registered L13 in `Main.start`; handler L13; through `kafkajs`; id `message`;";
    assert!(message.contains(note), "the registration is read in its class: {message}");
    laid(message, &["src/main.ts"], &["src/start.ts"]);
    assert!(message.contains("- `src/stops.ts`"), "the handler reaches the service too: {message}");
}

// The manifest's `start` script names the bootstrap over the conventional
// entry, which here only re-exports; a router mounted with no prefix keeps
// its own path. When `main` names that barrel too — the CommonJS shape, the
// package's exports beside the server its `start` runs — the bootstrap is
// the first entry named that runs, not the first that resolves, and the
// barrel is a module the survey lists as reached by no surface.
#[tokio::test]
async fn typescript_bootstrap() {
    const SOURCES: [(&str, &str); 3] = [
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
    ];
    for manifest in [
        "{\"name\":\"shop\",\"scripts\":{\"start\":\"ts-node src/server.ts\"}}\n",
        "{\"name\":\"shop\",\"main\":\"src/index.ts\",\"scripts\":{\"start\":\"ts-node \
         src/server.ts\"}}\n",
    ] {
        let project = scratch();
        modules(&project, &SOURCES);
        project.write("package.json", manifest);
        let survey =
            inventory(&[("GET /orders/:id", "src/routes.ts#L5-L7", "orders")], &["src/index.ts"]);

        let model = mined(TYPESCRIPT, &project, &survey, 1).await;

        let turns = surveyed(TYPESCRIPT, &model, 1);
        assert_eq!(
            surfaces(&turns[0]),
            [("start", "src/server.ts", "start"), ("GET /orders/:id", "src/routes.ts", "orders")],
            "under {manifest}"
        );
    }
}

// The survey over the committed `cli-jobs` fixture, the tree the eval case of
// that name runs over, answered as the eval expects it: within the budget, so
// one seam lays every module; the bootstrap the manifest's `main` names, four
// commands registered at module level, a schedule registered inside a
// function, and a worker. The code reads each anchor for what registers
// there — the chain's `command` literal, the schedule's constant, the
// worker's queue — and under the `nightly` stem the command and the schedule
// are told apart by the registering method.
#[tokio::test]
async fn typescript_fixture_cli_jobs() {
    let project = scratch();
    fixture(&project, "cli-jobs");
    let survey = inventory(
        &[
            ("import command", "src/cli.ts#L14-L24", "import"),
            ("reconcile command", "src/cli.ts#L26-L38", "reconcile"),
            ("nightly command", "src/cli.ts#L40-L45", "nightly"),
            ("serve command", "src/cli.ts#L47-L64", "serve"),
            ("nightly schedule", "src/jobs/nightly.ts#L34-L47", "nightly"),
            ("invoices worker", "src/workers/invoices.ts#L32-L57", "invoices"),
        ],
        &[],
    );
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/cli.ts", "start"),
            ("import command", "src/cli.ts", "import"),
            ("reconcile command", "src/cli.ts", "reconcile"),
            ("nightly command", "src/cli.ts", "nightly"),
            ("serve command", "src/cli.ts", "serve"),
            ("nightly schedule", "src/jobs/nightly.ts", "nightly"),
            ("invoices worker", "src/workers/invoices.ts", "invoices"),
        ]
    );
    for note in [
        "; id `start`; reaches `src/lib/db.ts`, `src/queues.ts`, `src/config.ts`, \
         `src/jobs/nightly.ts`, `src/services/importer.ts`, `src/services/reconcile.ts`, \
         `src/workers/invoices.ts`, `src/lib/csv.ts`.",
        "registered L47–L64 at module level; handler L50–L64; through `commander`; id `serve`;",
        "registered L35–L45 in `scheduleNightly`; handler L37–L43; through `node-cron`; id \
         `nightly.schedule`; reaches `src/config.ts`, `src/lib/db.ts`, `src/queues.ts`, \
         `src/services/reconcile.ts`.",
        "through `commander`; id `nightly.command`;",
        "registered L33–L48 in `startInvoiceWorker`; handler L35–L46; through `bullmq`; id \
         `invoices`; reaches `src/queues.ts`, `src/lib/db.ts`, `src/config.ts`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    laid(
        &turns[0],
        &[
            "src/cli.ts",
            "src/lib/db.ts",
            "src/queues.ts",
            "src/config.ts",
            "src/jobs/nightly.ts",
            "src/services/importer.ts",
            "src/services/reconcile.ts",
            "src/workers/invoices.ts",
            "src/lib/csv.ts",
        ],
        &["package.json", "tsconfig.json"],
    );
}

// The `express-orders` bootstrap's closure in the order the survey lays it:
// the entry, then every module it imports breadth-first — the three routers
// it mounts reached and stopped at, the services and stores it constructs
// followed. The postcode table is where the SDK's budget runs out, so the
// two modules after it are listed rather than laid.
const EXPRESS_START: [&str; 20] = [
    "src/server.ts",
    "src/config.ts",
    "src/lib/cache.ts",
    "src/lib/shipping.ts",
    "src/services/customers.ts",
    "src/repositories/customers.ts",
    "src/services/orders.ts",
    "src/repositories/orders.ts",
    "src/middleware/request-log.ts",
    "src/routes/health.ts",
    "src/middleware/auth.ts",
    "src/routes/customers.ts",
    "src/routes/orders.ts",
    "src/middleware/errors.ts",
    "src/lib/errors.ts",
    "src/data/postcodes.ts",
    "src/domain/order.ts",
    "src/schemas/customers.ts",
    "src/schemas/orders.ts",
    "src/services/pricing.ts",
];
const EXPRESS_LAID: usize = 18;

// The `orders` router's closure, from its entry.
const EXPRESS_ORDERS: [&str; 14] = [
    "src/routes/orders.ts",
    "src/schemas/orders.ts",
    "src/services/orders.ts",
    "src/config.ts",
    "src/domain/order.ts",
    "src/lib/cache.ts",
    "src/lib/errors.ts",
    "src/lib/shipping.ts",
    "src/repositories/orders.ts",
    "src/services/customers.ts",
    "src/services/pricing.ts",
    "src/data/postcodes.ts",
    "src/repositories/customers.ts",
    "src/schemas/customers.ts",
];

// The `customers` router's closure, from its entry.
const EXPRESS_CUSTOMERS: [&str; 6] = [
    "src/routes/customers.ts",
    "src/services/customers.ts",
    "src/schemas/customers.ts",
    "src/domain/order.ts",
    "src/lib/errors.ts",
    "src/repositories/customers.ts",
];

// The survey over the committed `express-orders` fixture, answered as the
// eval expects it: past the budget, so one seam per stem. The bootstrap's
// seam lays its closure in import order up to where the budget runs out and
// lists the rest; each router's seam lays its own closure and nothing of the
// entry's. Every route's stem is the code's, read from its path under the
// router's mount, and each is told apart by its verb and the path past the
// resource.
#[tokio::test]
async fn typescript_fixture_express_orders() {
    let project = scratch();
    fixture(&project, "express-orders");
    let survey = inventory(
        &[
            ("GET /api/orders", "src/routes/orders.ts#L17-L25", "orders"),
            ("GET /api/orders/:id", "src/routes/orders.ts#L27-L33", "orders"),
            ("POST /api/orders", "src/routes/orders.ts#L35-L42", "orders"),
            ("PUT /api/orders/:id/lines", "src/routes/orders.ts#L44-L51", "orders"),
            ("POST /api/orders/:id/pay", "src/routes/orders.ts#L53-L59", "orders"),
            ("POST /api/orders/:id/ship", "src/routes/orders.ts#L61-L67", "orders"),
            ("POST /api/orders/:id/cancel", "src/routes/orders.ts#L69-L76", "orders"),
            ("GET /api/customers/:id", "src/routes/customers.ts#L17-L22", "customers"),
            ("POST /api/customers", "src/routes/customers.ts#L24-L31", "customers"),
            ("PUT /api/customers/:id/tier", "src/routes/customers.ts#L33-L39", "customers"),
            ("GET /health/live", "src/routes/health.ts#L22-L24", "health"),
            ("GET /health/ready", "src/routes/health.ts#L26-L38", "health"),
        ],
        &[],
    );
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 4);
    let start = turn_for(&turns, "start");
    assert_eq!(surfaces(start), [("start", "src/server.ts", "start")]);
    let reached: Vec<String> = EXPRESS_START[1..].iter().map(|path| format!("`{path}`")).collect();
    assert!(
        start.contains(&format!("; id `start`; reaches {}.", reached.join(", "))),
        "the bootstrap reaches the tree in import order: {start}"
    );
    laid(start, &EXPRESS_START[..EXPRESS_LAID], &[]);
    for listed in &EXPRESS_START[EXPRESS_LAID..] {
        assert!(start.contains(&format!("- `{listed}`\n")), "`{listed}` is listed: {start}");
        assert!(!start.contains(&format!("### `{listed}`")), "`{listed}` is not laid: {start}");
    }

    let orders = turn_for(&turns, "GET /api/orders");
    assert_eq!(
        surfaces(orders),
        [
            ("GET /api/orders", "src/routes/orders.ts", "orders"),
            ("GET /api/orders/:id", "src/routes/orders.ts", "orders"),
            ("POST /api/orders", "src/routes/orders.ts", "orders"),
            ("PUT /api/orders/:id/lines", "src/routes/orders.ts", "orders"),
            ("POST /api/orders/:id/pay", "src/routes/orders.ts", "orders"),
            ("POST /api/orders/:id/ship", "src/routes/orders.ts", "orders"),
            ("POST /api/orders/:id/cancel", "src/routes/orders.ts", "orders"),
        ]
    );
    for note in [
        "registered L17–L25 in `ordersRouter`; handler L19–L24; through `express`; id `orders.get`;",
        "id `orders.get-id`;",
        "id `orders.put-id-lines`;",
        "id `orders.post-id-cancel`;",
    ] {
        assert!(orders.contains(note), "{note} is in the brief: {orders}");
    }
    laid(
        orders,
        &EXPRESS_ORDERS,
        &["src/server.ts", "src/routes/health.ts", "src/routes/customers.ts", "src/middleware/"],
    );

    let customers = turn_for(&turns, "GET /api/customers/:id");
    assert_eq!(
        surfaces(customers),
        [
            ("GET /api/customers/:id", "src/routes/customers.ts", "customers"),
            ("POST /api/customers", "src/routes/customers.ts", "customers"),
            ("PUT /api/customers/:id/tier", "src/routes/customers.ts", "customers"),
        ]
    );
    laid(
        customers,
        &EXPRESS_CUSTOMERS,
        &["src/server.ts", "src/services/orders.ts", "src/routes/orders.ts"],
    );

    let health = turn_for(&turns, "GET /health/live");
    assert_eq!(
        surfaces(health),
        [
            ("GET /health/live", "src/routes/health.ts", "health"),
            ("GET /health/ready", "src/routes/health.ts", "health"),
        ]
    );
    assert!(
        health.contains("id `health.get-ready`; reaches nothing beyond its entry."),
        "the probes reach no module: {health}"
    );
    laid(health, &["src/routes/health.ts"], &["src/server.ts", "src/config.ts"]);
}

// The commands, the worker, and the tree's own bus of a CLI: what the code
// reads at each anchor the survey names.
const JOBS: [(&str, &str); 5] = [
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
        "export async function runImport(file: string) {\n  console.log(file);\n}\n\nexport async \
         function listInvoices() {\n  console.log(\"invoices\");\n}\n",
    ),
    (
        "src/worker.ts",
        "import { Worker } from \"bullmq\";\nimport { Bus } from \"./bus\";\n\nexport async \
         function startWorker() {\n  const worker = new Worker(\"invoices\", async (job) => {\n    \
         console.log(job.id);\n  });\n  worker.on(\"failed\", (job, error) => {\n    \
         console.error(job?.id, error);\n  });\n  const bus = new Bus();\n  bus.on(\"paid\", (id: \
         string) => {\n    console.log(id);\n  });\n  return worker;\n}\n",
    ),
    (
        "src/bus.ts",
        "export class Bus {\n  on(event: string, handler: (id: string) => void) {\n    \
         handler(event);\n  }\n}\n",
    ),
];

// A registration the survey anchors is read for what it spells: the chain's
// `command` literal gives the stem, in place of the survey's where the two
// differ; a worker's queue gives its own. Under a stem two surfaces share, a
// handler passed by name tells its surface apart and the registering method
// tells the other. A hook on the worker (`worker.on("failed")`) the survey
// left unnamed is no surface, and neither is the handler handed to the
// tree's own class (`bus.on`).
#[tokio::test]
async fn typescript_callbacks() {
    let project = scratch();
    modules(&project, &JOBS);
    let survey = inventory(
        &[
            ("import command", "src/cli.ts#L6", "ledger-import"),
            ("invoices command", "src/cli.ts#L7", "invoices"),
            ("serve command", "src/cli.ts#L8-L10", "serve"),
            ("invoices worker", "src/worker.ts#L5-L7", "invoices"),
        ],
        &[],
    );
    let answer = claim("invoices.worker");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/cli.ts", "start"),
            ("import command", "src/cli.ts", "import"),
            ("invoices command", "src/cli.ts", "invoices"),
            ("serve command", "src/cli.ts", "serve"),
            ("invoices worker", "src/worker.ts", "invoices"),
        ]
    );
    for note in [
        ": registered L6 at module level; through `commander`; id `import`; reaches \
         `src/import.ts`, `src/worker.ts`, `src/bus.ts`.",
        "through `commander`; id `invoices.list-invoices`; reaches",
        ": registered L8–L10 at module level; handler L8–L10; through `commander`; id `serve`; \
         reaches `src/worker.ts`, `src/import.ts`, `src/bus.ts`.",
        ": registered L5–L7 in `startWorker`; handler L5–L7; through `bullmq`; id \
         `invoices.worker`; reaches `src/bus.ts`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
}

// The survey turn carries what the parser read of the tree, for the model to
// name its surfaces from: the manifest, the bootstrap, every call outside a
// handler that hands a function to something a package provides — with the
// literal that led it and the package — and the production modules; the
// manifest and the modules that locate a surface are laid into it whole.
#[tokio::test]
async fn typescript_survey_facts() {
    let project = scratch();
    modules(&project, &JOBS);
    let survey = inventory(&[("import command", "src/cli.ts#L6", "import")], &[]);
    let answer = claim("import.run-import");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let survey = &seen[0].messages[0];
    for fact in [
        "The manifest names the package `jobs`; `main` is `src/cli.ts`.",
        "The bootstrap — the entry that runs something — is `src/cli.ts`: it runs or constructs \
         the application at load.",
        "- `src/cli.ts#L6` — `program.action` led by `\"import <file>\"` handed a function, through \
         `commander`",
        "- `src/cli.ts#L8-L10` — `program.action` led by `\"serve\"` handed a function, through \
         `commander`",
        "- `src/worker.ts#L5-L7` — `new Worker` led by `\"invoices\"` handed a function, through \
         `bullmq`",
        "- `src/worker.ts#L8-L10` — `worker.on` led by `\"failed\"` handed a function, through \
         `bullmq` as `Worker`",
        "- `src/bus.ts`",
    ] {
        assert!(survey.contains(fact), "{fact} is among the facts: {survey}");
    }
    assert!(!survey.contains("`bus.on`"), "the tree's own receiver is no fact: {survey}");
    assert!(!survey.contains("`program.parse`"), "a call handed no function is none: {survey}");
    let at = |file: &str| {
        survey
            .find(&format!("### `{file}` ("))
            .unwrap_or_else(|| panic!("`{file}` is laid: {survey}"))
    };
    assert!(
        at("package.json") < at("src/cli.ts") && at("src/cli.ts") < at("src/worker.ts"),
        "the manifest, the bootstrap, then the modules that locate a surface: {survey}"
    );
    assert!(seen[0].workspace.is_some(), "the tree is lent to the survey");
    assert!(seen[2].workspace.is_none(), "the inline value lends nothing and is not surveyed");
}

// The survey's stem stands where the code spells none at the anchor — a
// route on `/`, a schedule led by a pattern — and gives way where it does:
// a route's resource under its mount replaces the survey's reading, so the
// seam's stems are the code's.
#[tokio::test]
async fn typescript_survey_stem_derived() {
    let project = scratch();
    modules(&project, &APP[1..]);
    modules(
        &project,
        &[
            (
                "src/index.ts",
                "import express from \"express\";\nimport cron from \"node-cron\";\nimport { \
                 ordersRouter } from \"./routes\";\nimport { sweep } from \"./sweep\";\n\nconst app \
                 = express();\napp.get(\"/\", (req, res) => {\n  res.json({ ok: true \
                 });\n});\napp.use(\"/api\", ordersRouter());\ncron.schedule(\"0 2 * * *\", \
                 sweep);\napp.listen(3000);\n",
            ),
            ("src/sweep.ts", "export function sweep() {\n  console.log(\"swept\");\n}\n"),
        ],
    );
    let survey = inventory(
        &[
            ("GET /", "src/index.ts#L7-L9", "home"),
            ("POST /api/orders", "src/routes.ts#L6-L8", "api-orders"),
            ("GET /api/orders/:id", "src/routes.ts#L9", "api-orders"),
            ("nightly sweep", "src/index.ts#L11", "sweep"),
        ],
        &[],
    );

    let model = mined(TYPESCRIPT, &project, &survey, 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/index.ts", "start"),
            ("GET /", "src/index.ts", "home"),
            ("POST /api/orders", "src/routes.ts", "orders"),
            ("GET /api/orders/:id", "src/routes.ts", "orders"),
            ("nightly sweep", "src/index.ts", "sweep"),
        ]
    );
    for id in ["id `home`;", "id `orders.post`;", "id `orders.find-order`;", "id `sweep`;"] {
        assert!(turns[0].contains(id), "{id} is in the brief: {}", turns[0]);
    }
    assert!(!turns[0].contains("api-orders"), "the survey's stem gave way: {}", turns[0]);
}

// A loader registered over a directory of the tree — `register(autoload, {
// dir: join(__dirname, "routes") })` — mounts each module beneath it at the
// path of its directory from there, under the loader's `prefix` option where
// it names one, so a route's resource is read from its directory and the
// survey's stem for the literal alone gives way: `/login` registered in
// `routes/api/auth/index.ts` is `auth`, as `/daily` in `admin/reports/` under
// `/internal` is `reports`, while a route on `/` at the root or under a
// namespace alone keeps the survey's.
const AUTOLOAD: [(&str, &str); 7] = [
    (
        "src/server.ts",
        "import Fastify from \"fastify\";\nimport { app } from \"./app\";\n\nconst server = \
         Fastify();\nserver.register(app);\nserver.listen({ port: 3000 });\n",
    ),
    (
        "src/app.ts",
        "import path from \"node:path\";\nimport autoload from \"@fastify/autoload\";\nimport type \
         { FastifyInstance } from \"fastify\";\n\nexport async function app(fastify: \
         FastifyInstance) {\n  fastify.register(autoload, {\n    dir: \
         path.join(import.meta.dirname, \"routes\"),\n    autoHooks: true\n  });\n  \
         fastify.register(autoload, {\n    dir: __dirname + \"/admin\",\n    options: { prefix: \
         \"/internal\" }\n  });\n}\n",
    ),
    (
        "src/routes/home.ts",
        "import type { FastifyInstance } from \"fastify\";\n\nexport default async function \
         (fastify: FastifyInstance) {\n  fastify.get(\"/\", async () => ({ ok: true }));\n}\n",
    ),
    (
        "src/routes/api/index.ts",
        "import type { FastifyInstance } from \"fastify\";\n\nexport default async function \
         (fastify: FastifyInstance) {\n  fastify.get(\"/\", async () => ({ version: 1 }));\n}\n",
    ),
    (
        "src/routes/api/auth/index.ts",
        "import type { FastifyPluginAsync } from \"fastify\";\n\nconst plugin: FastifyPluginAsync \
         = async (fastify) => {\n  fastify.post(\"/login\", async () => {\n    return { token: \
         \"t\" };\n  });\n};\n\nexport default plugin;\n",
    ),
    (
        "src/routes/api/users/index.ts",
        "import type { FastifyInstance } from \"fastify\";\n\nexport default async function \
         (fastify: FastifyInstance) {\n  fastify.get(\"/:id\", async () => ({ id: 1 }));\n  \
         fastify.put(\"/update-password\", async () => ({ ok: true }));\n}\n",
    ),
    (
        "src/admin/reports/index.ts",
        "import type { FastifyInstance } from \"fastify\";\n\nexport default async function \
         (fastify: FastifyInstance) {\n  fastify.get(\"/daily\", async () => ({ rows: [] \
         }));\n}\n",
    ),
];

#[tokio::test]
async fn typescript_autoload_routes() {
    let project = scratch();
    modules(&project, &AUTOLOAD);
    let survey = inventory(
        &[
            ("GET /", "src/routes/home.ts#L4", "home"),
            ("GET /api", "src/routes/api/index.ts#L4", "api"),
            ("POST /api/auth/login", "src/routes/api/auth/index.ts#L4-L6", "login"),
            ("GET /api/users/:id", "src/routes/api/users/index.ts#L4", "users"),
            (
                "PUT /api/users/update-password",
                "src/routes/api/users/index.ts#L5",
                "update-password",
            ),
            ("GET /internal/reports/daily", "src/admin/reports/index.ts#L4", "daily"),
        ],
        &[],
    );
    let login = claim("auth.login");
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &login, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/server.ts", "start"),
            ("GET /", "src/routes/home.ts", "home"),
            ("GET /api", "src/routes/api/index.ts", "api"),
            ("POST /api/auth/login", "src/routes/api/auth/index.ts", "auth"),
            ("GET /api/users/:id", "src/routes/api/users/index.ts", "users"),
            ("PUT /api/users/update-password", "src/routes/api/users/index.ts", "users"),
            ("GET /internal/reports/daily", "src/admin/reports/index.ts", "reports"),
        ]
    );
    for id in [
        "id `home`;",
        "id `api`;",
        "id `auth`;",
        "id `users.get-id`;",
        "id `users.put-update-password`;",
        "id `reports`;",
    ] {
        assert!(turns[0].contains(id), "{id} is in the brief: {}", turns[0]);
    }
    for stem in ["`login`", "`update-password`", "`daily`"] {
        assert!(
            !turns[0].contains(&format!("stem {stem}")),
            "the survey's stem gave way: {}",
            turns[0]
        );
    }
}

// The survey names a surface under `start` — the bootstrap's stem, which
// the caller names itself — and the finding sends the answer back for the
// stem of what a caller does through the surface; the corrected answer is
// accepted, and the run goes on to mine.
#[tokio::test]
async fn typescript_survey_start_stem() {
    let project = scratch();
    modules(&project, &APP);
    let strayed = inventory(
        &[
            ("POST /api/orders", "src/routes.ts#L6-L8", "start"),
            ("GET /api/orders/:id", "src/routes.ts#L9", "orders"),
        ],
        &[],
    );
    let corrected = app_inventory();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&strayed, &corrected, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "two survey rounds, one seam, one for the inline value");
    for request in &seen[..2] {
        system(request, prompt::survey(typescript::PROSE));
    }
    system(&seen[2], prompt::extract(typescript::PROSE));
    let exchanges = model.exchanges();
    let finding = exchanges[0].outcome.as_ref().expect_err("the `start` stem is refused");
    assert!(
        finding.contains(
            "- surface `POST /api/orders`: `start` is the bootstrap's stem, which the caller names \
             itself; give the surface the stem of what a caller does through it"
        ),
        "the finding names the surface and the stem: {finding}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected inventory is accepted");
    assert_eq!(surfaces(&seen[2].messages[0]).len(), 3, "{}", seen[2].messages[0]);
}

// A module the facts list a registration in — a worker nothing imports — that
// no named surface reaches and the answer does not list under `unreached` is
// a finding, naming the module; the answer that names the worker's surface
// is accepted, and the module is that surface's to reach.
#[tokio::test]
async fn typescript_survey_unplaced() {
    let project = scratch();
    modules(&project, &JOBS);
    project.write(
        "src/cli.ts",
        "import { Command } from \"commander\";\nimport { runImport } from \"./import\";\n\nconst \
         program = new Command();\nprogram.command(\"import <file>\").action(runImport);\n\
         program.parse();\n",
    );
    let partial = inventory(&[("import command", "src/cli.ts#L5", "import")], &[]);
    let complete = inventory(
        &[
            ("import command", "src/cli.ts#L5", "import"),
            ("invoices worker", "src/worker.ts#L5-L7", "invoices"),
        ],
        &[],
    );
    let answer = claim("invoices.remind");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&partial, &complete, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "two survey rounds, one seam, one for the inline value");
    let exchanges = model.exchanges();
    let finding = exchanges[0].outcome.as_ref().expect_err("the unplaced worker is refused");
    assert!(
        finding.contains(
            "- one module the facts list a registration or declaration in is reached by no \
             surface you named and not listed under `unreached`: `src/worker.ts`;"
        ),
        "the finding names the module: {finding}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the completed inventory is accepted");
    let turn = &seen[2].messages[0];
    assert_eq!(
        surfaces(turn),
        [
            ("start", "src/cli.ts", "start"),
            ("import command", "src/cli.ts", "import"),
            ("invoices worker", "src/worker.ts", "invoices"),
        ]
    );
    assert!(turn.contains("id `invoices`; reaches `src/bus.ts`."), "{turn}");
}

// A method under a package's decorator is a surface at the lines the survey
// names: a verb decorator maps a route under the class decorator's prefix,
// which gives the stem; a shaping decorator (`@HttpCode`) marks none, so the
// method under two is one surface; the method's name tells it apart, and its
// modules are read through the class it sits in.
#[tokio::test]
async fn typescript_decorated() {
    let project = scratch();
    modules(&project, &NEST);
    let survey = inventory(
        &[
            ("GET /orders/:id", "src/orders.controller.ts#L8-L12", "orders"),
            ("POST /orders", "src/orders.controller.ts#L14-L17", "orders"),
        ],
        &[],
    );

    let model = mined(TYPESCRIPT, &project, &survey, 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
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

// The survey anchors a whole controller as one surface: the class decorator's
// prefix gives the stem, the class's decorated methods each give an id
// beside the surface's own, and the decorators section of the survey turn
// is where the model read them.
#[tokio::test]
async fn typescript_survey_methods() {
    let project = scratch();
    modules(&project, &NEST);
    let survey =
        inventory(&[("orders controller", "src/orders.controller.ts#L4-L18", "orders")], &[]);

    let model = mined(TYPESCRIPT, &project, &survey, 1).await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    for fact in [
        "- `src/orders.controller.ts#L5-L18` — `@Controller(\"orders\")` on class \
         `OrdersController`, through `@nestjs/common`",
        "- `src/orders.controller.ts#L8-L12` — `@Get(\":id\")` on `OrdersController.find`, through \
         `@nestjs/common`",
        "- `src/orders.controller.ts#L14-L17` — `@Post` on `OrdersController.create`, through \
         `@nestjs/common`",
    ] {
        assert!(facts.contains(fact), "{fact} is among the facts: {facts}");
    }
    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/main.ts", "start"),
            ("orders controller", "src/orders.controller.ts", "orders")
        ]
    );
    assert!(
        turns[0].contains(
            "under `@Controller(\"orders\")`; through `@nestjs/common`; methods `find` L8–L12, \
             `create` L14–L17; ids `orders`, `orders.find`, `orders.create`; reaches \
             `src/orders.service.ts`."
        ),
        "the class carries its decorated methods as ids: {}",
        turns[0]
    );
}

// Two surfaces the code tells apart by nothing at their anchors — one
// `@Post("upload")` method `upload` under `@Controller("files")` in each of
// two drivers' modules — are told apart by the nearest segment of their
// entries' paths that spells neither the stem nor the id's own tail.
#[tokio::test]
async fn typescript_survey_alike() {
    const CONTROLLER: &str = "import { Controller, Post } from \"@nestjs/common\";\n\n\
                              @Controller(\"files\")\nexport class FilesController {\n  \
                              @Post(\"upload\")\n  upload() {\n    return { stored: true };\n  \
                              }\n}\n";
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"nest\",\"main\":\"src/main.ts\"}\n"),
            (
                "src/main.ts",
                "import { NestFactory } from \"@nestjs/core\";\nimport { FilesController as Local } \
                 from \"./local/files.controller\";\nimport { FilesController as S3 } from \
                 \"./s3/files.controller\";\n\nasync function bootstrap() {\n  const app = await \
                 NestFactory.create([Local, S3]);\n  await app.listen(3000);\n}\nbootstrap();\n",
            ),
            ("src/local/files.controller.ts", CONTROLLER),
            ("src/s3/files.controller.ts", CONTROLLER),
        ],
    );
    let survey = inventory(
        &[
            ("POST /files/upload (local)", "src/local/files.controller.ts#L5-L8", "files"),
            ("POST /files/upload (s3)", "src/s3/files.controller.ts#L5-L8", "files"),
        ],
        &[],
    );
    let answer = claim("files.upload.s3");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    for id in ["id `files.upload.local`;", "id `files.upload.s3`;"] {
        assert!(turns[0].contains(id), "{id} is in the brief: {}", turns[0]);
    }
}

// The Nest service `typescript_decorated` and `typescript_survey_methods`
// read: a controller under a package's decorators, and the service it
// constructs.
const NEST: [(&str, &str); 4] = [
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
];

// A class decorator that takes its path in an object — `@Controller({ path,
// version })` — prefixes its methods' routes as a literal one does, so the
// routes stem by the resource the code reads and not by the survey's
// versioned reading; a documentation decorator before it (`@ApiTags("Users")`)
// carries no prefix, and one that shapes a method's answer
// (`@SerializeOptions`) does not stand in for the verb beneath it.
#[tokio::test]
async fn typescript_decorated_object_path() {
    let project = scratch();
    modules(
        &project,
        &[
            ("package.json", "{\"name\":\"nest\",\"main\":\"src/main.ts\"}\n"),
            (
                "src/main.ts",
                "import { NestFactory } from \"@nestjs/core\";\nimport { UsersController } from \
                 \"./users.controller\";\n\nasync function bootstrap() {\n  const app = await \
                 NestFactory.create(UsersController);\n  await app.listen(3000);\n}\nbootstrap();\n",
            ),
            (
                "src/users.controller.ts",
                "import { Controller, Delete, Get, Param, SerializeOptions } from \
                 \"@nestjs/common\";\nimport { ApiTags } from \"@nestjs/swagger\";\n\n@ApiTags(\"Users\")\n\
                 @Controller({\n  path: \"users\",\n  version: \"1\",\n})\nexport class UsersController \
                 {\n  @SerializeOptions({ groups: [\"admin\"] })\n  @Get(\":id\")\n  find(@Param(\"id\") \
                 id: string) {\n    return { id };\n  }\n\n  @Delete(\":id\")\n  remove(@Param(\"id\") \
                 id: string) {\n    return { id, removed: true };\n  }\n}\n",
            ),
        ],
    );

    let survey = inventory(
        &[
            ("GET /v1/users/:id", "src/users.controller.ts#L10-L14", "users-v1"),
            ("DELETE /v1/users/:id", "src/users.controller.ts#L16-L19", "users-v1"),
        ],
        &[],
    );
    let answer = claim("users.find");
    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/main.ts", "start"),
            ("GET /v1/users/:id", "src/users.controller.ts", "users"),
            ("DELETE /v1/users/:id", "src/users.controller.ts", "users"),
        ]
    );
    for id in ["id `users.find`", "id `users.remove`"] {
        assert!(turns[0].contains(id), "the method tells the surface: {id}: {}", turns[0]);
    }
}

// A library's entry only declares and exports, so the tree has no bootstrap;
// the survey turn lists what the entry exports, following a barrel's
// re-exports one hop to the modules that declare them, and the surfaces the
// survey names there are read at their declarations: a class carries an id
// per public method beside its own, a private method none; a function
// reaches what it imports.
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
    let survey = inventory(
        &[("Money", "src/money.ts#L3-L11", "money"), ("parse", "src/parse.ts#L5-L7", "parse")],
        &["src/index.ts"],
    );
    let answer = claim("parse.text");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    for fact in [
        "No bootstrap runs at load",
        "- `src/index.ts` re-exports from `src/money.ts`, where each is declared: `Money` (class) \
         L3–L11",
        "- `src/index.ts` re-exports from `src/parse.ts`, where each is declared: `parse` \
         (function) L5–L7, `ParseError` (class) L3",
    ] {
        assert!(facts.contains(fact), "{fact} is among the facts: {facts}");
    }
    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("Money", "src/money.ts", "money"), ("parse", "src/parse.ts", "parse")]
    );
    assert!(
        turns[0].contains(
            ": exported class L3–L11; methods `add` L6–L8; ids `money`, `money.add`; reaches \
             nothing beyond its entry."
        ),
        "public methods only, each an id: {}",
        turns[0]
    );
    assert!(
        turns[0].contains(": exported function L5–L7; id `parse`; reaches `src/money.ts`."),
        "{}",
        turns[0]
    );
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
    let survey = inventory(
        &[("Money", "src/money.ts#L15-L21", "money"), ("parse", "src/money.ts#L27-L30", "parse")],
        &["src/index.ts"],
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
        &[
            "types",
            "export interface Inline { id: string }\ninterface Local { n: number }\n",
            "Inline",
            "Currency",
            "Rounding",
            "Money",
        ],
        ScriptedModel::answering([&survey, &answered, &answered]),
    )
    .await;

    assert_eq!(model.seen().len(), 3, "the survey, one seam over the tree, one over the value");
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
                 await sleep(FIVE_SEC_DELAY);\n  return attempts;\n}\n\nexport const \
                 validation = {\n  transform: true,\n  status: Status.UNPROCESSABLE,\n  \
                 factory: (errors: string[]) => errors.join(\",\"),\n};\n\nexport function \
                 accept(kind: string) {\n  const allowedKinds = [\"image/jpeg\", \
                 \"image/png\"];\n  const fallback = allowedKinds[0];\n  return \
                 allowedKinds.includes(kind) ? kind : fallback;\n}\n",
            ),
            (
                "src/index.ts",
                "import express from \"express\";\nimport { ordersRouter } from \"./routes\";\n\
                 import { PORT } from \"./config\";\n\nconst app = express();\nconst port = \
                 PORT;\napp.use(\"/api\", ordersRouter());\napp.listen(port);\n",
            ),
        ],
    );

    let model = mined(TYPESCRIPT, &project, &app_inventory(), 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    for line in [
        "- `src/config.ts#L3` — `PORT = Number(process.env.PORT ?? 3000)`",
        "- `src/config.ts#L4` — `MAX_LINES = 50`",
        "- `src/config.ts#L5` — `SKU = /^[A-Z]{2,4}-\\d{3,6}$/`",
        "- `src/config.ts#L6` — `API_TOKENS = (process.env.API_TOKENS ?? \"\").split(\",\").filter((token) \
         => token.length > 0)`",
        "- `src/config.ts#L7-L10` — `ORDER = define({ id: \"string\", lines: \"number\", })`",
        "- `src/config.ts#L13` — `Limits.pageSize = Number(process.env.PAGE_SIZE ?? 20)`",
        "- `src/config.ts#L17` — `process.env.PORT` in `app.listen(Number(process.env.PORT ?? 3000));`",
        "- `src/config.ts#L21` — `FIVE_SEC_DELAY = 5 * 1000`",
        "- `src/config.ts#L27-L31` — `validation = { transform: true, status: Status.UNPROCESSABLE, \
         factory: (errors: string[]) => errors.join(\",\"), }`",
        "- `src/config.ts#L34` — `allowedKinds = [\"image/jpeg\", \"image/png\"]`",
        "- `express` — `express` (default), `Router` — in `src/index.ts`, `src/routes.ts`",
    ] {
        assert!(turns[0].contains(line), "{line} is in the brief: {}", turns[0]);
    }
    for absent in
        ["`app = express()`", "`port = PORT`", "`attempts = 0`", "`fallback = allowedKinds[0]`"]
    {
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

    let model = mined(TYPESCRIPT, &project, &app_inventory(), 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
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

// A class of the tree's that extends a package's class: a call through a
// member it inherits — an HTTP client on a shared config base — is a call
// through that package, listed under the base's name and an anchor a
// requirement may take; a call to a member the class declares itself is the
// tree's own. The finding at a line that only builds a URL names the seam's
// anchors in the file: the function heads, the inherited calls, the return.
#[tokio::test]
async fn typescript_inherited() {
    let project = scratch();
    modules(&project, &APP);
    modules(
        &project,
        &[
            (
                "src/config.ts",
                "import { ConfigCommon } from \"acme-common\";\n\nexport class Config extends \
                 ConfigCommon {\n  public static ordersUrl = process.env.ORDERS_URL || \
                 \"http://orders\";\n\n  public static prefixed(value: string): string {\n    return \
                 `orders-${value}`;\n  }\n}\n",
            ),
            (
                "src/orders.ts",
                "import { Config } from \"./config\";\n\nexport async function createOrder(input: \
                 unknown) {\n  const url = `${Config.ordersUrl}/orders`;\n  const response = await \
                 Config.axios.post(url, input);\n  Config.logger.info(Config.prefixed(\"created\"));\n  \
                 return response.data;\n}\n\nexport function findOrder(req: { params: { id: string } }, \
                 res: { json(body: unknown): void }) {\n  res.json({ id: req.params.id });\n}\n",
            ),
        ],
    );
    let posted = serde_json::json!({
        "kind": "requirement", "id": "orders.create",
        "statement": "An order is posted to the orders service.", "path": "src/orders.ts#L5"
    });
    let strayed = serde_json::json!({ "claims": [{
        "kind": "requirement", "id": "orders.create",
        "statement": "An order is posted to the orders service.", "path": "src/orders.ts#L4"
    }] })
    .to_string();
    let corrected = serde_json::json!({ "claims": [&posted] }).to_string();
    let survey = app_inventory();
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let turn = &seen[1].messages[0];
    assert!(
        turn.contains("- `acme-common:ConfigCommon.axios.post` in `src/orders.ts` at L5")
            && turn.contains("- `acme-common:ConfigCommon.logger.info` in `src/orders.ts` at L6"),
        "a member the class inherits is the package's: {turn}"
    );
    assert!(
        !turn.contains("ConfigCommon.prefixed") && !turn.contains("ConfigCommon.ordersUrl"),
        "a member the class declares is the tree's own: {turn}"
    );
    let exchanges = checked(&model);
    let correction =
        exchanges[0].outcome.as_ref().expect_err("a requirement at the URL's binding is refused");
    assert!(
        correction.contains("claim 0: path `src/orders.ts#L4`")
            && correction.contains("in `src/orders.ts` it names L3, L5, L6, L7, L10;"),
        "the finding names the heads, the inherited calls, and the return as anchors: \
         {correction}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the requirement at the post is accepted");
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
    let survey = inventory(&[("GET /api/orders/:id", "src/routes/orders.ts#L6-L8", "orders")], &[]);
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 2);
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

    let model = mined(TYPESCRIPT, &project, &app_inventory(), 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
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

// The survey answer over `DECIDING`: the one route, at its registration.
fn deciding_inventory() -> String {
    inventory(&[("POST /orders", "src/index.ts#L5", "orders")], &[])
}

// The points where the code decides — a guard, a switch, a throw, a catch,
// a timer, a conditional — are listed at their lines with their text and
// the function they run in, so a requirement anchors where a behaviour
// starts.
#[tokio::test]
async fn typescript_decisions() {
    let project = scratch();
    modules(&project, &DECIDING);

    let model = mined(TYPESCRIPT, &project, &deciding_inventory(), 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
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
// starts or its result is decided — a decision, a `return`, a function's
// head, a call through a package, a boundary, a surface's lines: one
// anchored at a line that only imports is the SDK's finding, a `criterion`
// there is not, the corrected answer is accepted, and the inline value is
// held to no anchor.
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
    let headed = serde_json::json!({
        "kind": "requirement", "id": "orders.create-order",
        "statement": "Creating an order validates the body, then responds 201 with the id.",
        "path": "src/orders.ts#L5"
    });
    let strayed = serde_json::json!({
        "claims": [&guarded, &wired, &listening, &retry, &returned, &headed]
    })
    .to_string();
    let corrected =
        serde_json::json!({ "claims": [&guarded, &listening, &retry, &returned, &headed] })
            .to_string();
    let survey = deciding_inventory();
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("the wired anchor is refused");
    assert!(
        correction.contains("claim 1: path `src/orders.ts#L1`"),
        "the finding names the claim at the import: {correction}"
    );
    for held in ["claim 0:", "claim 2:", "claim 3:", "claim 4:", "claim 5:"] {
        assert!(!correction.contains(held), "{held} is at an anchor or not held: {correction}");
    }
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the inline value is held to no anchor");
}

// An Express service whose handler steps into the tree through instances:
// a repository constructed into a local, awaited and then called for its
// effect alone, and a mailer handed to a helper whose parameter is typed
// by the tree's class.
const STEPPING: [(&str, &str); 4] = [
    (
        "src/index.ts",
        "import express from \"express\";\nimport { createOrder } from \"./orders\";\n\nconst app \
         = express();\napp.post(\"/orders\", createOrder);\napp.listen(3000);\n",
    ),
    (
        "src/orders.ts",
        "import { OrdersRepository } from \"./repository\";\nimport { Mailer } from \
         \"./mailer\";\n\nexport async function createOrder(req: { body: { sku: string } }, res: { \
         json(body: unknown): void }) {\n  const repo = new OrdersRepository();\n  const id = \
         await repo.insert(req.body.sku);\n  repo.flush();\n  await notify(new Mailer(), id);\n  \
         res.json({ id });\n}\n\nasync function notify(mailer: Mailer, id: string) {\n  await \
         mailer.send(id);\n}\n",
    ),
    (
        "src/repository.ts",
        "export class OrdersRepository {\n  async insert(sku: string): Promise<string> {\n    \
         return sku;\n  }\n  flush(): void {}\n}\n",
    ),
    ("src/mailer.ts", "export class Mailer {\n  async send(id: string): Promise<void> {}\n}\n"),
];

// A step a function takes into the tree is a `requirement`'s anchor however
// the code reaches the instance it steps on: a call awaited or made for its
// effect alone on a local a tree class constructs, or on a parameter typed
// by one, as on `this` or a module import; the construction the local holds
// only wires, and is refused.
#[tokio::test]
async fn typescript_steps() {
    let project = scratch();
    modules(&project, &STEPPING);
    let inserted = serde_json::json!({
        "kind": "requirement", "id": "orders.insert",
        "statement": "An order is inserted by its sku.", "path": "src/orders.ts#L6"
    });
    let flushed = serde_json::json!({
        "kind": "requirement", "id": "orders.flush",
        "statement": "The repository is flushed after the insert.", "path": "src/orders.ts#L7"
    });
    let sent = serde_json::json!({
        "kind": "requirement", "id": "orders.notify",
        "statement": "The order's id is mailed.", "path": "src/orders.ts#L13"
    });
    let constructed = serde_json::json!({
        "kind": "requirement", "id": "orders.repository",
        "statement": "Orders use a repository.", "path": "src/orders.ts#L5"
    });
    let strayed =
        serde_json::json!({ "claims": [&inserted, &constructed, &flushed, &sent] }).to_string();
    let corrected = serde_json::json!({ "claims": [&inserted, &flushed, &sent] }).to_string();
    let survey = inventory(&[("POST /orders", "src/index.ts#L5", "orders")], &[]);
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("the construction is refused");
    assert!(
        correction.contains("claim 1: path `src/orders.ts#L5`"),
        "the finding names the claim at the construction: {correction}"
    );
    for held in ["claim 0:", "claim 2:", "claim 3:"] {
        assert!(!correction.contains(held), "{held} is at a step into the tree: {correction}");
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
    let survey = app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 2);
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

// The service imports a module the tree does not hold and loads another by
// a computed name, so its seam cannot know what it reaches: the rest of the
// tree follows the closure — the entry, listed after the store past the
// budget — and the brief says what could not be followed and why the list
// runs on. The bootstrap's seam, whose modules import nothing unresolved,
// is not widened. Within the budget every module is laid already, so the
// brief says only what could not be followed.
#[tokio::test]
async fn typescript_unresolved() {
    const ORDERS: &str = "import { pool } from \"./db\";\nimport { seed } from \
                          \"./generated\";\n\nexport async function createOrder(input: unknown) \
                          {\n  return { id: pool, input, seed };\n}\n\nexport function \
                          loadPlugin(name: string) {\n  return import(name);\n}\n\nexport function \
                          findOrder(req: { params: { id: string } }, res: { json(body: unknown): \
                          void }) {\n  res.json({ id: req.params.id, pool });\n}\n";
    const UNFOLLOWED: &str = "The caller could not follow every import: `./generated` from \
                              `src/orders.ts` names no module of the tree; `src/orders.ts` loads a \
                              module by a computed name at L9. What these name is in none of the \
                              lists above.";
    let project = scratch();
    modules(&project, &APP);
    project.write("src/orders.ts", ORDERS);
    bulk(&project, "src/db.ts", "export const pool = \"o-1\";\n");
    let survey = app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 2);
    let orders = turn_for(&turns, "POST /api/orders");
    laid(orders, &["src/routes.ts", "src/orders.ts"], &[]);
    let db = orders.find("- `src/db.ts`\n").expect("the store past the budget is listed");
    let index = orders.find("- `src/index.ts`\n").expect("the entry follows the closure");
    assert!(db < index, "the rest of the tree is listed after the closure: {orders}");
    assert!(
        orders.contains(&format!(
            "{UNFOLLOWED} The modules after the closure are the rest of the tree, laid so what \
             these name is still within reach; read them for that alone."
        )),
        "the brief says what could not be followed and why the list runs on: {orders}"
    );
    let start = turn_for(&turns, "start");
    laid(start, &["src/index.ts", "src/routes.ts"], &["src/orders.ts", "src/db.ts"]);
    assert!(!start.contains("could not follow"), "nothing of `start` is unresolved: {start}");

    // within the budget
    let project = scratch();
    modules(&project, &APP);
    project.write("src/orders.ts", ORDERS);

    let model = mined(TYPESCRIPT, &project, &app_inventory(), 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    laid(&turns[0], &["src/index.ts", "src/routes.ts", "src/orders.ts", "src/db.ts"], &[]);
    assert!(turns[0].contains(UNFOLLOWED), "{}", turns[0]);
    assert!(!turns[0].contains("rest of the tree"), "nothing was widened: {}", turns[0]);
}

// A `.json` a module imports by path is a data file of the seam: named in the
// brief with the module reading it, laid directly after the first module
// importing it when small, and a file a `criterion` may cite a value in —
// while a `requirement` there is at no anchor and comes back.
#[tokio::test]
async fn typescript_data() {
    let project = scratch();
    modules(&project, &APP);
    project.write(
        "src/orders.ts",
        "import { pool } from \"./db\";\nimport zones from \"./zones.json\";\n\nexport async \
         function createOrder(input: unknown) {\n  return { id: pool, input, zone: \
         zones.rural };\n}\n\nexport function findOrder(req: { params: { id: string } }, res: { \
         json(body: unknown): void }) {\n  res.json({ id: req.params.id, pool });\n}\n",
    );
    project.write("src/zones.json", "{\n  \"rural\": 650,\n  \"urban\": 0\n}\n");
    let cited = serde_json::json!({
        "kind": "criterion", "id": "orders.rural-surcharge",
        "criterion": "The rural surcharge is 650 cents.", "path": "src/zones.json#L2"
    });
    let anchored = serde_json::json!({
        "kind": "requirement", "id": "orders.rural",
        "statement": "A rural order carries the surcharge.", "path": "src/zones.json#L2"
    });
    let strayed = serde_json::json!({ "claims": [&cited, &anchored] }).to_string();
    let corrected = serde_json::json!({ "claims": [&cited] }).to_string();
    let survey = app_inventory();
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let turn = &seen[1].messages[0];
    assert!(
        turn.contains(
            "Data files these modules name by path, each with the modules reading it, laid after \
             the first module naming it — a large one after every module:\n\n- `src/zones.json` \
             — named by `src/orders.ts`"
        ),
        "the data file is named with its reader: {turn}"
    );
    laid(
        turn,
        &["src/index.ts", "src/routes.ts", "src/orders.ts", "src/db.ts", "src/zones.json"],
        &[],
    );
    let reader = turn.find("### `src/orders.ts`").expect("the reading module is laid");
    let data_start = turn.find("### `src/zones.json`").expect("the data file is laid");
    let next = turn.find("### `src/db.ts`").expect("the module after the reader is laid");
    assert!(
        reader < data_start && data_start < next,
        "a small data file is laid directly after the module naming it: {turn}"
    );
    assert!(turn.contains("2|  \"rural\": 650,"), "numbered: {turn}");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("a requirement in data is refused");
    assert!(
        correction.contains("claim 1: path `src/zones.json#L2`")
            && !correction.contains("claim 0:"),
        "the criterion is held to no anchor, the requirement to the modules': {correction}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the criterion in data is accepted");
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

    let model = mined(TYPESCRIPT, &project, &app_inventory(), 1).await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    laid(&turns[0], &["vite.config.ts", "src/index.ts"], &[]);
}

// A tree of one production module cuts no finer than itself: one extract
// over the module, listed when it is past the budget, the export the survey
// names its surface.
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
    let survey = inventory(&[("OrderService", "src/orders.ts#L1-L5", "order-service")], &[]);
    let answer = claim("order-service.create");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
    assert_eq!(surfaces(&turns[0]), [("OrderService", "src/orders.ts", "order-service")]);
    assert!(
        turns[0].contains("ids `order-service`, `order-service.create`;"),
        "the class carries its method: {}",
        turns[0]
    );
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

// A tree the survey names no surface in — no bootstrap, no registration, no
// decorator, and no entry export — is cut mechanically, never refused:
// within the budget, one extract over every module, told no surface was
// found and held to the one stem the manifest's package name gives.
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
    let survey = inventory(&[], &["src/lib/config.ts", "src/lib/db.ts"]);
    let answer = claim("shared-helpers.pool");

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 1);
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
    let survey = inventory(&[], &["src/index.ts", "src/lib/config.ts", "src/lib/db.ts"]);
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_TYPESCRIPT,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(TYPESCRIPT, &model, 2);
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

// A FastAPI service the way one is written: the entry includes two routers
// under a prefix, each router declares its routes under a package's
// decorator, and the orders routes reach a service and its store.
const PY_APP: [(&str, &str); 7] = [
    ("app/__init__.py", ""),
    (
        "app/main.py",
        "from fastapi import FastAPI\n\nfrom .routers import health, orders\n\napp = \
         FastAPI()\napp.include_router(orders.router, prefix=\"/api\")\n\
         app.include_router(health.router)\n",
    ),
    ("app/routers/__init__.py", ""),
    (
        "app/routers/orders.py",
        "from fastapi import APIRouter\n\nfrom ..services.orders import create_order, \
         find_order\n\nrouter = APIRouter(prefix=\"/orders\")\n\n\n@router.post(\"\")\nasync def \
         create(body: dict) -> dict:\n    return await create_order(body)\n\n\n\
         @router.get(\"/{order_id}\")\nasync def get(order_id: str) -> dict:\n    return \
         find_order(order_id)\n",
    ),
    (
        "app/routers/health.py",
        "from fastapi import APIRouter\n\nrouter = APIRouter()\n\n\n@router.get(\"/health\")\ndef \
         health() -> dict:\n    return {\"ok\": True}\n",
    ),
    (
        "app/services/orders.py",
        "from ..db import pool\n\n\nasync def create_order(body: dict) -> dict:\n    return \
         {\"id\": pool, \"body\": body}\n\n\ndef find_order(order_id: str) -> dict:\n    return \
         {\"id\": order_id, \"pool\": pool}\n",
    ),
    ("app/db.py", "pool = \"o-1\"\n"),
];

// The survey answer over `PY_APP`: the three routes, each at its decorated
// function.
fn py_app_inventory() -> String {
    inventory(
        &[
            ("POST /api/orders", "app/routers/orders.py#L8-L10", "orders"),
            ("GET /api/orders/{order_id}", "app/routers/orders.py#L13-L15", "orders"),
            ("GET /health", "app/routers/health.py#L6-L8", "health"),
        ],
        &[],
    )
}

// A Python workspace opens one survey turn before it is mined, and the
// inline value none. A tree whose modules fit within the SDK's inline budget
// is one extract over every module, laid into the turn, told every surface
// the survey named and held to every stem; the conventional entry — a
// package's `main.py` constructing the application at load — is the
// bootstrap when no script names one, and the caller's `start` leads the
// surfaces. Each surface line carries what the code read at its anchor —
// the decorated `def`, its decorator, the package the decorator's head was
// constructed from — the id its requirements lead with (the stem alone for
// a stem's one surface; under a shared stem, the verb and the path past the
// resource) and the modules it reaches beyond its entry. The inline value
// spends one turn more.
#[tokio::test]
async fn python() {
    let project = scratch();
    modules(&project, &PY_APP);

    let model = mined(PYTHON, &project, &py_app_inventory(), 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "app/main.py", "start"),
            ("POST /api/orders", "app/routers/orders.py", "orders"),
            ("GET /api/orders/{order_id}", "app/routers/orders.py", "orders"),
            ("GET /health", "app/routers/health.py", "health"),
        ]
    );
    for note in [
        "- Surface `start` — entry `app/main.py` — stem `start`: the process bootstrap, run at \
         load: what runs before each handler is registered, what it awaits before serving, and \
         at shutdown — `stop` and what a signal handler calls, wherever declared; id `start`; \
         reaches `app/routers/orders.py`, `app/routers/health.py`.",
        ": def `create` L8–L10; under `@router.post(\"\")`; through `fastapi`; id `orders.post`; \
         reaches `app/services/orders.py`, `app/db.py`.",
        ": def `get` L13–L15; under `@router.get(\"/{order_id}\")`; through `fastapi`; id \
         `orders.get-order-id`; reaches `app/services/orders.py`, `app/db.py`.",
        ": def `health` L6–L8; under `@router.get(\"/health\")`; through `fastapi`; id `health`; \
         reaches nothing beyond its entry.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    laid(
        &turns[0],
        &[
            "app/main.py",
            "app/routers/orders.py",
            "app/routers/health.py",
            "app/services/orders.py",
            "app/db.py",
        ],
        &[],
    );
    assert!(turns[0].contains("1|from fastapi import FastAPI"), "numbered: {}", turns[0]);
}

// The seam's stems hold the call's ids: an id under another stem is the SDK's
// finding, naming the stems the adapter chose, and the corrected answer is the
// seam's; the inline value is held to none.
#[tokio::test]
async fn python_stem() {
    let project = scratch();
    modules(&project, &PY_APP);
    let survey = py_app_inventory();
    let strayed = claim("reconciliation.nightly");
    let corrected = claim("orders.create");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &strayed]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("the stray stem is refused");
    assert!(
        correction.contains("`reconciliation.nightly`")
            && correction.contains("`orders`")
            && correction.contains("`health`")
            && correction.contains("`start`"),
        "the finding names the id and the seam's stems: {correction}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the inline value is held to no stem");
}

// A tree past the budget is one extract per stem over the modules its
// surfaces reach: the orders routes' seam reaches the service and the store
// from the router, never the entry; the bootstrap's reaches the routers it
// includes and stops there; the health route's reaches nothing — and each
// surface line says so. A module past the budget is listed, the rest laid.
#[tokio::test]
async fn python_closure() {
    let project = scratch();
    modules(&project, &PY_APP);
    py_bulk(&project, "app/db.py", "pool = \"o-1\"\n");
    let survey = py_app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 3);
    let start = turn_for(&turns, "start");
    assert_eq!(surfaces(start), [("start", "app/main.py", "start")]);
    assert!(
        start.contains("; id `start`; reaches `app/routers/orders.py`, `app/routers/health.py`."),
        "{start}"
    );
    laid(
        start,
        &["app/main.py", "app/routers/orders.py", "app/routers/health.py"],
        &["app/services/orders.py", "app/db.py"],
    );
    let orders = turn_for(&turns, "POST /api/orders");
    assert!(orders.contains("This call mines the 2 surfaces under the stem `orders` alone:"));
    assert_eq!(
        surfaces(orders),
        [
            ("POST /api/orders", "app/routers/orders.py", "orders"),
            ("GET /api/orders/{order_id}", "app/routers/orders.py", "orders"),
        ]
    );
    assert!(
        orders.contains("; id `orders.post`; reaches `app/services/orders.py`, `app/db.py`."),
        "{orders}"
    );
    laid(orders, &["app/routers/orders.py", "app/services/orders.py"], &["app/main.py"]);
    assert!(orders.contains("- `app/db.py`"), "the store past the budget is listed: {orders}");
    let health = turn_for(&turns, "GET /health");
    assert!(health.contains("This call mines the surface under the stem `health` alone:"));
    laid(health, &["app/routers/health.py"], &["app/main.py", "app/routers/orders.py"]);
}

// The bootstrap is the first entry that runs something: the module a
// console script names, run by the script's call of the function it names,
// over the conventional entry constructing the application at load; with
// no manifest, the conventional entry that runs under its `__main__` guard.
#[tokio::test]
async fn python_bootstrap() {
    const SHOP: [(&str, &str); 3] = [
        ("shop/__init__.py", ""),
        (
            "shop/app.py",
            "from fastapi import FastAPI\n\nfrom .routes import router\n\napp = FastAPI()\n\
             app.include_router(router)\n",
        ),
        (
            "shop/routes.py",
            "from fastapi import APIRouter\n\nrouter = APIRouter()\n\n\n\
             @router.get(\"/orders/{order_id}\")\ndef get_order(order_id: str) -> dict:\n    \
             return {\"id\": order_id}\n",
        ),
    ];
    let survey = inventory(&[("GET /orders/{order_id}", "shop/routes.py#L6-L8", "orders")], &[]);

    // the console script
    let project = scratch();
    modules(&project, &SHOP);
    modules(
        &project,
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"shop\"\n\n[project.scripts]\nshop = \"shop.cli:main\"\n",
            ),
            ("shop/cli.py", "from .app import app\n\n\ndef main() -> None:\n    app.run()\n"),
        ],
    );

    let model = mined(PYTHON, &project, &survey, 1).await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    for fact in [
        "The manifest names the package `shop`; installs the console scripts `shop` runs \
         `shop.cli:main`.",
        "The bootstrap — the entry that runs something — is `shop/cli.py`: the console script \
         `shop` calls its `main()`.",
    ] {
        assert!(facts.contains(fact), "{fact} is among the facts: {facts}");
    }
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("start", "shop/cli.py", "start"), ("GET /orders/{order_id}", "shop/routes.py", "orders")]
    );
    assert!(
        turns[0].contains(
            "the process bootstrap, run by the console script `shop`, which calls `main()`: "
        ) && turns[0].contains("; id `start`; reaches `shop/app.py`, `shop/routes.py`."),
        "{}",
        turns[0]
    );

    // the guard
    let project = scratch();
    modules(&project, &SHOP);
    project.write(
        "manage.py",
        "import sys\n\nfrom shop.app import app\n\nif __name__ == \"__main__\":\n    \
         app.run(sys.argv)\n",
    );

    let model = mined(PYTHON, &project, &survey, 1).await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    for fact in [
        "The tree has no `pyproject.toml` or `setup.cfg` the parser could read.",
        "The bootstrap — the entry that runs something — is `manage.py`: it runs under its \
         `__main__` guard.",
    ] {
        assert!(facts.contains(fact), "{fact} is among the facts: {facts}");
    }
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("start", "manage.py", "start"), ("GET /orders/{order_id}", "shop/routes.py", "orders")]
    );
    assert!(
        turns[0].contains("the process bootstrap, run under its `__main__` guard: "),
        "{}",
        turns[0]
    );
}

// A console script whose function the entry module imports from a module of
// the tree rather than declares — `ledger = "ledger.cli:cli"` over a `cli.py`
// of `from .app import cli` — is still the bootstrap: the manifest names a
// running entry, and a module of the tree declares what it runs. The
// `start` reaches the module the function is imported from.
#[tokio::test]
async fn python_bootstrap_imported() {
    let project = scratch();
    modules(
        &project,
        &[
            (
                "pyproject.toml",
                "[project]\nname = \"ledger\"\n\n[project.scripts]\nledger = \"ledger.cli:cli\"\n",
            ),
            ("ledger/__init__.py", ""),
            ("ledger/cli.py", "from .app import cli\n"),
            (
                "ledger/app.py",
                "import click\n\nfrom .importer import run_import\n\n\n@click.group()\ndef cli() -> \
                 None:\n    pass\n\n\n@cli.command(\"import\")\n@click.argument(\"file\")\ndef \
                 import_command(file: str) -> None:\n    run_import(file)\n",
            ),
            ("ledger/importer.py", "def run_import(file: str) -> None:\n    print(file)\n"),
        ],
    );
    let survey = inventory(&[("import command", "ledger/app.py#L11-L14", "ledger-import")], &[]);
    let answer = claim("import.file");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    assert!(
        facts.contains(
            "The bootstrap — the entry that runs something — is `ledger/cli.py`: the console \
             script `ledger` calls its `cli()`."
        ),
        "{facts}"
    );
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("start", "ledger/cli.py", "start"), ("import command", "ledger/app.py", "import")]
    );
    assert!(
        turns[0].contains(
            "the process bootstrap, run by the console script `ledger`, which calls `cli()`: "
        ) && turns[0].contains("; id `start`; reaches `ledger/app.py`."),
        "{}",
        turns[0]
    );
}

// An entry that only declares — a factory a server imports — is no
// bootstrap, so the tree has no `start` and its surfaces are what the
// factory registers.
#[tokio::test]
async fn python_bootstrap_factory() {
    let project = scratch();
    modules(
        &project,
        &[
            ("app/__init__.py", ""),
            (
                "app/main.py",
                "from flask import Flask\n\nfrom .views import health\n\n\ndef create_app() -> \
                 Flask:\n    app = Flask(__name__)\n    app.add_url_rule(\"/health\", \
                 view_func=health)\n    return app\n",
            ),
            ("app/views.py", "def health() -> dict:\n    return {\"ok\": True}\n"),
        ],
    );
    let survey = inventory(&[("GET /health", "app/main.py#L8", "health")], &[]);
    let answer = claim("health.ok");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    assert!(facts.contains("No bootstrap runs at load"), "{facts}");
    assert!(
        facts.contains(
            "- `app/main.py#L8` — `app.add_url_rule` led by `\"/health\"` handed a function, \
             through `flask` as `Flask`"
        ),
        "{facts}"
    );
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(surfaces(&turns[0]), [("GET /health", "app/main.py", "health")]);
    assert!(
        turns[0].contains(
            ": registered L8 in `create_app`; through `flask`; id `health`; reaches \
             `app/views.py`."
        ),
        "{}",
        turns[0]
    );
}

// A Django project the way one is written: `manage.py` names its settings
// by a string, the settings name the URL configuration by one, which
// includes an application's URL module by one more; the application's
// routes hand a view function and a view class by name, a signal receiver
// hooks a model, an admin module registers the model with the admin site
// and declares an action and a display on it, and a management command is
// a module under `management/commands/`.
const DJANGO: [(&str, &str); 14] = [
    (
        "manage.py",
        "import os\nimport sys\n\n\ndef main() -> None:\n    \
         os.environ.setdefault(\"DJANGO_SETTINGS_MODULE\", \"shop.settings\")\n    from \
         django.core.management import execute_from_command_line\n\n    \
         execute_from_command_line(sys.argv)\n\n\nif __name__ == \"__main__\":\n    main()\n",
    ),
    ("shop/__init__.py", ""),
    (
        "shop/settings.py",
        "import os\n\nSECRET_KEY = os.environ[\"SECRET_KEY\"]\nINSTALLED_APPS = \
         [\"django.contrib.admin\", \"orders\"]\nROOT_URLCONF = \"shop.urls\"\n\
         ORDER_CANCEL_WINDOW_HOURS = 24\n",
    ),
    (
        "shop/urls.py",
        "from django.urls import include, path\n\nurlpatterns = [\n    path(\"orders/\", \
         include(\"orders.urls\")),\n]\n",
    ),
    ("orders/__init__.py", ""),
    (
        "orders/urls.py",
        "from django.urls import path\n\nfrom . import views\n\nurlpatterns = [\n    path(\"\", \
         views.list_orders),\n    path(\"<int:pk>/cancel/\", views.CancelView.as_view()),\n]\n",
    ),
    (
        "orders/views.py",
        "from django.http import JsonResponse\nfrom django.views import View\n\nfrom .services \
         import cancel, recent\n\n\ndef list_orders(request):\n    return JsonResponse({\"orders\": \
         recent()})\n\n\nclass CancelView(View):\n    def post(self, request, pk: int):\n        \
         cancel(pk)\n        return JsonResponse({\"id\": pk})\n",
    ),
    (
        "orders/services.py",
        "from django.conf import settings\n\n\ndef recent() -> list:\n    return []\n\n\ndef \
         cancel(pk: int) -> None:\n    if settings.ORDER_CANCEL_WINDOW_HOURS < 1:\n        raise \
         ValueError(\"closed\")\n",
    ),
    (
        "orders/signals.py",
        "from django.db.models.signals import post_save\nfrom django.dispatch import \
         receiver\n\nfrom .models import Order\n\n\n@receiver(post_save, sender=Order)\ndef \
         on_saved(sender, instance, **kwargs):\n    pass\n",
    ),
    (
        "orders/models.py",
        "from django.db import models\n\n\nclass Order(models.Model):\n    status = \
         models.CharField(max_length=16)\n",
    ),
    (
        "orders/admin.py",
        "from django.contrib import admin\n\nfrom .models import Order\nfrom .services import \
         cancel\n\n\n@admin.register(Order)\nclass OrderAdmin(admin.ModelAdmin):\n    \
         list_display = [\"id\", \"status\"]\n    actions = [\"cancel_orders\"]\n\n    \
         @admin.action(description=\"Cancel selected orders.\")\n    def cancel_orders(self, \
         request, queryset):\n        for order in queryset:\n            cancel(order.pk)\n\n    \
         @admin.display(description=\"State\")\n    def state(self, order):\n        return \
         order.status\n",
    ),
    ("orders/management/__init__.py", ""),
    ("orders/management/commands/__init__.py", ""),
    (
        "orders/management/commands/import_orders.py",
        "from django.core.management.base import BaseCommand\n\nfrom orders.services import \
         recent\n\n\nclass Command(BaseCommand):\n    def handle(self, *args, **options):\n        \
         self.stdout.write(str(len(recent())))\n",
    ),
];

// The survey answer over `DJANGO`: the two routes at their registrations,
// the command at its class, the receiver and the model under no surface.
fn django_inventory() -> String {
    inventory(
        &[
            ("GET /orders/", "orders/urls.py#L6", "orders"),
            ("POST /orders/<int:pk>/cancel/", "orders/urls.py#L7", "orders"),
            (
                "import_orders command",
                "orders/management/commands/import_orders.py#L6-L8",
                "import-orders",
            ),
        ],
        &["orders/signals.py", "orders/models.py"],
    )
}

// A string that spells a module path is an import: the bootstrap reaches
// the settings it names, the URL configuration the settings name, and the
// application's URL module that includes — another surface's entry, where
// it stops. The include mounts that module's routes under its path, so a
// route on `""` there is `orders`, told apart by the view handed by name;
// a view class handed by `as_view()` carries its verb methods as ids; a
// module under `management/commands/` is a command named by its file,
// declared by its `Command` class; a migration and a `tests.py` are no
// modules. The admin site's `register`, `action`, and `display` decorators
// hook what they decorate onto a site a package serves, so none is a fact
// and the admin module is asked after by no finding.
#[tokio::test]
async fn python_string_imports() {
    let project = scratch();
    modules(&project, &DJANGO);
    modules(
        &project,
        &[
            ("orders/migrations/0001_initial.py", "from django.db import migrations\n"),
            ("orders/tests.py", "def test_recent():\n    assert True\n"),
        ],
    );

    let model = mined(PYTHON, &project, &django_inventory(), 1).await;

    let facts = &model.seen()[0].messages[0];
    assert!(!facts.contains("`@admin."), "the admin site's decorators register nothing: {facts}");
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "manage.py", "start"),
            ("GET /orders/", "orders/urls.py", "orders"),
            ("POST /orders/<int:pk>/cancel/", "orders/urls.py", "orders"),
            (
                "import_orders command",
                "orders/management/commands/import_orders.py",
                "import-orders"
            ),
        ]
    );
    for note in [
        "; id `start`; reaches `shop/settings.py`, `shop/urls.py`, `orders/urls.py`.",
        ": registered L6 at module level; through `django`; id `orders.list-orders`; reaches \
         `orders/views.py`, `orders/services.py`.",
        ": registered L7 at module level; through `django`; class `CancelView` handed; methods \
         `post` L12–L14; ids `orders.cancel-view`, `orders.cancel-view.post`; reaches \
         `orders/views.py`, `orders/services.py`.",
        ": management command `import_orders.py` by its path; class `Command` L6–L8; id \
         `import-orders`; reaches `orders/services.py`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    laid(
        &turns[0],
        &[
            "manage.py",
            "shop/settings.py",
            "shop/urls.py",
            "orders/urls.py",
            "orders/views.py",
            "orders/services.py",
            "orders/signals.py",
            "orders/models.py",
            "orders/admin.py",
            "orders/management/commands/import_orders.py",
        ],
        &["orders/migrations/0001_initial.py"],
    );
    assert!(
        turns[0].contains("- `orders/tests.py#L1` — recent"),
        "the application's tests state behaviours: {}",
        turns[0]
    );
    let last_module = turns[0].find("### `shop/__init__.py`").expect("the modules are laid");
    let tests = turns[0].find("### `orders/tests.py`").expect("the tests are laid");
    assert!(last_module < tests, "a `tests.py` is laid after the modules, never among them");
}

// A Flask service built by a factory: the server constructs the application
// at load from the tree's own factory, which registers two blueprints — one
// under a prefix of its own choosing, one under none — and each blueprint
// declares its routes with a prefix of its own.
const FLASK: [(&str, &str); 6] = [
    ("wsgi.py", "from app import create_app\n\napp = create_app()\n"),
    (
        "app/__init__.py",
        "from flask import Flask\n\nfrom .auth.routes import bp as auth_bp\nfrom .orders.routes \
         import bp as orders_bp\n\n\ndef create_app() -> Flask:\n    app = Flask(__name__)\n    \
         app.register_blueprint(orders_bp, url_prefix=\"/orders\")\n    \
         app.register_blueprint(auth_bp)\n    return app\n",
    ),
    ("app/auth/__init__.py", ""),
    (
        "app/auth/routes.py",
        "from flask import Blueprint\n\nbp = Blueprint(\"auth\", __name__, \
         url_prefix=\"/auth\")\n\n\n@bp.post(\"/login\")\ndef login() -> dict:\n    return \
         {\"token\": \"t\"}\n",
    ),
    ("app/orders/__init__.py", ""),
    (
        "app/orders/routes.py",
        "from flask import Blueprint, request\n\nbp = Blueprint(\"orders\", __name__, \
         url_prefix=\"/legacy\")\n\n\n@bp.route(\"/\", methods=[\"POST\"])\ndef create() -> \
         dict:\n    return request.json\n\n\n@bp.get(\"/<int:order_id>\")\ndef \
         get_order(order_id: int) -> dict:\n    return {\"id\": order_id}\n",
    ),
];

// A blueprint registered under a prefix mounts its module's routes there,
// over the prefix the blueprint names for itself; one registered under none
// keeps its own. A route's resource under its mount is the stem, in place
// of the survey's, and the verb and the path past the resource tell the
// routes under one stem apart — a `route` by its `methods=`, a parameter by
// its bare name. The factory's construction at load makes the server the
// bootstrap.
#[tokio::test]
async fn python_mounts() {
    let project = scratch();
    modules(&project, &FLASK);
    let survey = inventory(
        &[
            ("POST /orders/", "app/orders/routes.py#L6-L8", "legacy"),
            ("GET /orders/<int:order_id>", "app/orders/routes.py#L11-L13", "legacy"),
            ("POST /auth/login", "app/auth/routes.py#L6-L8", "login"),
        ],
        &[],
    );

    let model = mined(PYTHON, &project, &survey, 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "wsgi.py", "start"),
            ("POST /orders/", "app/orders/routes.py", "orders"),
            ("GET /orders/<int:order_id>", "app/orders/routes.py", "orders"),
            ("POST /auth/login", "app/auth/routes.py", "auth"),
        ]
    );
    for note in [
        "the process bootstrap, run at load: ",
        "; id `start`; reaches `app/__init__.py`, `app/auth/routes.py`, `app/orders/routes.py`.",
        ": def `create` L6–L8; under `@bp.route(\"/\")`; through `flask`; id `orders.post`; \
         reaches nothing beyond its entry.",
        ": def `get_order` L11–L13; under `@bp.get(\"/<int:order_id>\")`; through `flask`; id \
         `orders.get-order-id`; reaches nothing beyond its entry.",
        ": def `login` L6–L8; under `@bp.post(\"/login\")`; through `flask`; id `auth`; reaches \
         nothing beyond its entry.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    for stem in ["`legacy`", "`login`"] {
        assert!(!turns[0].contains(&format!("stem {stem}")), "gave way: {}", turns[0]);
    }
}

// A router included beneath a router: the entry includes the package's
// router under the resource, and that router includes the versioned one
// under it. The inner module's mount joins the outer prefix however the
// modules sort — the package's `__init__` reads before the entry — so the
// stem its routes spell is the resource the outer include names, and the
// path past it tells them apart.
#[tokio::test]
async fn python_mounts_nested() {
    let project = scratch();
    modules(
        &project,
        &[
            ("app/__init__.py", ""),
            (
                "app/main.py",
                "from fastapi import FastAPI\n\nfrom .billing import router\n\napp = \
                 FastAPI()\napp.include_router(router, prefix=\"/billing\")\n",
            ),
            (
                "app/billing/__init__.py",
                "from fastapi import APIRouter\n\nfrom . import v1\n\nrouter = \
                 APIRouter()\nrouter.include_router(v1.router, prefix=\"/v1\")\n",
            ),
            (
                "app/billing/v1.py",
                "from fastapi import APIRouter\n\nrouter = APIRouter()\n\n\n@router.get(\"\")\ndef \
                 list_invoices() -> list:\n    return []\n\n\n@router.post(\"/{invoice_id}/pay\")\n\
                 def pay(invoice_id: str) -> dict:\n    return {\"id\": invoice_id}\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("GET /billing/v1", "app/billing/v1.py#L6-L8", "invoices"),
            ("POST /billing/v1/{invoice_id}/pay", "app/billing/v1.py#L11-L13", "pay"),
        ],
        &[],
    );
    let answer = claim("billing.get-v1");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "app/main.py", "start"),
            ("GET /billing/v1", "app/billing/v1.py", "billing"),
            ("POST /billing/v1/{invoice_id}/pay", "app/billing/v1.py", "billing"),
        ]
    );
    for note in [
        ": def `list_invoices` L6–L8; under `@router.get(\"\")`; through `fastapi`; id \
         `billing.get-v1`;",
        ": def `pay` L11–L13; under `@router.post(\"/{invoice_id}/pay\")`; through `fastapi`; id \
         `billing.post-v1-invoice-id-pay`;",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
}

// A router declared in the module that includes it: the module is mounted
// under the prefix the include passes, as it is under one that imports the
// router, so the resource that prefix spells is the stem its routes lead
// with — in place of the survey's — and the verb and the path past it tell
// them apart.
#[tokio::test]
async fn python_mounts_local() {
    let project = scratch();
    modules(
        &project,
        &[
            ("app/__init__.py", ""),
            (
                "app/main.py",
                "from fastapi import APIRouter, FastAPI\n\napp = FastAPI()\nrouter = \
                 APIRouter()\n\n\n@router.get(\"\")\ndef list_orders() -> list:\n    return \
                 []\n\n\n@router.post(\"/{order_id}/pay\")\ndef pay(order_id: str) -> dict:\n    \
                 return {\"id\": order_id}\n\n\napp.include_router(router, prefix=\"/orders\")\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("GET /orders", "app/main.py#L7-L9", "listing"),
            ("POST /orders/{order_id}/pay", "app/main.py#L12-L14", "pay"),
        ],
        &[],
    );

    let model = mined(PYTHON, &project, &survey, 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "app/main.py", "start"),
            ("GET /orders", "app/main.py", "orders"),
            ("POST /orders/{order_id}/pay", "app/main.py", "orders"),
        ]
    );
    for note in [
        ": def `list_orders` L7–L9; under `@router.get(\"\")`; through `fastapi`; id \
         `orders.get`;",
        ": def `pay` L12–L14; under `@router.post(\"/{order_id}/pay\")`; through `fastapi`; id \
         `orders.post-order-id-pay`;",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    for stem in ["`listing`", "`pay`"] {
        assert!(!turns[0].contains(&format!("stem {stem}")), "gave way: {}", turns[0]);
    }
}

// Django's namespaced include — `include((module, namespace))`, the module
// the first element of a tuple — mounts the module it names as the bare
// string does, through two levels: the project's `urls.py` includes the
// API's under `api/`, the API's includes the application's under
// `orders/`, so a route on `create/` there is `orders`, the resource past
// `api`, in place of the survey's own.
#[tokio::test]
async fn python_mounts_namespaced() {
    let project = scratch();
    modules(
        &project,
        &[
            (
                "manage.py",
                "import os\nimport sys\n\n\ndef main() -> None:\n    \
                 os.environ.setdefault(\"DJANGO_SETTINGS_MODULE\", \"shop.settings\")\n    from \
                 django.core.management import execute_from_command_line\n\n    \
                 execute_from_command_line(sys.argv)\n\n\nif __name__ == \"__main__\":\n    \
                 main()\n",
            ),
            ("shop/__init__.py", ""),
            ("shop/settings.py", "ROOT_URLCONF = \"shop.urls\"\n"),
            (
                "shop/urls.py",
                "from django.urls import include, path\n\nurlpatterns = [\n    path(\"api/\", \
                 include((\"shop.api.urls\", \"api\"))),\n]\n",
            ),
            ("shop/api/__init__.py", ""),
            (
                "shop/api/urls.py",
                "from django.urls import include, path\n\nurlpatterns = [\n    \
                 path(\"orders/\", include((\"orders.urls\", \"orders\"))),\n]\n",
            ),
            ("orders/__init__.py", ""),
            (
                "orders/urls.py",
                "from django.urls import path\n\nfrom . import views\n\nurlpatterns = [\n    \
                 path(\"\", views.list_orders, name=\"list\"),\n    path(\"create/\", \
                 views.create_order, name=\"create\"),\n]\n",
            ),
            (
                "orders/views.py",
                "from django.http import JsonResponse\n\n\ndef list_orders(request):\n    return \
                 JsonResponse({\"orders\": []})\n\n\ndef create_order(request):\n    return \
                 JsonResponse({\"id\": 1}, status=201)\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("GET /api/orders/", "orders/urls.py#L6", "orders"),
            ("POST /api/orders/create/", "orders/urls.py#L7", "create"),
        ],
        &[],
    );

    let model = mined(PYTHON, &project, &survey, 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "manage.py", "start"),
            ("GET /api/orders/", "orders/urls.py", "orders"),
            ("POST /api/orders/create/", "orders/urls.py", "orders"),
        ]
    );
    for note in [
        "; id `start`; reaches `shop/settings.py`, `shop/urls.py`, `shop/api/urls.py`, \
         `orders/urls.py`.",
        ": registered L6 at module level; through `django`; id `orders.list-orders`; reaches \
         `orders/views.py`.",
        ": registered L7 at module level; through `django`; id `orders.create-order`; reaches \
         `orders/views.py`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    assert!(!turns[0].contains("stem `create`"), "gave way: {}", turns[0]);
}

// A decorator's head is a binding of the tree, not an import: it is traced
// through the module-level construction (`app = Flask(..)`) and one import
// hop (`from .app import app`) to the package, so the `def` under it is a
// surface through `flask`. A decorator that hooks the application's
// lifecycle (`errorhandler`, `before_request`) or shapes a class or a
// member (`dataclass`, attrs' `define`, pydantic's `computed_field`)
// registers nothing, and is no fact.
#[tokio::test]
async fn python_decorated() {
    let project = scratch();
    modules(
        &project,
        &[
            ("wsgi.py", "from app import views\nfrom app.app import app\n\napp.run()\n"),
            ("app/__init__.py", ""),
            ("app/app.py", "from flask import Flask\n\napp = Flask(__name__)\n"),
            (
                "app/views.py",
                "from dataclasses import dataclass\n\nfrom attrs import define\nfrom pydantic \
                 import BaseModel, computed_field\n\nfrom .app import app\n\n\n@dataclass\nclass \
                 Page:\n    size: int = 20\n\n\n@define\nclass Cursor:\n    after: str\n\n\nclass \
                 Window(BaseModel):\n    size: int = 20\n\n    @computed_field\n    @property\n    \
                 def wide(self) -> bool:\n        return self.size > 50\n\n\n@app.get(\"/users\")\n\
                 def list_users() -> dict:\n    return {\"page\": Page().size}\n\n\n\
                 @app.errorhandler(404)\ndef not_found(error) -> dict:\n    return {\"error\": \
                 \"not-found\"}\n\n\n@app.before_request\ndef audit() -> None:\n    pass\n",
            ),
        ],
    );
    let survey = inventory(&[("GET /users", "app/views.py#L28-L30", "users")], &[]);
    let answer = claim("users.list");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    assert!(
        facts.contains(
            "- `app/views.py#L28-L30` — `@app.get(\"/users\")` on `list_users`, through \
                        `flask`"
        ),
        "{facts}"
    );
    for absent in ["errorhandler", "before_request", "dataclass", "define", "computed_field"] {
        assert!(!facts.contains(&format!("`@{absent}")), "`{absent}` registers nothing: {facts}");
        assert!(!facts.contains(&format!("`@app.{absent}")), "`{absent}` is a hook: {facts}");
    }
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("start", "wsgi.py", "start"), ("GET /users", "app/views.py", "users")]
    );
    assert!(
        turns[0].contains(
            ": def `list_users` L28–L30; under `@app.get(\"/users\")`; through `flask`; id \
             `users`; reaches `app/app.py`."
        ),
        "{}",
        turns[0]
    );
}

// A command-line tool with jobs beside it, in the `src` layout: the manifest
// installs a console script onto a `click` group, whose commands are the
// group's decorated functions; a scheduler is handed a job inside a plain
// function, and a listener beside it; a worker module declares a task under
// its application's decorator; a hooks module hands functions to the
// process, the lifecycle, and wrappers, registering nothing.
const LEDGER: [(&str, &str); 8] = [
    (
        "pyproject.toml",
        "[project]\nname = \"ledger\"\n\n[project.scripts]\nledger = \"ledger.cli:cli\"\n",
    ),
    ("src/ledger/__init__.py", ""),
    (
        "src/ledger/cli.py",
        "import click\n\nfrom .importer import run_import\nfrom .jobs import \
         schedule\n\n\n@click.group()\ndef cli() -> None:\n    pass\n\n\n@cli.command(\"import\")\n\
         @click.argument(\"file\")\ndef import_command(file: str) -> None:\n    \
         run_import(file)\n\n\n@cli.command()\ndef serve() -> None:\n    \
         schedule().start()\n\n\nif __name__ == \"__main__\":\n    cli()\n",
    ),
    (
        "src/ledger/jobs.py",
        "from apscheduler.schedulers.blocking import BlockingScheduler\n\nfrom .bus import \
         Bus\n\n\ndef schedule() -> BlockingScheduler:\n    scheduler = BlockingScheduler()\n    \
         scheduler.add_job(nightly, \"cron\", hour=2)\n    scheduler.add_listener(on_error, \
         mask=1)\n    Bus().on(\"paid\", lambda id: print(id))\n    return scheduler\n\n\ndef \
         nightly() -> None:\n    print(\"swept\")\n\n\ndef on_error(event) -> None:\n    \
         print(event)\n\n\ndef retried(fn):\n    return sorted([fn], key=lambda f: f.__name__)\n",
    ),
    ("src/ledger/importer.py", "def run_import(file: str) -> None:\n    print(file)\n"),
    (
        "src/ledger/bus.py",
        "class Bus:\n    def on(self, event: str, handler) -> None:\n        handler(event)\n",
    ),
    (
        "src/ledger/worker.py",
        "from celery import Celery\n\nfrom .bus import Bus\n\napp = Celery(\"ledger\")\n\n\n\
         @app.task(name=\"invoices.send\", max_retries=3)\ndef send_invoice(order_id: int) -> \
         None:\n    Bus().on(\"sent\", lambda id: None)\n",
    ),
    (
        "src/ledger/hooks.py",
        "import atexit\nimport signal\nfrom functools import partial\n\nfrom fastapi import \
         Depends, FastAPI\n\nfrom .bus import Bus\n\napp = FastAPI(lifespan=lambda app: \
         None)\n\n\ndef shutdown() -> None:\n    pass\n\n\ndef get_bus() -> Bus:\n    return \
         Bus()\n\n\natexit.register(shutdown)\nsignal.signal(signal.SIGTERM, lambda signum, \
         frame: shutdown())\napp.add_event_handler(\"startup\", shutdown)\nretry = \
         partial(shutdown)\ndependency = Depends(get_bus)\n",
    ),
];

// The survey answer over `LEDGER`: the two commands at their decorated
// functions, the job at its registration, the task at its decorated
// function; the hooks module under no surface.
fn ledger_inventory() -> String {
    inventory(
        &[
            ("import command", "src/ledger/cli.py#L12-L15", "ledger-import"),
            ("serve command", "src/ledger/cli.py#L18-L20", "serve"),
            ("nightly job", "src/ledger/jobs.py#L8", "nightly"),
            ("invoices task", "src/ledger/worker.py#L8-L10", "invoices"),
        ],
        &["src/ledger/hooks.py"],
    )
}

// A decorated `def` the survey anchors is read for what its decorator
// spells: the command's literal gives the stem, in place of the survey's
// where the two differ; a task's `name=` gives its own by its first
// segment; a decorator spelling none leaves the survey's standing. A job
// handed to a scheduler by name is read at its registration. A listener
// handed to the scheduler the survey left unnamed is no surface, and
// neither is the handler handed to the tree's own class (`Bus().on`).
#[tokio::test]
async fn python_registrations() {
    let project = scratch();
    modules(&project, &LEDGER);
    let survey = ledger_inventory();
    let answer = claim("invoices.send");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/ledger/cli.py", "start"),
            ("import command", "src/ledger/cli.py", "import"),
            ("serve command", "src/ledger/cli.py", "serve"),
            ("nightly job", "src/ledger/jobs.py", "nightly"),
            ("invoices task", "src/ledger/worker.py", "invoices"),
        ]
    );
    for note in [
        "the process bootstrap, run by the console script `ledger`, which calls `cli()`: ",
        "; id `start`; reaches `src/ledger/importer.py`, `src/ledger/jobs.py`.",
        ": def `import_command` L12–L15; under `@cli.command(\"import\")`; through `click`; id \
         `import`; reaches `src/ledger/importer.py`, `src/ledger/jobs.py`, `src/ledger/bus.py`.",
        ": def `serve` L18–L20; under `@cli.command`; through `click`; id `serve`; reaches",
        ": registered L8 in `schedule`; through `apscheduler`; id `nightly`; reaches \
         `src/ledger/bus.py`.",
        ": def `send_invoice` L8–L10; under `@app.task(\"invoices.send\")`; through `celery`; id \
         `invoices`; reaches `src/ledger/bus.py`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
}

// A route registered by a call named for its verb is a registration under
// every verb alike: `web.patch(..)` hands its handler as `web.post(..)`
// does. `patch` is structural only spelled as `unittest.mock`'s — bare or
// through `mock` — so a stubs module handing lambdas to it registers
// nothing.
#[tokio::test]
async fn python_registrations_patch() {
    let project = scratch();
    modules(
        &project,
        &[
            ("app/__init__.py", ""),
            (
                "app/server.py",
                "from aiohttp import web\n\nfrom .handlers import create, update\n\napp = \
                 web.Application()\napp.add_routes([\n    web.post(\"/orders\", create),\n    \
                 web.patch(\"/orders/{id}\", update),\n])\n",
            ),
            (
                "app/handlers.py",
                "async def create(request):\n    return {}\n\n\nasync def update(request):\n    \
                 return {}\n",
            ),
            (
                "app/stubs.py",
                "from unittest import mock\nfrom unittest.mock import patch\n\n\ndef \
                 stub_create():\n    return patch(\"app.handlers.create\", lambda request: \
                 {})\n\n\ndef stub_update():\n    return mock.patch(\"app.handlers.update\", \
                 lambda request: {})\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("POST /orders", "app/server.py#L7", "orders"),
            ("PATCH /orders/{id}", "app/server.py#L8", "orders"),
        ],
        &[],
    );
    let answer = claim("orders.update");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    for fact in [
        "- `app/server.py#L7` — `web.post` led by `\"/orders\"` handed a function, through \
         `aiohttp`",
        "- `app/server.py#L8` — `web.patch` led by `\"/orders/{id}\"` handed a function, through \
         `aiohttp`",
    ] {
        assert!(facts.contains(fact), "{fact} is among the facts: {facts}");
    }
    assert!(!facts.contains("app/stubs.py#"), "a mock's `patch` registers nothing: {facts}");
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("POST /orders", "app/server.py", "orders"),
            ("PATCH /orders/{id}", "app/server.py", "orders"),
        ]
    );
    for note in [
        ": registered L7 at module level; through `aiohttp`; id `orders.create`; reaches \
         `app/handlers.py`.",
        ": registered L8 at module level; through `aiohttp`; id `orders.update`; reaches \
         `app/handlers.py`.",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
}

// The survey turn carries what the parser read of the tree, for the model to
// name its surfaces from: the manifest and the scripts it installs, the
// bootstrap and how it runs, every call outside a handler that hands a
// function to something a package provides — with the package and the
// class the receiver was constructed as — every `def` under a package's
// decorator, what the entry modules export, and the production modules;
// the manifest and the modules that locate a surface are laid into it
// whole. A function handed to the tree's own class, a wrapper, a lifecycle
// hook, or a hook keyword is no fact.
#[tokio::test]
async fn python_survey_facts() {
    let project = scratch();
    modules(&project, &LEDGER);
    let survey = ledger_inventory();
    let answer = claim("import.run-import");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let survey = &seen[0].messages[0];
    for fact in [
        "The manifest names the package `ledger`; installs the console scripts `ledger` runs \
         `ledger.cli:cli`.",
        "The bootstrap — the entry that runs something — is `src/ledger/cli.py`: the console \
         script `ledger` calls its `cli()`.",
        "- `src/ledger/jobs.py#L8` — `scheduler.add_job` handed a function, through `apscheduler` \
         as `BlockingScheduler`",
        "- `src/ledger/jobs.py#L9` — `scheduler.add_listener` handed a function, through \
         `apscheduler` as `BlockingScheduler`",
        "- `src/ledger/cli.py#L7-L9` — `@click.group` on `cli`, through `click`",
        "- `src/ledger/cli.py#L12-L15` — `@cli.command(\"import\")` on `import_command`, through \
         `click`",
        "- `src/ledger/cli.py#L18-L20` — `@cli.command` on `serve`, through `click`",
        "- `src/ledger/worker.py#L8-L10` — `@app.task(\"invoices.send\")` on `send_invoice`, \
         through `celery`",
        "- `src/ledger/cli.py` exports `cli` (function) L7–L9, `import_command` (function) \
         L12–L15, `serve` (function) L18–L20",
        "- `src/ledger/worker.py` exports `app` (value) L5, `send_invoice` (function) L8–L10",
        "- `src/ledger/bus.py`",
    ] {
        assert!(survey.contains(fact), "{fact} is among the facts: {survey}");
    }
    assert!(!survey.contains("`Bus"), "the tree's own receiver is no fact: {survey}");
    assert!(!survey.contains("src/ledger/hooks.py#"), "nothing in the hooks registers: {survey}");
    assert!(
        !survey.contains("src/ledger/cli.py#L24"),
        "a call handed no function is none: {survey}"
    );
    let at = |file: &str| {
        survey
            .find(&format!("### `{file}` ("))
            .unwrap_or_else(|| panic!("`{file}` is laid: {survey}"))
    };
    assert!(
        at("pyproject.toml") < at("src/ledger/cli.py")
            && at("src/ledger/cli.py") < at("src/ledger/jobs.py"),
        "the manifest, the bootstrap, then the modules that locate a surface: {survey}"
    );
    assert!(seen[0].workspace.is_some(), "the tree is lent to the survey");
    assert!(seen[2].workspace.is_none(), "the inline value lends nothing and is not surveyed");
}

// The survey's stem stands where the code spells none at the anchor — a
// command decorator with no literal, a job led by a function — and gives
// way where it does: a command's literal, a task's `name=`, and a route's
// resource under its router's prefix and the prefix it is included under
// replace the survey's reading, so the seam's stems are the code's.
#[tokio::test]
async fn python_survey_stem_derived() {
    let project = scratch();
    modules(&project, &LEDGER);
    let survey = inventory(
        &[
            ("import command", "src/ledger/cli.py#L12-L15", "ledger-import"),
            ("serve command", "src/ledger/cli.py#L18-L20", "serving"),
            ("nightly job", "src/ledger/jobs.py#L8", "sweep"),
            ("invoices task", "src/ledger/worker.py#L8-L10", "send-invoice"),
        ],
        &["src/ledger/hooks.py"],
    );
    let answer = claim("invoices.send");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/ledger/cli.py", "start"),
            ("import command", "src/ledger/cli.py", "import"),
            ("serve command", "src/ledger/cli.py", "serving"),
            ("nightly job", "src/ledger/jobs.py", "sweep"),
            ("invoices task", "src/ledger/worker.py", "invoices"),
        ]
    );
    for stem in ["`ledger-import`", "`send-invoice`"] {
        assert!(!turns[0].contains(&format!("stem {stem}")), "gave way: {}", turns[0]);
    }

    // a route under a prefixed router included under a prefix
    let project = scratch();
    modules(&project, &PY_APP);
    let survey = inventory(
        &[
            ("POST /api/orders", "app/routers/orders.py#L8-L10", "api-orders"),
            ("GET /api/orders/{order_id}", "app/routers/orders.py#L13-L15", "api-orders"),
            ("GET /health", "app/routers/health.py#L6-L8", "health"),
        ],
        &[],
    );

    let model = mined(PYTHON, &project, &survey, 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    for id in ["id `orders.post`;", "id `orders.get-order-id`;", "id `health`;"] {
        assert!(turns[0].contains(id), "{id} is in the brief: {}", turns[0]);
    }
    assert!(!turns[0].contains("api-orders"), "the survey's stem gave way: {}", turns[0]);
}

// A router that declares no prefix and is included under none groups its
// routes under its first tag, whatever each path spells, so one router is
// one stem; a prefix, its own or its inclusion's, stands over the tag.
#[tokio::test]
async fn python_survey_stem_tagged() {
    let project = scratch();
    modules(
        &project,
        &[
            ("app/__init__.py", ""),
            (
                "app/main.py",
                "from fastapi import FastAPI\n\nfrom .routes import items, login\n\napp = \
                 FastAPI()\napp.include_router(login.router)\napp.include_router(items.router)\n",
            ),
            ("app/routes/__init__.py", ""),
            (
                "app/routes/login.py",
                "from fastapi import APIRouter\n\nrouter = APIRouter(tags=[\"login\"])\n\n\n\
                 @router.post(\"/login/access-token\")\ndef login_access_token() -> dict:\n    \
                 return {}\n\n\n@router.post(\"/password-recovery/{email}\")\ndef \
                 recover_password(email: str) -> dict:\n    return {\"email\": email}\n",
            ),
            (
                "app/routes/items.py",
                "from fastapi import APIRouter\n\nrouter = APIRouter(prefix=\"/items\", \
                 tags=[\"things\"])\n\n\n@router.get(\"/\")\ndef read_items() -> list:\n    \
                 return []\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("POST /login/access-token", "app/routes/login.py#L6-L8", "login"),
            ("POST /password-recovery/{email}", "app/routes/login.py#L11-L13", "password-recovery"),
            ("GET /items/", "app/routes/items.py#L6-L8", "things"),
        ],
        &[],
    );
    let answer = claim("login.post-access-token");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "app/main.py", "start"),
            ("POST /login/access-token", "app/routes/login.py", "login"),
            ("POST /password-recovery/{email}", "app/routes/login.py", "login"),
            ("GET /items/", "app/routes/items.py", "items"),
        ]
    );
    for id in
        ["id `login.post-access-token`;", "id `login.post-password-recovery-email`;", "id `items`;"]
    {
        assert!(turns[0].contains(id), "{id} is in the brief: {}", turns[0]);
    }
    for stem in ["`password-recovery`", "`things`"] {
        assert!(!turns[0].contains(&format!("stem {stem}")), "gave way: {}", turns[0]);
    }
}

// The survey names a surface under `start` — the bootstrap's stem, which
// the caller names itself — and the finding sends the answer back for the
// stem of what a caller does through the surface; the corrected answer is
// accepted, and the run goes on to mine.
#[tokio::test]
async fn python_survey_start_stem() {
    let project = scratch();
    modules(&project, &PY_APP);
    let strayed = inventory(
        &[
            ("POST /api/orders", "app/routers/orders.py#L8-L10", "start"),
            ("GET /api/orders/{order_id}", "app/routers/orders.py#L13-L15", "orders"),
            ("GET /health", "app/routers/health.py#L6-L8", "health"),
        ],
        &[],
    );
    let corrected = py_app_inventory();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&strayed, &corrected, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "two survey rounds, one seam, one for the inline value");
    for request in &seen[..2] {
        system(request, prompt::survey(python::PROSE));
    }
    system(&seen[2], prompt::extract(python::PROSE));
    let exchanges = model.exchanges();
    let finding = exchanges[0].outcome.as_ref().expect_err("the `start` stem is refused");
    assert!(
        finding.contains(
            "- surface `POST /api/orders`: `start` is the bootstrap's stem, which the caller names \
             itself; give the surface the stem of what a caller does through it"
        ),
        "the finding names the surface and the stem: {finding}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected inventory is accepted");
    assert_eq!(surfaces(&seen[2].messages[0]).len(), 4, "{}", seen[2].messages[0]);
}

// A module the facts list a decorated task in — a worker nothing imports —
// that no named surface reaches and the answer does not list under
// `unreached` is a finding, naming the module; the answer that names the
// task's surface is accepted, and the module is that surface's to reach.
#[tokio::test]
async fn python_survey_unplaced() {
    let project = scratch();
    modules(&project, &LEDGER);
    let partial = inventory(
        &[
            ("import command", "src/ledger/cli.py#L12-L15", "import"),
            ("serve command", "src/ledger/cli.py#L18-L20", "serve"),
            ("nightly job", "src/ledger/jobs.py#L8", "nightly"),
        ],
        &["src/ledger/hooks.py"],
    );
    let complete = ledger_inventory();
    let answer = claim("invoices.remind");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&partial, &complete, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "two survey rounds, one seam, one for the inline value");
    let exchanges = model.exchanges();
    let finding = exchanges[0].outcome.as_ref().expect_err("the unplaced worker is refused");
    assert!(
        finding.contains(
            "- one module the facts list a registration or declaration in is reached by no \
             surface you named and not listed under `unreached`: `src/ledger/worker.py`;"
        ),
        "the finding names the module: {finding}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the completed inventory is accepted");
    let turn = &seen[2].messages[0];
    assert_eq!(surfaces(turn).len(), 5, "{turn}");
    assert!(turn.contains("id `invoices`; reaches `src/ledger/bus.py`."), "{turn}");
}

// A view set handed to a router by name is one surface at the registration:
// the literal gives the stem, the class's actions each give an id beside
// the surface's own — a verb method would, an undecorated or private one
// does not — and the survey's facts list the registration alone, never an
// action's decorator as a surface of its own.
#[tokio::test]
async fn python_survey_methods() {
    let project = scratch();
    modules(
        &project,
        &[
            DJANGO[0],
            ("shop/__init__.py", ""),
            ("shop/settings.py", "ROOT_URLCONF = \"shop.urls\"\n"),
            (
                "shop/urls.py",
                "from rest_framework.routers import DefaultRouter\n\nfrom orders.views import \
                 OrderViewSet\n\nrouter = DefaultRouter()\nrouter.register(\"orders\", \
                 OrderViewSet)\nurlpatterns = router.urls\n",
            ),
            ("orders/__init__.py", ""),
            (
                "orders/views.py",
                "from rest_framework.decorators import action\nfrom rest_framework.viewsets \
                 import ViewSet\n\nfrom .services import cancel, recent\n\n\nclass \
                 OrderViewSet(ViewSet):\n    def list(self, request):\n        return \
                 recent()\n\n    @action(detail=True, methods=[\"post\"])\n    def cancel(self, \
                 request, pk=None):\n        return cancel(pk)\n\n    def _audit(self) -> \
                 None:\n        pass\n",
            ),
            (
                "orders/services.py",
                "def recent() -> list:\n    return []\n\n\ndef cancel(pk: int) -> dict:\n    \
                 return {\"id\": pk}\n",
            ),
        ],
    );
    let survey = inventory(&[("orders view set", "shop/urls.py#L6", "orders")], &[]);

    let model = mined(PYTHON, &project, &survey, 1).await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    let registration = "- `shop/urls.py#L6` — `router.register` led by `\"orders\"` handed a \
                        function, through `rest_framework` as `DefaultRouter`";
    assert!(facts.contains(registration), "{registration} is among the facts: {facts}");
    assert!(
        !facts.contains("`@action` on `OrderViewSet.cancel`"),
        "an action of a handed view set is the registration's to carry, not a decorated surface \
         of its own: {facts}"
    );
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("start", "manage.py", "start"), ("orders view set", "shop/urls.py", "orders")]
    );
    assert!(
        turns[0].contains(
            ": registered L6 at module level; through `rest_framework`; class `OrderViewSet` \
             handed; methods `cancel` L11–L13; ids `orders`, `orders.cancel`; reaches \
             `orders/views.py`, `orders/services.py`."
        ),
        "the class carries its actions as ids: {}",
        turns[0]
    );
}

// A library's package `__init__` only imports and lists `__all__`, so the
// tree has no bootstrap; the survey turn lists what it re-exports, one hop,
// at the modules that declare them — an enumeration and a dataclass of
// fields and properties alone declare data, and are none — and the surfaces
// the survey names there are read at their declarations: a class carries an
// id per public method beside its own, a private or a dunder method none; a
// function reaches what it imports.
#[tokio::test]
async fn python_exports() {
    let project = scratch();
    modules(
        &project,
        &[
            ("pyproject.toml", "[project]\nname = \"money\"\n"),
            (
                "src/money/__init__.py",
                "from .money import Currency, Money, Rate\nfrom .parse import ParseError, parse\n\n\
                 __all__ = [\"Currency\", \"Money\", \"Rate\", \"parse\", \"ParseError\"]\n",
            ),
            (
                "src/money/money.py",
                "from dataclasses import dataclass\nfrom enum import Enum\n\n\nclass Currency(str, \
                 Enum):\n    GBP = \"GBP\"\n    EUR = \"EUR\"\n\n    @property\n    def symbol(self) \
                 -> str:\n        return self.value.lower()\n\n\n@dataclass(frozen=True)\nclass \
                 Rate:\n    currency: Currency\n    value: int\n\n    @property\n    def \
                 inverse(self) -> int:\n        return -self.value\n\n\n@dataclass(frozen=True)\n\
                 class Money:\n    amount: int\n    currency: Currency\n\n    def add(self, other: \
                 \"Money\") -> \"Money\":\n        return Money(self.amount + other.amount, \
                 self.currency)\n\n    def _check(self) -> None:\n        pass\n\n    def \
                 __str__(self) -> str:\n        return str(self.amount)\n",
            ),
            (
                "src/money/parse.py",
                "from .money import Money\n\n\nclass ParseError(ValueError):\n    pass\n\n\ndef \
                 parse(text: str) -> Money:\n    return Money(int(text), \"GBP\")\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("Money", "src/money/money.py#L24-L36", "money"),
            ("parse", "src/money/parse.py#L8-L9", "parse"),
        ],
        &["src/money/__init__.py"],
    );
    let answer = claim("parse.text");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    for fact in [
        "No bootstrap runs at load",
        "- `src/money/__init__.py` re-exports from `src/money/money.py`, where each is declared: \
         `Money` (class) L24–L36",
        "- `src/money/__init__.py` re-exports from `src/money/parse.py`, where each is declared: \
         `ParseError` (class) L4–L5, `parse` (function) L8–L9",
    ] {
        assert!(facts.contains(fact), "{fact} is among the facts: {facts}");
    }
    for data in ["`Currency`", "`Rate`"] {
        assert!(
            !facts.contains(data),
            "{data} declares data alone, no export a caller calls: {facts}"
        );
    }
    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [("Money", "src/money/money.py", "money"), ("parse", "src/money/parse.py", "parse")]
    );
    assert!(
        turns[0].contains(
            ": exported class L24–L36; methods `add` L29–L30; ids `money`, `money.add`; reaches \
             nothing beyond its entry."
        ),
        "public methods only, each an id — a private or a dunder one none: {}",
        turns[0]
    );
    assert!(
        turns[0].contains(": exported function L8–L9; id `parse`; reaches `src/money/money.py`."),
        "{}",
        turns[0]
    );
}

// The `type` claims are the declarations the parser read — every exported
// enum, alias, dataclass, and model class of the modules the seams reach,
// anchored at its lines; one a module keeps to itself is none — and a
// `type` the model answers gives way to them; an inline value's
// declarations are copied too, with no file to anchor in.
#[tokio::test]
async fn python_types() {
    let project = scratch();
    modules(
        &project,
        &[
            ("pyproject.toml", "[project]\nname = \"money\"\n"),
            ("src/money/__init__.py", "from .money import Money, parse\n"),
            (
                "src/money/money.py",
                "from dataclasses import dataclass\nfrom enum import Enum\nfrom typing import \
                 Literal\n\nfrom pydantic import BaseModel\n\nCurrency = Literal[\"GBP\", \
                 \"EUR\"]\n\n\nclass Rounding(Enum):\n    BANKERS = 1\n    HALF_UP = 2\n\n\ntype \
                 Adjust = int\n\n\n@dataclass\nclass Money:\n    amount: int\n    currency: \
                 Currency\n\n    def add(self, other: \"Money\") -> \"Money\":\n        return \
                 Money(self.amount + other.amount, self.currency)\n\n\nclass Order(BaseModel):\n    \
                 id: str\n    total: Money\n\n\nclass _Ledger:\n    entries: list = []\n\n\ndef \
                 parse(text: str) -> Money:\n    return Money(int(text), \"GBP\")\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("Money", "src/money/money.py#L18-L24", "money"),
            ("parse", "src/money/money.py#L36-L37", "parse"),
        ],
        &["src/money/__init__.py"],
    );
    let answered = serde_json::json!({
        "claims": [
            { "kind": "requirement", "id": "parse.text", "statement": "Parses an amount." },
            { "kind": "type", "name": "Phantom", "signature": "class Phantom: ..." }
        ]
    })
    .to_string();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[
            "types",
            "class Inline:\n    id: str\n\n\nclass _Local:\n    n: int\n",
            "Inline",
            "Currency",
            "Rounding",
            "Adjust",
            "Money",
            "Order",
        ],
        ScriptedModel::answering([&survey, &answered, &answered]),
    )
    .await;

    assert_eq!(model.seen().len(), 3, "the survey, one seam over the tree, one over the value");
}

// An `__all__` declared with an annotation — `__all__: list[str] = [..]` —
// lists the module's exports as the plain form does: a public class or
// function it leaves out is the module's own, so a package's star re-export
// of the module is listed without it and no `type` claim copies it; an
// inline value's annotated list cuts its declarations the same way.
#[tokio::test]
async fn python_exports_annotated_all() {
    let project = scratch();
    modules(
        &project,
        &[
            ("pyproject.toml", "[project]\nname = \"money\"\n"),
            ("src/money/__init__.py", "from .money import *\n"),
            (
                "src/money/money.py",
                "from dataclasses import dataclass\nfrom enum import Enum\n\n__all__: list[str] = \
                 [\"Money\", \"parse\"]\n\n\nclass Currency(str, Enum):\n    GBP = \"GBP\"\n    \
                 EUR = \"EUR\"\n\n\n@dataclass\nclass Money:\n    amount: int\n    currency: \
                 Currency\n\n    def add(self, other: \"Money\") -> \"Money\":\n        return \
                 Money(self.amount + other.amount, self.currency)\n\n\nclass Ledger:\n    entries: \
                 list = []\n\n    def post(self, money: Money) -> None:\n        \
                 self.entries.append(money)\n\n\ndef parse(text: str) -> Money:\n    return \
                 Money(int(text), \"GBP\")\n\n\ndef audit(ledger: Ledger) -> int:\n    return \
                 len(ledger.entries)\n",
            ),
        ],
    );
    let survey = inventory(
        &[
            ("Money", "src/money/money.py#L12-L18", "money"),
            ("parse", "src/money/money.py#L28-L29", "parse"),
        ],
        &["src/money/__init__.py"],
    );
    let answer = claim("parse.text");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[
            "types",
            "__all__: list[str] = [\"Inline\"]\n\n\nclass Inline:\n    id: str\n\n\nclass \
             Local:\n    n: int\n",
            "Inline",
            "Money",
        ],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let seen = model.seen();
    let facts = &seen[0].messages[0];
    assert!(
        facts.contains(
            "- `src/money/__init__.py` re-exports from `src/money/money.py`, where each is \
             declared: `Money` (class) L12–L18, `parse` (function) L28–L29"
        ),
        "the listed names alone are re-exported: {facts}"
    );
    for own in ["`Ledger` (class)", "`audit` (function)"] {
        assert!(!facts.contains(own), "{own} is left off the list, so the module's own: {facts}");
    }
    assert_eq!(seen.len(), 3, "the survey, one seam over the tree, one over the value");
}

// The turn lists what the modules spell as values of their own — an
// environment read with its default, or with whatever the code does to it,
// a named constant at module level or inside a function, a pattern, a
// mapping written out, listed by its head at its range when it runs over
// several lines, a class field — at its anchor, and the packages they
// import with the names bound to them; a construction, a reference to a
// value spelled elsewhere, and a function's working local are none.
#[tokio::test]
async fn python_boundaries() {
    let project = scratch();
    modules(&project, &PY_APP);
    modules(
        &project,
        &[
            (
                "app/config.py",
                "import os\nimport re\nfrom dataclasses import dataclass, field\n\nPORT = \
                 int(os.environ.get(\"PORT\", \"3000\"))\nMAX_LINES = 50\nSKU = \
                 re.compile(r\"^[A-Z]{2,4}-\\d{3,6}$\")\nAPI_TOKENS = [token for token in \
                 os.getenv(\"API_TOKENS\", \"\").split(\",\") if token]\nORDER = {\n    \"id\": \
                 \"string\",\n    \"lines\": \"number\",\n}\n\n\n@dataclass\nclass Limits:\n    \
                 page_size: int = int(os.environ.get(\"PAGE_SIZE\", \"20\"))\n    tags: list = \
                 field(default_factory=list)\n\n\ndef listen(app) -> None:\n    \
                 app.run(port=int(os.environ.get(\"PORT\", \"3000\")))\n\n\ndef settle(sleep) -> \
                 int:\n    FIVE_SEC_DELAY = 5 * 1000\n    attempts = 0\n    \
                 sleep(FIVE_SEC_DELAY)\n    return attempts\n\n\ndef accept(kind: str) -> str:\n    \
                 allowed_kinds = [\"image/jpeg\", \"image/png\"]\n    fallback = \
                 allowed_kinds[0]\n    return kind if kind in allowed_kinds else fallback\n",
            ),
            (
                "app/main.py",
                "from fastapi import FastAPI\n\nfrom .config import PORT\nfrom .routers import \
                 health, orders\n\napp = FastAPI()\nport = PORT\napp.include_router(orders.router, \
                 prefix=\"/api\")\napp.include_router(health.router)\n",
            ),
        ],
    );

    let model = mined(PYTHON, &project, &py_app_inventory(), 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    for line in [
        "- `app/config.py#L5` — `PORT = int(os.environ.get(\"PORT\", \"3000\"))`",
        "- `app/config.py#L6` — `MAX_LINES = 50`",
        "- `app/config.py#L7` — `SKU = re.compile(r\"^[A-Z]{2,4}-\\d{3,6}$\")`",
        "- `app/config.py#L8` — `API_TOKENS = [token for token in os.getenv(\"API_TOKENS\", \
         \"\").split(\",\") if token]`",
        "- `app/config.py#L9-L12` — `ORDER = { \"id\": \"string\", \"lines\": \"number\", }`",
        "- `app/config.py#L17` — `Limits.page_size = int(os.environ.get(\"PAGE_SIZE\", \"20\"))`",
        "- `app/config.py#L22` — env `PORT` in `app.run(port=int(os.environ.get(\"PORT\", \
         \"3000\")))`",
        "- `app/config.py#L26` — `FIVE_SEC_DELAY = 5 * 1000`",
        "- `app/config.py#L33` — `allowed_kinds = [\"image/jpeg\", \"image/png\"]`",
        "- `fastapi` — `FastAPI`, `APIRouter` — in `app/main.py`, `app/routers/orders.py`, \
         `app/routers/health.py`",
    ] {
        assert!(turns[0].contains(line), "{line} is in the brief: {}", turns[0]);
    }
    for absent in [
        "`app = FastAPI()`",
        "`port = PORT`",
        "`attempts = 0`",
        "`fallback = allowed_kinds[0]`",
        "`Limits.tags",
    ] {
        assert!(!turns[0].contains(absent), "{absent} is no boundary: {}", turns[0]);
    }
    assert!(
        !turns[0].contains("env `API_TOKENS` in"),
        "a read a listed binding holds is not listed twice: {}",
        turns[0]
    );
}

// The turn lists the calls the modules make through a package, grouped by
// callee at their sites — a store's statement wherever the service makes
// it, through the connection the store module binds — and none of what is
// structure: the router's inclusion, the construction bound at module
// level, the connection made in a module-level declaration.
#[tokio::test]
async fn python_calls() {
    let project = scratch();
    modules(&project, &PY_APP);
    modules(
        &project,
        &[
            (
                "app/db.py",
                "import os\n\nimport psycopg\n\npool = psycopg.connect(os.environ[\"DATABASE_URL\"])\n\
                 \n\ndef replica() -> psycopg.Connection:\n    return \
                 psycopg.connect(os.environ[\"REPLICA_URL\"], autocommit=True)\n",
            ),
            (
                "app/services/orders.py",
                "from ..db import pool\n\n\nasync def create_order(body: dict) -> dict:\n    \
                 cursor = pool.execute(\"INSERT INTO orders (body) VALUES (%s) RETURNING id\", \
                 [body])\n    pool.commit()\n    return {\"id\": cursor.rowcount}\n\n\ndef \
                 find_order(order_id: str) -> dict:\n    rows = pool.execute(\"SELECT * FROM \
                 orders WHERE id = %s\", [order_id])\n    return {\"id\": rows}\n",
            ),
        ],
    );

    let model = mined(PYTHON, &project, &py_app_inventory(), 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    for line in [
        "- `psycopg:pool.execute` in `app/services/orders.py` at L5, L11",
        "- `psycopg:pool.commit` in `app/services/orders.py` at L6",
        "- `psycopg:connect` in `app/db.py` at L9",
    ] {
        assert!(turns[0].contains(line), "{line}: the store's calls are listed: {}", turns[0]);
    }
    assert!(
        !turns[0].contains("`fastapi:") && !turns[0].contains("at L5, L9"),
        "inclusions, constructions, and declarations are no calls: {}",
        turns[0]
    );
}

// A FastAPI service whose one handler decides: a guard, a match, raises, an
// except, a timer, a loop, an assertion, and a conditional it returns, over
// a named constant and a store.
const PY_DECIDING: [(&str, &str); 4] = [
    ("app/__init__.py", ""),
    (
        "app/main.py",
        "from fastapi import FastAPI\n\nfrom .orders import create_order\n\napp = FastAPI()\n\
         app.add_api_route(\"/orders\", create_order, methods=[\"POST\"])\n",
    ),
    (
        "app/orders.py",
        "import asyncio\n\nfrom .db import pool\n\nRETRY_S = 0.5\n\n\nasync def \
         create_order(body: dict) -> str:\n    if not body.get(\"sku\"):\n        raise \
         ValueError(\"sku-required\")\n    match body.get(\"qty\"):\n        case 0:\n            \
         raise ValueError(\"empty\")\n        case _:\n            pass\n    try:\n        \
         pool.insert(body)\n    except KeyError:\n        await asyncio.sleep(RETRY_S)\n    while \
         body.get(\"retry\"):\n        body[\"retry\"] -= 1\n    assert pool is not None\n    \
         return \"bulk\" if body[\"qty\"] > 1 else \"single\"\n",
    ),
    ("app/db.py", "pool = \"o-1\"\n"),
];

// The survey answer over `PY_DECIDING`: the one route, at its registration.
fn py_deciding_inventory() -> String {
    inventory(&[("POST /orders", "app/main.py#L6", "orders")], &[])
}

// The points where the code decides — a guard, a match, a raise, an except,
// a timer, a loop, an assertion, a conditional — are listed at their lines
// with their text and the function they run in, so a requirement anchors
// where a behaviour starts.
#[tokio::test]
async fn python_decisions() {
    let project = scratch();
    modules(&project, &PY_DECIDING);

    let model = mined(PYTHON, &project, &py_deciding_inventory(), 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    let sections: Vec<&str> = turns[0].split("\n\n").collect();
    let at = sections
        .iter()
        .position(|section| section.starts_with("Decision points"))
        .unwrap_or_else(|| panic!("the section is rendered: {}", turns[0]));
    let listed: Vec<&str> = sections[at + 1].lines().collect();
    assert_eq!(
        listed,
        [
            "- `app/orders.py#L9-L10` — `if not body.get(\"sku\")` in `create_order`",
            "- `app/orders.py#L10` — `raise ValueError(\"sku-required\")` in `create_order`",
            "- `app/orders.py#L11-L15` — `match body.get(\"qty\")` in `create_order`",
            "- `app/orders.py#L13` — `raise ValueError(\"empty\")` in `create_order`",
            "- `app/orders.py#L18-L19` — `except KeyError` in `create_order`",
            "- `app/orders.py#L19` — `asyncio.sleep(RETRY_S)` in `create_order`",
            "- `app/orders.py#L20-L21` — `while body.get(\"retry\")` in `create_order`",
            "- `app/orders.py#L22` — `assert pool is not None` in `create_order`",
            "- `app/orders.py#L23` — `\"bulk\" if body[\"qty\"] > 1 else …` in `create_order`",
        ],
        "each decision at its lines, and the bootstrap decides nothing"
    );
}

// The seam's anchors hold every `requirement` to where the code's behaviour
// starts or its result is decided — a decision, a `return`, a `def`'s head,
// a call through a package, a boundary, a surface's lines: one anchored at
// a line that only imports is the SDK's finding, a `criterion` there is
// not, the corrected answer is accepted, and the inline value is held to no
// anchor.
#[tokio::test]
async fn python_anchors() {
    let project = scratch();
    modules(&project, &PY_DECIDING);
    let guarded = serde_json::json!({
        "kind": "requirement", "id": "orders.sku-required",
        "statement": "An order without a sku is refused.", "path": "app/orders.py#L9-L10"
    });
    let returned = serde_json::json!({
        "kind": "requirement", "id": "orders.size",
        "statement": "An order of more than one unit is bulk.", "path": "app/orders.py#L23"
    });
    let routed = serde_json::json!({
        "kind": "requirement", "id": "start.route",
        "statement": "The app routes POST /orders to the handler.", "path": "app/main.py#L6"
    });
    let wired = serde_json::json!({
        "kind": "requirement", "id": "orders.pool",
        "statement": "Orders use the pool.", "path": "app/orders.py#L3"
    });
    let retry = serde_json::json!({
        "kind": "criterion", "id": "orders.retry-s",
        "criterion": "RETRY_S is half a second.", "path": "app/orders.py#L3"
    });
    let headed = serde_json::json!({
        "kind": "requirement", "id": "orders.create-order",
        "statement": "Creating an order validates the body, then answers its size.",
        "path": "app/orders.py#L8"
    });
    let strayed = serde_json::json!({
        "claims": [&guarded, &wired, &routed, &retry, &returned, &headed]
    })
    .to_string();
    let corrected =
        serde_json::json!({ "claims": [&guarded, &routed, &retry, &returned, &headed] })
            .to_string();
    let survey = py_deciding_inventory();
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("the wired anchor is refused");
    assert!(
        correction.contains("claim 1: path `app/orders.py#L3`"),
        "the finding names the claim at the import: {correction}"
    );
    for held in ["claim 0:", "claim 2:", "claim 3:", "claim 4:", "claim 5:"] {
        assert!(!correction.contains(held), "{held} is at an anchor or not held: {correction}");
    }
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the inline value is held to no anchor");
}

// A step a function takes into the tree is a `requirement`'s anchor however
// the code reaches the instance it steps on: a call awaited or made for its
// effect alone on a local a tree class constructs, or on a parameter typed
// by one, as on `self` or a module import; the construction the local holds
// only wires, and is refused.
#[tokio::test]
async fn python_steps() {
    let project = scratch();
    modules(
        &project,
        &[
            PY_DECIDING[0],
            PY_DECIDING[1],
            (
                "app/orders.py",
                "from .mailer import Mailer\nfrom .repository import OrdersRepository\n\n\nasync \
                 def create_order(body: dict) -> dict:\n    repo = OrdersRepository()\n    \
                 order_id = await repo.insert(body[\"sku\"])\n    repo.flush()\n    await \
                 notify(Mailer(), order_id)\n    return {\"id\": order_id}\n\n\nasync def \
                 notify(mailer: Mailer, order_id: str) -> None:\n    await \
                 mailer.send(order_id)\n",
            ),
            (
                "app/repository.py",
                "class OrdersRepository:\n    async def insert(self, sku: str) -> str:\n        \
                 return sku\n\n    def flush(self) -> None:\n        pass\n",
            ),
            (
                "app/mailer.py",
                "class Mailer:\n    async def send(self, order_id: str) -> None:\n        pass\n",
            ),
        ],
    );
    let inserted = serde_json::json!({
        "kind": "requirement", "id": "orders.insert",
        "statement": "An order is inserted by its sku.", "path": "app/orders.py#L7"
    });
    let flushed = serde_json::json!({
        "kind": "requirement", "id": "orders.flush",
        "statement": "The repository is flushed after the insert.", "path": "app/orders.py#L8"
    });
    let sent = serde_json::json!({
        "kind": "requirement", "id": "orders.notify",
        "statement": "The order's id is mailed.", "path": "app/orders.py#L14"
    });
    let constructed = serde_json::json!({
        "kind": "requirement", "id": "orders.repository",
        "statement": "Orders use a repository.", "path": "app/orders.py#L6"
    });
    let strayed =
        serde_json::json!({ "claims": [&inserted, &constructed, &flushed, &sent] }).to_string();
    let corrected = serde_json::json!({ "claims": [&inserted, &flushed, &sent] }).to_string();
    let survey = py_deciding_inventory();
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("the construction is refused");
    assert!(
        correction.contains("claim 1: path `app/orders.py#L6`"),
        "the finding names the claim at the construction: {correction}"
    );
    for held in ["claim 0:", "claim 2:", "claim 3:"] {
        assert!(!correction.contains(held), "{held} is at a step into the tree: {correction}");
    }
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the inline value is held to no anchor");
}

// What a class declares is decided at its fields, whatever initialises them
// — a view's `permission_classes` names its guards — so a field is an
// anchor; a decorated `def` is anchored through its `def` line, not at its
// decorator alone; and a call for its effect on a local a member of the
// class made (`serializer = self.InputSerializer(..)`, then
// `serializer.is_valid(..)`) is a step into the tree, while the construction
// that made it only wires.
#[tokio::test]
async fn python_anchors_declared() {
    let project = scratch();
    modules(
        &project,
        &[
            ("manage.py", "import sys\n\nif __name__ == \"__main__\":\n    sys.exit(0)\n"),
            ("app/__init__.py", ""),
            (
                "app/urls.py",
                "from django.urls import path\n\nfrom . import views\n\nurlpatterns = \
                 [\n    path(\"users/\", views.UserCreateApi.as_view()),\n]\n",
            ),
            (
                "app/views.py",
                "from django.db import transaction\nfrom rest_framework import \
                 serializers\nfrom rest_framework.permissions import IsAuthenticated\nfrom \
                 rest_framework.response import Response\nfrom rest_framework.views import \
                 APIView\n\nfrom .services import user_create\n\n\nclass ApiAuthMixin:\n    \
                 permission_classes = (IsAuthenticated,)\n\n\nclass UserCreateApi(ApiAuthMixin, \
                 APIView):\n    class InputSerializer(serializers.Serializer):\n        email = \
                 serializers.CharField()\n\n    def post(self, request):\n        serializer = \
                 self.InputSerializer(data=request.data)\n        \
                 serializer.is_valid(raise_exception=True)\n        user = \
                 user_create(**serializer.validated_data)\n        return Response({\"id\": \
                 user.id})\n\n\n@transaction.atomic\ndef user_rename(\n    user_id: int,\n    \
                 name: str,\n) -> None:\n    pass\n",
            ),
            ("app/services.py", "def user_create(**data):\n    return data\n"),
        ],
    );
    let guarded = serde_json::json!({
        "kind": "requirement", "id": "users.authenticated",
        "statement": "Creating a user requires an authenticated caller.",
        "path": "app/views.py#L11"
    });
    let validated = serde_json::json!({
        "kind": "requirement", "id": "users.create.body",
        "statement": "A body that fails the input serializer is refused.",
        "path": "app/views.py#L20"
    });
    let headed = serde_json::json!({
        "kind": "requirement", "id": "users.rename",
        "statement": "A rename runs in one transaction.", "path": "app/views.py#L26-L29"
    });
    let constructed = serde_json::json!({
        "kind": "requirement", "id": "users.create.serializer",
        "statement": "Creating a user builds the input serializer.", "path": "app/views.py#L19"
    });
    let strayed =
        serde_json::json!({ "claims": [&guarded, &constructed, &validated, &headed] }).to_string();
    let corrected = serde_json::json!({ "claims": [&guarded, &validated, &headed] }).to_string();
    let survey = inventory(&[("POST /users/", "app/urls.py#L6", "users")], &[]);
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("the construction is refused");
    assert!(
        correction.contains("claim 1: path `app/views.py#L19`"),
        "the finding names the claim at the construction: {correction}"
    );
    for held in ["claim 0:", "claim 2:", "claim 3:"] {
        assert!(!correction.contains(held), "{held} is at a declared anchor: {correction}");
    }
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the corrected answer is accepted");
    assert_eq!(exchanges[2].outcome, Ok(String::new()), "the inline value is held to no anchor");
}

// The tree's own tests are read for what they state: each test by its
// docstring or its name read as words, under its class, a parametrized one
// over several values, each scenario under its feature, listed at its line
// in the seam whose modules the test imports — a feature through the step
// modules beside it, a test importing no module of the tree in every seam
// — and the test file itself follows the seam's modules, laid or listed as
// they are; a skipped test states nothing.
#[tokio::test]
async fn python_stated() {
    let project = scratch();
    modules(&project, &PY_APP);
    py_bulk(&project, "app/db.py", "pool = \"o-1\"\n");
    modules(
        &project,
        &[
            (
                "tests/test_orders.py",
                "import pytest\n\nfrom app.services.orders import create_order\n\n\ndef \
                 test_creates_an_order_from_the_body():\n    assert create_order\n\n\n\
                 @pytest.mark.skip\ndef test_rejects_an_empty_body():\n    pass\n\n\nclass \
                 TestMoney:\n    def test_adds(self):\n        \"\"\"Adds two amounts of one \
                 currency.\"\"\"\n        assert True\n\n\n@pytest.mark.parametrize(\"qty\", [1, \
                 2])\ndef test_accepts_quantities(qty):\n    assert qty\n",
            ),
            ("tests/test_health.py", "def test_answers_ok():\n    assert True\n"),
            (
                "features/orders.feature",
                "Feature: Orders\n  Scenario: An order is created\n    Given a body\n",
            ),
            (
                "features/steps/orders_steps.py",
                "from app.db import pool\n\n\ndef given_a_body():\n    return pool\n",
            ),
        ],
    );
    let survey = py_app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 3);
    let orders = turn_for(&turns, "POST /api/orders");
    for stated in [
        "- `tests/test_orders.py#L6` — creates an order from the body",
        "- `tests/test_orders.py#L16` — Money › Adds two amounts of one currency.",
        "- `tests/test_orders.py#L21` — accepts quantities (over several values)",
        "- `features/orders.feature#L2` — Orders › An order is created",
        "- `tests/test_health.py#L1` — answers ok",
    ] {
        assert!(orders.contains(stated), "{stated} is stated: {orders}");
    }
    assert!(!orders.contains("rejects an empty body"), "a skipped test states nothing: {orders}");
    assert!(
        orders.contains("- `tests/test_orders.py`"),
        "listed after the store past the budget: {orders}"
    );
    assert!(!orders.contains("orders_steps.py"), "a step module states nothing: {orders}");
    let start = turn_for(&turns, "start");
    assert!(start.contains("- `tests/test_health.py#L1` — answers ok"), "{start}");
    assert!(
        !start.contains("tests/test_orders.py"),
        "the orders test imports no module of `start`: {start}"
    );
    assert!(!start.contains("orders.feature"), "{start}");
    laid(start, &["app/main.py", "app/routers/orders.py", "tests/test_health.py"], &[]);
}

// An import under `if TYPE_CHECKING:` is a declaration's: the module it
// names is reached by no seam, counts as nothing the resolver could not
// follow, and the guard itself is no decision.
#[tokio::test]
async fn python_type_checking() {
    let project = scratch();
    modules(&project, &PY_APP);
    py_bulk(&project, "app/db.py", "pool = \"o-1\"\n");
    modules(
        &project,
        &[
            (
                "app/services/orders.py",
                "from __future__ import annotations\n\nfrom typing import TYPE_CHECKING\n\nfrom \
                 ..db import pool\n\nif TYPE_CHECKING:\n    from ..audit import \
                 Auditor\n\n\nasync def create_order(body: dict, auditor: Auditor | None = None) \
                 -> dict:\n    return {\"id\": pool, \"body\": body}\n\n\ndef find_order(order_id: \
                 str) -> dict:\n    return {\"id\": order_id, \"pool\": pool}\n",
            ),
            ("app/audit.py", "class Auditor:\n    pass\n"),
        ],
    );
    let survey = py_app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 3);
    let orders = turn_for(&turns, "POST /api/orders");
    laid(orders, &["app/routers/orders.py", "app/services/orders.py"], &["app/audit.py"]);
    assert!(orders.contains("- `app/db.py`"), "the store is reached: {orders}");
    assert!(!orders.contains("could not follow"), "a typing import is followed nowhere: {orders}");
    assert!(!orders.contains("TYPE_CHECKING`"), "the guard decides nothing: {orders}");
}

// A `from .. import *` under `if TYPE_CHECKING:` in a package `__init__` is
// a declaration's too: the star re-export it records is type-only, so the
// module it names is reached by no seam, and counts as nothing the resolver
// could not follow.
#[tokio::test]
async fn python_type_checking_star() {
    let project = scratch();
    modules(
        &project,
        &[
            ("app/__init__.py", ""),
            (
                "app/main.py",
                "from fastapi import FastAPI\n\nfrom .routers import orders\n\napp = \
                 FastAPI()\napp.include_router(orders.router, prefix=\"/api\")\n",
            ),
            ("app/routers/__init__.py", ""),
            (
                "app/routers/orders.py",
                "from fastapi import APIRouter\n\nfrom ..store import pool\n\nrouter = \
                 APIRouter(prefix=\"/orders\")\n\n\n@router.get(\"/{order_id}\")\nasync def \
                 get(order_id: str) -> dict:\n    return {\"id\": order_id, \"pool\": pool}\n",
            ),
            ("app/audit.py", "class Auditor:\n    pass\n"),
        ],
    );
    py_bulk(
        &project,
        "app/store/__init__.py",
        "from typing import TYPE_CHECKING\n\nif TYPE_CHECKING:\n    from ..audit import *\n\npool \
         = \"o-1\"\n",
    );
    let survey =
        inventory(&[("GET /api/orders/{order_id}", "app/routers/orders.py#L8-L10", "orders")], &[]);
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 2);
    let orders = turn_for(&turns, "GET /api/orders/{order_id}");
    laid(orders, &["app/routers/orders.py"], &["app/audit.py"]);
    assert!(orders.contains("- `app/store/__init__.py`"), "the store is reached: {orders}");
    assert!(!orders.contains("could not follow"), "a typing import is followed nowhere: {orders}");
    let start = turn_for(&turns, "start");
    assert!(!start.contains("app/audit.py"), "nothing of `start` reaches it either: {start}");
}

// A relative import whose module shares a typing module's name — `from
// .types import Status`, as `.enum`, `.abc`, or `.dataclasses` would — is the
// tree's own module, not the standard library's: the seam reaches it past
// the budget, and its declaration is a parser `type` claim.
#[tokio::test]
async fn python_relative_typing_name() {
    let project = scratch();
    modules(&project, &PY_APP);
    py_bulk(&project, "app/db.py", "pool = \"o-1\"\n");
    modules(
        &project,
        &[
            (
                "app/services/orders.py",
                "from ..db import pool\nfrom .types import Status\n\n\nasync def \
                 create_order(body: dict, status: Status = \"new\") -> dict:\n    return {\"id\": \
                 pool, \"body\": body, \"status\": status}\n\n\ndef find_order(order_id: str) -> \
                 dict:\n    return {\"id\": order_id, \"pool\": pool}\n",
            ),
            (
                "app/services/types.py",
                "from typing import Literal\n\nStatus = Literal[\"new\", \"paid\"]\n",
            ),
        ],
    );
    let survey = py_app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &["types", "class Inline:\n    id: str\n", "Inline", "Status"],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 3);
    let orders = turn_for(&turns, "POST /api/orders");
    assert!(
        orders.contains(
            "; id `orders.post`; reaches `app/services/orders.py`, `app/db.py`, \
             `app/services/types.py`."
        ),
        "{orders}"
    );
    laid(orders, &["app/routers/orders.py", "app/services/orders.py"], &[]);
    assert!(orders.contains("- `app/services/types.py`"), "the module is listed: {orders}");
    assert!(!orders.contains("could not follow"), "the import is followed: {orders}");
}

// The service imports a module the tree does not hold and loads another by
// a computed name, so its seam cannot know what it reaches: the rest of the
// tree follows the closure — the entry, listed after the store past the
// budget — and the brief says what could not be followed and why the list
// runs on. The bootstrap's seam, whose modules import nothing unresolved,
// is not widened. Within the budget every module is laid already — the
// package `__init__.py` files no closure holds among them — so the brief says
// only what could not be followed.
#[tokio::test]
async fn python_unresolved() {
    const ORDERS: &str = "import importlib\n\nfrom ..db import pool\nfrom .generated import \
                          seed\n\n\nasync def create_order(body: dict) -> dict:\n    return \
                          {\"id\": pool, \"body\": body, \"seed\": seed}\n\n\ndef \
                          load_plugin(name: str):\n    return \
                          importlib.import_module(name)\n\n\ndef find_order(order_id: str) -> \
                          dict:\n    return {\"id\": order_id, \"pool\": pool}\n";
    const UNFOLLOWED: &str = "The caller could not follow every import: `.generated` from \
                              `app/services/orders.py` names no module of the tree; \
                              `app/services/orders.py` loads a module by a computed name at L12. \
                              What these name is in none of the lists above.";
    let project = scratch();
    modules(&project, &PY_APP);
    project.write("app/services/orders.py", ORDERS);
    py_bulk(&project, "app/db.py", "pool = \"o-1\"\n");
    let survey = py_app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 3);
    let orders = turn_for(&turns, "POST /api/orders");
    laid(orders, &["app/routers/orders.py", "app/services/orders.py"], &[]);
    let db = orders.find("- `app/db.py`\n").expect("the store past the budget is listed");
    let main = orders.find("- `app/main.py`\n").expect("the entry follows the closure");
    assert!(db < main, "the rest of the tree is listed after the closure: {orders}");
    assert!(
        orders.contains(&format!(
            "{UNFOLLOWED} The modules after the closure are the rest of the tree, laid so what \
             these name is still within reach; read them for that alone."
        )),
        "the brief says what could not be followed and why the list runs on: {orders}"
    );
    let start = turn_for(&turns, "start");
    laid(start, &["app/main.py", "app/routers/orders.py"], &["app/services/orders.py"]);
    assert!(!start.contains("could not follow"), "nothing of `start` is unresolved: {start}");

    // within the budget
    let project = scratch();
    modules(&project, &PY_APP);
    project.write("app/services/orders.py", ORDERS);

    let model = mined(PYTHON, &project, &py_app_inventory(), 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    laid(&turns[0], &["app/main.py", "app/routers/orders.py", "app/services/orders.py"], &[]);
    assert!(turns[0].contains(UNFOLLOWED), "{}", turns[0]);
    assert!(!turns[0].contains("rest of the tree"), "nothing was widened: {}", turns[0]);
}

// The survey over the committed `click-jobs` fixture — the tree the eval of
// that name runs over, answered as the eval expects it: within the budget,
// so one seam lays every module; the bootstrap the manifest's console script
// names, four commands under the group, a schedule registered inside a
// function, and two tasks under a package's decorator. The code reads each
// anchor for what declares there — the command's literal, the task's dotted
// name by its first segment — and the seed script no surface reaches is
// laid with the rest.
#[tokio::test]
async fn python_fixture_click_jobs() {
    let project = scratch();
    fixture(&project, "click-jobs");
    let survey = inventory(
        &[
            ("import command", "src/ledger/cli.py#L22-L30", "import"),
            ("reconcile command", "src/ledger/cli.py#L33-L53", "reconcile"),
            ("nightly command", "src/ledger/cli.py#L56-L59", "nightly"),
            ("serve command", "src/ledger/cli.py#L62-L80", "serve"),
            ("nightly schedule", "src/ledger/jobs/nightly.py#L35", "nightly"),
            ("invoices.remind task", "src/ledger/workers/invoices.py#L12-L34", "invoices"),
            ("invoices.void task", "src/ledger/workers/invoices.py#L37-L40", "invoices"),
        ],
        &["scripts/seed.py"],
    );
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(
        surfaces(&turns[0]),
        [
            ("start", "src/ledger/cli.py", "start"),
            ("import command", "src/ledger/cli.py", "import"),
            ("reconcile command", "src/ledger/cli.py", "reconcile"),
            ("nightly command", "src/ledger/cli.py", "nightly"),
            ("serve command", "src/ledger/cli.py", "serve"),
            ("nightly schedule", "src/ledger/jobs/nightly.py", "nightly"),
            ("invoices.remind task", "src/ledger/workers/invoices.py", "invoices"),
            ("invoices.void task", "src/ledger/workers/invoices.py", "invoices"),
        ]
    );
    for note in [
        "the process bootstrap, run by the console script `ledger`, which calls `cli()`: ",
        "; id `start`; reaches `src/ledger/lib/db.py`, `src/ledger/config.py`, \
         `src/ledger/jobs/nightly.py`, `src/ledger/queues.py`, `src/ledger/services/importer.py`, \
         `src/ledger/services/reconcile.py`, `src/ledger/workers/invoices.py`, \
         `src/ledger/lib/csv.py`.",
        "def `import_command` L22–L30; under `@cli.command(\"import\")`; through `click`; id \
         `import`; reaches `src/ledger/services/importer.py`, `src/ledger/config.py`, \
         `src/ledger/jobs/nightly.py`, `src/ledger/lib/db.py`, `src/ledger/queues.py`, \
         `src/ledger/services/reconcile.py`, `src/ledger/lib/csv.py`, \
         `src/ledger/workers/invoices.py`.",
        "def `serve` L62–L80; under `@cli.command(\"serve\")`; through `click`; id `serve`;",
        "def `nightly_command` L56–L59; under `@cli.command(\"nightly\")`; through `click`; id \
         `nightly.nightly-command`;",
        "registered L35 in `schedule`; through `apscheduler`; id `nightly.add-job`; reaches \
         `src/ledger/config.py`, `src/ledger/lib/db.py`, `src/ledger/services/reconcile.py`, \
         `src/ledger/workers/invoices.py`, `src/ledger/queues.py`.",
        "def `send_reminder` L12–L34; under `@app.task(\"invoices.remind\")`; through `celery`; \
         id `invoices.remind`; reaches `src/ledger/queues.py`, `src/ledger/config.py`, \
         `src/ledger/lib/db.py`.",
        "def `void_invoice` L37–L40; under `@app.task(\"invoices.void\")`; through `celery`; id \
         `invoices.void`;",
    ] {
        assert!(turns[0].contains(note), "{note} is in the brief: {}", turns[0]);
    }
    laid(
        &turns[0],
        &[
            "src/ledger/cli.py",
            "src/ledger/config.py",
            "src/ledger/jobs/nightly.py",
            "src/ledger/lib/db.py",
            "src/ledger/queues.py",
            "src/ledger/services/importer.py",
            "src/ledger/services/reconcile.py",
            "src/ledger/workers/invoices.py",
            "src/ledger/lib/csv.py",
            "scripts/seed.py",
        ],
        &["pyproject.toml"],
    );
    for stated in [
        "- `tests/test_csv.py#L8` — parses a well formed row",
        "- `tests/test_csv.py#L26` — rejects a zero or non numeric amount (over several values)",
        "- `tests/test_reconcile.py#L14` — SameMovement › An entry matches a movement on account \
         and date when the amounts are within the tolerance.",
    ] {
        assert!(turns[0].contains(stated), "{stated} is stated: {}", turns[0]);
    }
}

// The `fastapi-routers` bootstrap's closure in the order the survey lays it:
// the entry, then what it imports breadth-first — the three routers it
// includes reached and stopped at, the shipping client one of them constructs
// at load followed into the postcode table, which is where the SDK's budget
// runs out, so the table is listed rather than laid.
const FASTAPI_START: [&str; 9] = [
    "app/main.py",
    "app/settings.py",
    "app/cache.py",
    "app/routers/orders.py",
    "app/lib/errors.py",
    "app/routers/health.py",
    "app/routers/customers.py",
    "app/services/shipping.py",
    "app/data/postcodes.py",
];
const FASTAPI_START_LAID: usize = 8;

// The `orders` router's closure, from its entry; the rates table is laid
// after `pricing.py`, which names it, the table ends the laid run, and the
// two models after it are listed with the tests.
const FASTAPI_ORDERS: [&str; 19] = [
    "app/routers/orders.py",
    "app/schemas/orders.py",
    "app/settings.py",
    "app/services/orders.py",
    "app/deps.py",
    "app/repositories/catalogue.py",
    "app/repositories/customers.py",
    "app/repositories/orders.py",
    "app/services/customers.py",
    "app/services/shipping.py",
    "app/cache.py",
    "app/lib/errors.py",
    "app/models/__init__.py",
    "app/services/pricing.py",
    "app/db.py",
    "app/schemas/customers.py",
    "app/data/postcodes.py",
    "app/models/customer.py",
    "app/models/order.py",
];
const FASTAPI_ORDERS_LAID: usize = 16;

// The `customers` router's closure, from its entry.
const FASTAPI_CUSTOMERS: [&str; 12] = [
    "app/routers/customers.py",
    "app/schemas/customers.py",
    "app/services/customers.py",
    "app/deps.py",
    "app/repositories/customers.py",
    "app/cache.py",
    "app/lib/errors.py",
    "app/models/__init__.py",
    "app/settings.py",
    "app/db.py",
    "app/models/customer.py",
    "app/models/order.py",
];

// The `health` router's closure, from its entry.
const FASTAPI_HEALTH: [&str; 4] =
    ["app/routers/health.py", "app/cache.py", "app/db.py", "app/settings.py"];

// The surfaces the survey names in `fastapi-routers`, each at its decorated
// `def`, under the stem its router's prefix spells.
const FASTAPI_SURFACES: [(&str, &str, &str); 10] = [
    ("GET /api/v1/orders", "app/routers/orders.py#L40-L49", "orders"),
    ("POST /api/v1/orders", "app/routers/orders.py#L52-L56", "orders"),
    ("GET /api/v1/orders/{order_id}", "app/routers/orders.py#L59-L61", "orders"),
    ("PUT /api/v1/orders/{order_id}/lines", "app/routers/orders.py#L64-L66", "orders"),
    ("POST /api/v1/orders/{order_id}/transitions", "app/routers/orders.py#L69-L71", "orders"),
    ("POST /api/v1/customers", "app/routers/customers.py#L26-L30", "customers"),
    ("GET /api/v1/customers/{customer_id}", "app/routers/customers.py#L33-L35", "customers"),
    (
        "POST /api/v1/customers/{customer_id}/tier/{tier}",
        "app/routers/customers.py#L38-L40",
        "customers",
    ),
    ("GET /health", "app/routers/health.py#L10-L15", "health"),
    ("GET /health/ready", "app/routers/health.py#L18-L20", "health"),
];

// The surface lines a seam under `stem` is told, from the survey's answer.
fn fastapi_named(stem: &str) -> Vec<(&str, &str, &str)> {
    FASTAPI_SURFACES
        .iter()
        .filter(|(_, _, own)| *own == stem)
        .map(|(name, anchor, own)| {
            (*name, anchor.split_once('#').map_or(*anchor, |(p, _)| p), *own)
        })
        .collect()
}

// `; id <id>; reaches <closure past the entry>.` as a surface line ends.
fn reaches(id: &str, closure: &[&str]) -> String {
    let reached: Vec<String> = closure[1..].iter().map(|path| format!("`{path}`")).collect();
    format!("; id `{id}`; reaches {}.", reached.join(", "))
}

// The leading `laid` files of a closure are laid whole and the rest listed.
fn laid_then_listed(turn: &str, closure: &[&str], laid_count: usize, refused: &[&str]) {
    laid(turn, &closure[..laid_count], refused);
    for listed in &closure[laid_count..] {
        assert!(turn.contains(&format!("- `{listed}`\n")), "`{listed}` is listed: {turn}");
        assert!(!turn.contains(&format!("### `{listed}`")), "`{listed}` is not laid: {turn}");
    }
}

// A router's seam in `fastapi-routers`: its surfaces alone, the first's line
// whole — its `def`, its decorator, its package, and its closure — the ids
// the others are told apart by, and its closure laid from its entry for as
// long as the budget holds, with nothing of the rest of the tree.
fn fastapi_router(
    turn: &str, stem: &str, first: &str, ids: &[&str], closure: &[&str], laid_count: usize,
    refused: &[&str],
) {
    assert_eq!(surfaces(turn), fastapi_named(stem));
    let line = format!("{first}{}", reaches(ids[0], closure));
    assert!(turn.contains(&line), "{line} is in the brief: {turn}");
    for id in &ids[1..] {
        assert!(turn.contains(&format!("id `{id}`;")), "`{id}` is in the brief: {turn}");
    }
    laid_then_listed(turn, closure, laid_count, refused);
}

// The second committed fixture pin: a FastAPI service past the inline budget,
// cut one seam per stem — the bootstrap's closure stopping at every router's
// module and reaching what each constructs at load, each router's seam laying
// the union of its surfaces' closures with the entry first, the `.json` the
// pricing module reads by a computed path named after them, and a table too
// large to lay ending the laid run where it falls. Every route's stem is the
// code's, read from its router's prefix under the include's, and each is told
// apart by its verb and the path past the resource.
#[tokio::test]
async fn python_fixture_fastapi_routers() {
    let project = scratch();
    fixture(&project, "fastapi-routers");
    let survey = inventory(&FASTAPI_SURFACES, &[]);
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 4);
    let start = turn_for(&turns, "start");
    assert_eq!(surfaces(start), [("start", "app/main.py", "start")]);
    assert!(
        start.contains(
            "the process bootstrap, run by the console script `shopapi`, which calls `serve()`: "
        ),
        "the bootstrap is the console script: {start}"
    );
    assert!(
        start.contains(&reaches("start", &FASTAPI_START)),
        "the bootstrap stops at each router and reaches what it constructs: {start}"
    );
    laid_then_listed(
        start,
        &FASTAPI_START,
        FASTAPI_START_LAID,
        &["app/services/orders.py", "app/repositories/", "app/schemas/", "pyproject.toml"],
    );

    let orders = turn_for(&turns, "GET /api/v1/orders");
    fastapi_router(
        orders,
        "orders",
        "def `list_orders` L40–L49; under `@router.get(\"\")`; through `fastapi`",
        &[
            "orders.get",
            "orders.post",
            "orders.get-order-id",
            "orders.put-order-id-lines",
            "orders.post-order-id-transitions",
        ],
        &FASTAPI_ORDERS,
        FASTAPI_ORDERS_LAID,
        &["app/main.py", "app/routers/health.py", "app/routers/customers.py", "pyproject.toml"],
    );
    for note in [
        "def `get_order` L59–L61; under `@router.get(\"/{order_id}\")`; through `fastapi`; id \
         `orders.get-order-id`;",
        "- `app/data/rates.json` — named by `app/services/pricing.py`",
        "- `tests/test_orders.py#L48` — a cancellation needs a reason",
        "- `tests/test_pricing.py#L30` — totals sum the parts (over several values)",
    ] {
        assert!(orders.contains(note), "{note} is in the brief: {orders}");
    }
    let pricing = orders.find("### `app/services/pricing.py`").expect("the reader is laid");
    let rates = orders.find("### `app/data/rates.json` (10 lines)").expect("the table is laid");
    let after = orders.find("### `app/db.py`").expect("the module after the reader is laid");
    assert!(pricing < rates && rates < after, "the table is laid after its reader: {orders}");
    assert!(!orders.contains("- `app/data/rates.json`\n"), "the data file is laid: {orders}");

    fastapi_router(
        turn_for(&turns, "POST /api/v1/customers"),
        "customers",
        "def `register` L26–L30; under `@router.post(\"\")`; through `fastapi`",
        &["customers.post", "customers.get-customer-id", "customers.post-customer-id-tier-tier"],
        &FASTAPI_CUSTOMERS,
        FASTAPI_CUSTOMERS.len(),
        &["app/main.py", "app/services/orders.py", "app/routers/orders.py", "app/data/"],
    );

    let health = turn_for(&turns, "GET /health");
    fastapi_router(
        health,
        "health",
        "def `live` L10–L15; under `@router.get(\"\")`; through `fastapi`",
        &["health.get", "health.get-ready"],
        &FASTAPI_HEALTH,
        FASTAPI_HEALTH.len(),
        &["app/main.py", "app/routers/orders.py", "app/services/", "tests/test_orders.py"],
    );
    // a test with no import into the tree is stated to every seam; one whose
    // imports resolve into a seam's files is stated there alone
    laid(health, &["tests/test_api.py"], &[]);
}

// A module loaded by a literal dotted name is followed into the closure as
// an import is; one loaded by a computed name, or found by walking a
// package, cannot be, so the seam is widened to the rest of the tree and the
// brief says where the load is spelled — every import of it followed.
#[tokio::test]
async fn python_dynamic() {
    let project = scratch();
    modules(&project, &PY_APP);
    modules(
        &project,
        &[
            (
                "app/services/orders.py",
                "import importlib\nimport pkgutil\n\nfrom ..db import pool\nfrom .. import \
                 plugins\n\nrural = importlib.import_module(\"app.plugins.rural\")\n\n\ndef \
                 load_all() -> list:\n    found = pkgutil.iter_modules(plugins.__path__)\n    \
                 return [importlib.import_module(f\"app.plugins.{m.name}\") for m in \
                 found]\n\n\nasync def create_order(body: dict) -> dict:\n    return {\"id\": \
                 pool, \"body\": body, \"zone\": rural.ZONE}\n\n\ndef find_order(order_id: str) \
                 -> dict:\n    return {\"id\": order_id, \"pool\": pool}\n",
            ),
            ("app/plugins/__init__.py", ""),
            ("app/plugins/rural.py", "ZONE = 650\n"),
            ("app/plugins/urban.py", "ZONE = 0\n"),
        ],
    );
    py_bulk(&project, "app/db.py", "pool = \"o-1\"\n");
    let survey = py_app_inventory();
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 3);
    let orders = turn_for(&turns, "POST /api/orders");
    assert!(
        orders.contains(
            "; id `orders.post`; reaches `app/services/orders.py`, `app/db.py`, \
             `app/plugins/__init__.py`, `app/plugins/rural.py`."
        ),
        "the literal load is followed: {orders}"
    );
    let rural = orders.find("- `app/plugins/rural.py`\n").expect("the closure is listed");
    let urban = orders.find("- `app/plugins/urban.py`\n").expect("the rest of the tree follows");
    assert!(rural < urban, "the rest of the tree is listed after the closure: {orders}");
    assert!(
        orders.contains(
            "The caller could not follow every import: `app/services/orders.py` loads a module by \
             a computed name at L11, L12. What these name is in none of the lists above. The \
             modules after the closure are the rest of the tree, laid so what these name is \
             still within reach; read them for that alone."
        ),
        "the brief says where the loads are spelled: {orders}"
    );
    assert!(!orders.contains("names no module"), "every import was followed: {orders}");
    let start = turn_for(&turns, "start");
    assert!(!start.contains("could not follow"), "nothing of `start` is dynamic: {start}");
}

// A `.json` a module names by path is a data file of the seam: named in the
// brief with the module reading it, laid directly after that module when
// it is small and after every module when it is large, and a file a
// `criterion` may cite a value in — while a `requirement` there is at no
// anchor and comes back.
#[tokio::test]
async fn python_data() {
    let project = scratch();
    modules(&project, &PY_APP);
    project.write(
        "app/services/orders.py",
        "import json\nfrom pathlib import Path\n\nfrom ..db import pool\n\nZONES = \
         json.loads(Path(\"app/data/zones.json\").read_text())\nTARIFFS = \
         json.loads(Path(\"app/data/tariffs.json\").read_text())\n\n\nasync def \
         create_order(body: dict) -> dict:\n    return {\"id\": pool, \"body\": body, \"zone\": \
         ZONES[\"rural\"], \"tariff\": TARIFFS[\"0\"]}\n\n\ndef find_order(order_id: str) -> \
         dict:\n    return {\"id\": order_id, \"pool\": pool}\n",
    );
    project.write("app/data/zones.json", "{\n  \"rural\": 650,\n  \"urban\": 0\n}\n");
    // a generated table past the 8 KB a data file is laid beside its module within
    let tariffs: Vec<String> = (0..800).map(|i| format!("  \"{i}\": {}", i * 7)).collect();
    project.write("app/data/tariffs.json", format!("{{\n{}\n}}\n", tariffs.join(",\n")));
    let cited = serde_json::json!({
        "kind": "criterion", "id": "orders.rural-surcharge",
        "criterion": "The rural surcharge is 650 cents.", "path": "app/data/zones.json#L2"
    });
    let anchored = serde_json::json!({
        "kind": "requirement", "id": "orders.rural",
        "statement": "A rural order carries the surcharge.", "path": "app/data/zones.json#L2"
    });
    let strayed = serde_json::json!({ "claims": [&cited, &anchored] }).to_string();
    let corrected = serde_json::json!({ "claims": [&cited] }).to_string();
    let survey = py_app_inventory();
    let inline = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &strayed, &corrected, &inline]),
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 4, "the survey, two rounds for the one seam, one for the inline value");
    let turn = &seen[1].messages[0];
    assert!(
        turn.contains(
            "Data files these modules name by path, each with the modules reading it, laid after \
             the first module naming it — a large one after every module:\n\n- \
             `app/data/zones.json` — named by `app/services/orders.py`\n- `app/data/tariffs.json` \
             — named by `app/services/orders.py`"
        ),
        "the data files are named with their reader: {turn}"
    );
    laid(
        turn,
        &["app/main.py", "app/services/orders.py", "app/data/zones.json", "app/data/tariffs.json"],
        &[],
    );
    let at = |heading: &str| turn.find(&format!("### `{heading}`")).expect(heading);
    let reader = at("app/services/orders.py");
    let last_module = at("app/routers/__init__.py");
    let small = at("app/data/zones.json");
    let large = at("app/data/tariffs.json");
    assert!(
        reader < small && small < last_module,
        "the small data file is laid directly after the module naming it: {turn}"
    );
    assert!(last_module < large, "the large data file is laid after every module: {turn}");
    assert!(turn.contains("2|  \"rural\": 650,"), "numbered: {turn}");
    let exchanges = checked(&model);
    let correction = exchanges[0].outcome.as_ref().expect_err("a requirement in data is refused");
    assert!(
        correction.contains("claim 1: path `app/data/zones.json#L2`")
            && !correction.contains("claim 0:"),
        "the criterion is held to no anchor, the requirement to the modules': {correction}"
    );
    assert_eq!(exchanges[1].outcome, Ok(String::new()), "the criterion in data is accepted");
}

// A string spelling a data file's name — `config.toml` — names the file and
// not the `config` module beside it: the file is the seam's data, named by
// the module reading it, and that module reaches nothing through the string.
#[tokio::test]
async fn python_data_beside_module() {
    let project = scratch();
    modules(&project, &PY_APP);
    project.write(
        "app/services/orders.py",
        "import tomllib\nfrom pathlib import Path\n\nfrom ..db import pool\n\nLIMITS = \
         tomllib.loads(Path(\"config.toml\").read_text())\n\n\nasync def create_order(body: \
         dict) -> dict:\n    return {\"id\": pool, \"body\": body, \"limit\": \
         LIMITS[\"lines\"]}\n\n\ndef find_order(order_id: str) -> dict:\n    return {\"id\": \
         order_id, \"pool\": pool}\n",
    );
    project.write("config.py", "LINES = 10\n");
    project.write("config.toml", "lines = 10\n");

    let model = mined(PYTHON, &project, &py_app_inventory(), 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    assert!(
        turns[0].contains("- `config.toml` — named by `app/services/orders.py`"),
        "the data file is named with its reader: {}",
        turns[0]
    );
    assert!(
        turns[0].contains(
            ": def `create` L8–L10; under `@router.post(\"\")`; through `fastapi`; id \
             `orders.post`; reaches `app/services/orders.py`, `app/db.py`."
        ),
        "the module of the file's stem is reached through no string: {}",
        turns[0]
    );
    laid(&turns[0], &["app/services/orders.py", "config.py", "config.toml"], &[]);
}

// Tests, stubs, fixture and build configuration, migrations, caches,
// environments, build output, documentation, and dot entries are not
// modules: what they register is no surface, and — a test stating nothing
// included — they are neither laid nor listed; `manage.py` is a module,
// and the bootstrap. The packaging `setup.py` is the root's alone: one
// beneath a package is a module like any other.
#[tokio::test]
async fn python_non_production() {
    const REFUSED: [&str; 11] = [
        "tests/test_orders.py",
        "app/orders_test.py",
        "app/types.pyi",
        "conftest.py",
        "app/migrations/0001_initial.py",
        "app/__pycache__/orders.cpython-312.py",
        "venv/lib/site-packages/flask/__init__.py",
        "build/lib/app.py",
        "docs/conf.py",
        ".git/HEAD",
        "README.md",
    ];
    let project = scratch();
    modules(&project, &PY_APP);
    for file in REFUSED.iter().chain(&["setup.py"]) {
        project.write(
            file,
            "from flask import Flask\n\napp = Flask(__name__)\n\n\n@app.get(\"/refused\")\ndef \
             refused() -> dict:\n    return {}\n",
        );
    }
    project.write(
        "manage.py",
        "from app.main import app\n\nif __name__ == \"__main__\":\n    app.run()\n",
    );
    project.write("app/setup.py", "ENV = \"production\"\n");

    let model = mined(PYTHON, &project, &py_app_inventory(), 1).await;

    let turns = surveyed(PYTHON, &model, 1);
    let surfaces = surfaces(&turns[0]);
    assert_eq!(surfaces.len(), 4, "the app's four surfaces alone: {}", turns[0]);
    assert_eq!(surfaces[0], ("start", "manage.py", "start"), "{}", turns[0]);
    assert!(!turns[0].contains("/refused"), "nothing refused registers: {}", turns[0]);
    laid(&turns[0], &["manage.py", "app/main.py", "app/setup.py"], &REFUSED);
    assert!(!turns[0].contains("### `setup.py` ("), "the root's `setup.py` is not a module");
}

// A tree of one production module cuts no finer than itself: one extract
// over the module, listed when it is past the budget, the export the survey
// names its surface.
#[tokio::test]
async fn python_one_module() {
    const REFUSED: [&str; 6] = [
        "tests/test_orders.py",
        "src/orders.pyi",
        "build/lib/orders.py",
        "venv/lib/site-packages/flask/__init__.py",
        ".git/HEAD",
        "README.md",
    ];
    let project = scratch();
    tree(&project, &REFUSED);
    py_bulk(
        &project,
        "src/orders.py",
        "class OrderService:\n    def create(self, body: dict) -> dict:\n        return {\"body\": \
         body}\n",
    );
    let survey = inventory(&[("OrderService", "src/orders.py#L1-L3", "order-service")], &[]);
    let answer = claim("order-service.create");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 1);
    assert_eq!(surfaces(&turns[0]), [("OrderService", "src/orders.py", "order-service")]);
    assert!(
        turns[0].contains("ids `order-service`, `order-service.create`;"),
        "the class carries its method: {}",
        turns[0]
    );
    assert!(turns[0].contains("- `src/orders.py`"), "the one module is listed: {}", turns[0]);
    for file in REFUSED {
        assert!(!turns[0].contains(file), "`{file}` is no module: {}", turns[0]);
    }
}

// A tree the survey names no surface in — no bootstrap, no registration, no
// decorator, and no entry export it counts — is cut mechanically, never
// refused: within the budget, one extract over every module, told no
// surface was found and held to the one stem the manifest's package name
// gives.
#[tokio::test]
async fn python_no_surface() {
    let project = scratch();
    modules(
        &project,
        &[
            ("pyproject.toml", "[project]\nname = \"acme-shared-helpers\"\n"),
            ("src/helpers/__init__.py", ""),
            ("src/helpers/db.py", "pool = 1\n"),
            (
                "src/helpers/config.py",
                "from typing import Literal\n\nretries = 3\nMode = Literal[\"fast\"]\n",
            ),
        ],
    );
    let survey =
        inventory(&[], &["src/helpers/__init__.py", "src/helpers/config.py", "src/helpers/db.py"]);
    let answer = claim("acme-shared-helpers.pool");

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &answer, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 1);
    assert!(surfaces(&turns[0]).is_empty(), "no surface is named: {}", turns[0]);
    assert!(turns[0].contains("No surface was found"), "{}", turns[0]);
    assert!(turns[0].contains("the one stem `acme-shared-helpers`"), "{}", turns[0]);
    laid(&turns[0], &["src/helpers/config.py", "src/helpers/db.py"], &[]);
}

// Past the budget, a tree with no surface is one extract per directory
// beneath its one top-level package under `src/`, under the directory's
// name, the package's own modules joined to the first; a directory's seam
// lays its own modules and no other's.
#[tokio::test]
async fn python_no_surface_directories() {
    let project = scratch();
    modules(
        &project,
        &[
            ("src/helpers/__init__.py", ""),
            ("src/helpers/version.py", "VERSION = \"1\"\n"),
            ("src/helpers/lib/config.py", "RETRIES = 3\n"),
            ("src/helpers/util/format.py", "SEPARATOR = \", \"\n"),
        ],
    );
    py_bulk(&project, "src/helpers/lib/db.py", "pool = 1\n");
    let survey = inventory(
        &[],
        &[
            "src/helpers/__init__.py",
            "src/helpers/version.py",
            "src/helpers/lib/config.py",
            "src/helpers/lib/db.py",
            "src/helpers/util/format.py",
        ],
    );
    let neutral = decision();
    let answer = answer();

    let model = support::run(
        test_programs::ADAPTER_PYTHON,
        &project,
        &[],
        ScriptedModel::answering([&survey, &neutral, &neutral, &answer]),
    )
    .await;

    let turns = surveyed(PYTHON, &model, 2);
    let seam = |stem: &str| {
        let naming: Vec<&String> =
            turns.iter().filter(|turn| turn.contains(&format!("the stem `{stem}`"))).collect();
        assert_eq!(naming.len(), 1, "`{stem}` is one seam's alone, got {naming:?}");
        naming[0]
    };
    let lib = seam("lib");
    assert!(lib.contains("`src/helpers/lib/`, with the root's own modules"), "{lib}");
    laid(
        lib,
        &["src/helpers/version.py", "src/helpers/lib/config.py"],
        &["src/helpers/util/format.py"],
    );
    assert!(
        lib.contains("- `src/helpers/lib/db.py`"),
        "the store past the budget is listed: {lib}"
    );
    let util = seam("util");
    laid(
        util,
        &["src/helpers/util/format.py"],
        &["src/helpers/version.py", "src/helpers/lib/config.py", "src/helpers/lib/db.py"],
    );
}

// A tree with no Python module is refused before any turn is spent.
#[tokio::test]
async fn python_no_module() {
    let project = scratch();
    tree(&project, &["README.md", "src/orders.pyi", "tests/test_orders.py"]);

    let model =
        refused(test_programs::ADAPTER_PYTHON, &project, None, ScriptedModel::default()).await;
    assert!(model.seen().is_empty(), "no turn is spent on a tree with no module");
}
