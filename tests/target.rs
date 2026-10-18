//! Tests each shipped target adapter natively against a scripted model.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::os::unix::fs::symlink;
use std::path::Path;

use emery_sdk::target::{
    BuildContext, InitContext, MergeStrategy, Outcome, Report, Slice, TargetAdapter, Task, Verdict,
    VerifyContext,
};
use emery_sdk::{Doc, Error, body};
use omnia_sdk::model::ToolCall;
use omnia_test::SeenWorker;
use omnia_test::guest::Scripted;
use omnia_test::host::{Scratch, scratch};

const PASSED: &str = r#"{"passed": true, "failures": []}"#;

const TYPESCRIPT_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["src/orders/reading/package.json", "src/orders/reading/index.ts", "src/orders/index.ts", "src/index.ts", "test/orders/reading.test.ts"]}"#;
const TYPESCRIPT_FAILED: &str = r#"{"passed": false, "failures": ["check 3 (npm test): 1 failing: an order with no items is refused"]}"#;

// The files the one scripted `write_files` call lays for the TypeScript
// target: a part of the `orders` stem in its own directory, with its
// manifest, its line in the stem's list and the stem's in the entry, and its
// test beside the stem's.
const TYPESCRIPT_WRITTEN: [(&str, &str); 5] = [
    ("src/orders/reading/package.json", "{ \"name\": \"@app/orders-reading\" }\n"),
    ("src/orders/reading/index.ts", "export function getOrder() {}\n"),
    ("src/orders/index.ts", "export * as reading from \"./reading/index.ts\";\n"),
    ("src/index.ts", "export * as orders from \"./orders/index.ts\";\n"),
    ("test/orders/reading.test.ts", "import { test } from \"node:test\";\n"),
];

const OMNIA_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/tests/reading.rs", "crates/common/src/topic.rs", "Cargo.toml", "src/routes.rs", "src/messaging.rs"]}"#;
// The same slice's report where its turn edited no list, so it names its
// stem's files alone.
const OMNIA_STEM_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/tests/reading.rs"]}"#;
// The lines the omnia slice adds to the seed's lists, each after the anchor
// the seeded file holds: the topic table's constant at its end, the route
// list's and the topic list's statements, and the stem in the root
// manifest's two lists.
const OMNIA_TABLE_LINE: &str =
    "\n/// An order was placed.\npub const ORDER_PLACED: &str = \"order-placed\";\n";
const OMNIA_ROUTE_ANCHOR: &str = "    let router = axum::Router::new();\n";
const OMNIA_ROUTE_LINE: &str = "    let router = router.merge(orders::router());\n";
const OMNIA_TOPICS_ANCHOR: &str =
    "    let router = messaging::Router::new(Client::new(OWNER, provider));\n";
const OMNIA_TOPICS_LINE: &str = "    let router = orders::topics(router, &env);\n";
const OMNIA_PATH_CRATE_ANCHOR: &str = "common = { path = \"crates/common\" }\n";
const OMNIA_PATH_CRATE_LINE: &str = "orders = { path = \"crates/orders\" }\n";
const OMNIA_STEM_ANCHOR: &str = "tracing.workspace = true\n";
const OMNIA_STEM_LINE: &str = "orders.workspace = true\n";
const OMNIA_FAILED: &str = r#"{"passed": false, "failures": ["check 2 (cargo test --workspace): crates/orders/tests/reading.rs — `an_order_with_no_items_is_refused` fails: expected `bad_request`, got Ok"]}"#;
// A pin of the root manifest and the edit a turn might put to it.
const OMNIA_PIN: &str = "omnia-sdk = \"0.36.0\"\n";
const OMNIA_PIN_EDITED: &str = "omnia-sdk = \"0.37.0\"\n";

// The three root files the node target's initialisation lays, as the crate
// embeds them.
const NODE_SEED: [(&str, &str); 3] = [
    (".gitignore", include_str!("../adapters/node/seed/.gitignore")),
    ("package.json", include_str!("../adapters/node/seed/package.json")),
    ("tsconfig.json", include_str!("../adapters/node/seed/tsconfig.json")),
];

// The files the one scripted `write_files` call lays for the omnia target: a
// part of the `orders` stem as a module of the stem's crate, with the stem's
// manifest, its line in the stem's list, and its test in the stem's
// `tests/`; the seed's lists are edited in place.
const OMNIA_WRITTEN: [(&str, &str); 4] = [
    ("crates/orders/Cargo.toml", "[package]\nname = \"orders\"\n\n[lints]\nworkspace = true\n"),
    ("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n"),
    ("crates/orders/src/reading.rs", "pub async fn get_order() {}\n"),
    (
        "crates/orders/tests/reading.rs",
        "#[tokio::test]\nasync fn an_order_is_returned_by_its_id() {}\n",
    ),
];
// The `orders` stem's entry in the layout's shape: the part's line, then
// the `router()` the root route list merges and the `topics(..)` the root
// topic list registers, each a fold of `common` over one entry per part.
const OMNIA_ENTRY: &str = "//! # orders\n//!\n//! The `orders` stem.\n\npub mod reading;\n\nuse \
                           common::Capabilities;\nuse omnia_sdk::api::{Client, messaging};\n\n/// \
                           The stem's routes, merged onto the root router.\npub fn router<P: \
                           Capabilities>() -> axum::Router<Client<P>> {\n    common::routers([\n        \
                           // one entry per part with routes, each added at the end\n        \
                           reading::routes(),\n    ])\n}\n\n/// The stem's topics, registered on \
                           the root messaging router under `env`.\n#[must_use]\npub fn topics<P: \
                           Capabilities>(router: messaging::Router<P>, env: &str) -> \
                           messaging::Router<P> {\n    common::topics(router, env).parts([\n        \
                           // one entry per part with topics, each added at the end\n        \
                           reading::topics,\n    ])\n}\n";
// The part behind the entry: its path as a `pub(crate)` constant, its
// routes and topics, and its handlers, `pub(crate)` so one the stem never
// routes is the compiler's to refuse.
const OMNIA_PART: &str = "//! Reads orders.\n\nuse common::{Capabilities, config, topic};\nuse \
                          omnia_sdk::api::http::get;\nuse omnia_sdk::api::{Client, Context, \
                          messaging};\n\npub(crate) const ORDER: &str = \"/orders/{id}\";\n\n\
                          pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
                          axum::Router::new().route(ORDER, get(get_order))\n}\n\npub(crate) fn \
                          topics<P: Capabilities>(\n    router: messaging::Router<P>, env: \
                          &str,\n) -> messaging::Router<P> {\n    \
                          router.route(config::topic_for(env, topic::ORDER_PLACED), \
                          messaging::consume(on_placed))\n}\n\npub(crate) async fn \
                          get_order<P>(input: GetOrder, context: Context<P>) -> \
                          omnia_sdk::Result<Order> {\n    todo!()\n}\n\npub(crate) async fn \
                          on_placed<P>(input: Order, context: Context<P>) -> omnia_sdk::Result<()> \
                          {\n    todo!()\n}\n";

// A shipped target adapter: its name, the prose its turns open under, and
// the `Adapter` every call goes through.
struct Shipped<A> {
    name: &'static str,
    docs: &'static [Doc],
    adapter: PhantomData<fn() -> A>,
}

const NODE: Shipped<emery_node::Adapter> = Shipped::new("node", emery_node::PROSE);

const OMNIA: Shipped<emery_omnia::Adapter> = Shipped::new("omnia", emery_omnia::PROSE);

impl<A: TargetAdapter> Shipped<A> {
    const fn new(name: &'static str, docs: &'static [Doc]) -> Self {
        Self {
            name,
            docs,
            adapter: PhantomData,
        }
    }

    // One initialisation of `project`, as the component's export puts it
    // before any slice builds into the tree.
    async fn init(&self, project: &Scratch) -> Result<(), Error> {
        let ctx = InitContext {
            adapter_id: self.name,
            workspace: root(project),
        };
        match A::run(Task::<Scripted>::Init(ctx)).await? {
            Outcome::Initialised => Ok(()),
            outcome => panic!("an init answers an initialised tree, not {outcome}"),
        }
    }

    // One build of the slice into `project`, as the component's export puts it.
    async fn build(&self, project: &Scratch, model: &Scripted) -> Result<Report, Error> {
        self.build_slice(project, model, &slice()).await
    }

    // One build of `slice` into `project`, for a scenario over a slice of its own.
    async fn build_slice(
        &self, project: &Scratch, model: &Scripted, slice: &Slice,
    ) -> Result<Report, Error> {
        let ctx = BuildContext {
            adapter_id: self.name,
            slice,
            workspace: root(project),
            worker: &A::WORKER,
            model,
        };
        match A::run(Task::Build(ctx)).await? {
            Outcome::Built(report) => Ok(report),
            outcome => panic!("a build answers a report, not {outcome}"),
        }
    }

    // One verification of `project`, as the component's export puts it.
    async fn verify(&self, project: &Scratch, model: &Scripted) -> Result<Verdict, Error> {
        let ctx = VerifyContext {
            adapter_id: self.name,
            workspace: root(project),
            worker: &A::WORKER,
            model,
        };
        match A::run(Task::Verify(ctx)).await? {
            Outcome::Verified(verdict) => Ok(verdict),
            outcome => panic!("a verify answers a verdict, not {outcome}"),
        }
    }

    // The grant the adapter's `WORKER` puts on a turn, as the script records it.
    fn granted() -> Option<SeenWorker> {
        A::WORKER.grant().map(SeenWorker::from)
    }

    // The prompt at `path` as this build compiled it in.
    fn prompt(&self, path: &str) -> &'static str {
        body(self.docs, path).unwrap_or_else(|| panic!("`{path}` is in `{}`'s PROSE", self.name))
    }

    // One initialisation of an empty tree, then one build laying `written`
    // through one `write_files` call, then the `edits` through one
    // `edit_files` call, and answering `report`, then one verify passing,
    // with what every target shares asserted: the init puts no turn; the
    // build turn opens under the embedded `build.md` with the tree lent and
    // both write tools beside the reference tools, its brief carrying the
    // slice; what the model wrote through the tools is what the tree holds
    // and the report names; the verify turn opens under `verify.md` over the
    // same tree, both tools declared for its repairs; each turn carries the
    // adapter's worker grant. Answers the tree, for what the target's own
    // layout must hold.
    async fn built(
        &self, written: &[(&str, &str)], edits: &[(&str, &str, &str)], report: &str,
    ) -> Scratch {
        let project = scratch();
        self.init(&project).await.expect("init over the empty tree");
        let mut calls = vec![write_files(written)];
        if !edits.is_empty() {
            calls.push(edit_files(edits));
        }
        let calls_made = calls.len();
        let model = Scripted::answering([report, PASSED]).calling(0, calls);

        let report = self.build(&project, &model).await.expect("build over the lent tree");
        let verdict = self.verify(&project, &model).await.expect("verify over the built tree");
        model.assert_exhausted();

        let findings = report.findings(&slice());
        assert!(findings.is_empty(), "report gate findings:\n{}", findings.join("\n"));
        assert_eq!(report.covered, ["REQ-001"], "the slice's one requirement is covered");
        assert!(verdict.passed, "the scripted verdict passes");

        let seen = model.seen();
        assert_eq!(seen.len(), 2, "the init opens none, the build one completion, the verify one");
        let build = &seen[0];
        assert!(
            build
                .system
                .as_deref()
                .is_some_and(|system| system.starts_with(self.prompt("build.md"))),
            "the compiled-in build prompt leads the system"
        );
        assert_eq!(
            build.workspace.as_deref(),
            Some(root(&project)),
            "the tree is lent to the build"
        );
        for tool in ["write_files", "edit_files"] {
            assert!(
                build.tools.iter().any(|declared| declared == tool),
                "the build writes through `{tool}`: {:?}",
                build.tools
            );
        }
        assert_eq!(build.worker, Self::granted(), "the build's worker carries the adapter's grant");
        for carried in [
            "SLICE-001",
            "orders-reading",
            "REQ-001",
            "0123456789abcdef0123456789abcdef01234567",
            "## Type: Order",
        ] {
            assert!(
                build.messages[0].contains(carried),
                "the brief carries `{carried}`: {}",
                build.messages[0]
            );
        }

        let verify = &seen[1];
        assert!(
            verify
                .system
                .as_deref()
                .is_some_and(|system| system.starts_with(self.prompt("verify.md"))),
            "the compiled-in verify prompt leads the system"
        );
        assert_eq!(
            verify.workspace.as_deref(),
            Some(root(&project)),
            "the integrated tree is lent to the verify"
        );
        for tool in ["write_files", "edit_files"] {
            assert!(
                verify.tools.iter().any(|declared| declared == tool),
                "a verify turn repairs through `{tool}`: {:?}",
                verify.tools
            );
        }
        assert_eq!(verify.worker, Self::granted(), "the verify's worker carries the same grant");

        let calls: Vec<_> = model
            .exchanges()
            .into_iter()
            .filter(|exchange| exchange.tool == "write_files" || exchange.tool == "edit_files")
            .collect();
        assert_eq!(calls.len(), calls_made, "every scripted call reached its tool: {calls:?}");
        assert!(calls.iter().all(|call| call.outcome.is_ok()), "every call landed: {calls:?}");
        for (path, _) in written {
            assert!(project.read(path).is_some(), "the tree holds `{path}` from the one call");
            assert!(report.written.iter().any(|named| named == path), "the report names `{path}`");
        }
        for (path, _, _) in edits {
            assert!(report.written.iter().any(|named| named == path), "the report names `{path}`");
        }
        project
    }

    // One verify over `project`, answering `failed`: a verdict that fails is
    // the adapter's answer, carried whole to the caller.
    async fn failed(&self, project: &Scratch, failed: &str) {
        let model = Scripted::answering([failed]);

        let verdict = self
            .verify(project, &model)
            .await
            .expect("a failed verdict is an answer, not an error");
        model.assert_exhausted();

        let findings = verdict.findings();
        assert!(findings.is_empty(), "verdict gate findings:\n{}", findings.join("\n"));
        assert!(!verdict.passed, "the checks failed");
        assert_eq!(verdict.failures.len(), 1, "the failure the checks found is listed");
        let seen = model.seen();
        assert_eq!(seen.len(), 1, "one verify turn");
        assert!(
            seen[0]
                .system
                .as_deref()
                .is_some_and(|system| system.starts_with(self.prompt("verify.md"))),
            "the compiled-in verify prompt leads the system"
        );
        for tool in ["write_files", "edit_files"] {
            assert!(
                seen[0].tools.iter().any(|declared| declared == tool),
                "a verify turn repairs through `{tool}`: {:?}",
                seen[0].tools
            );
        }
    }
}

// The slice every build is handed: one requirement under the `orders` stem,
// in a slice named for a part of it.
fn slice() -> Slice {
    slice_for("orders-reading", &["orders.get-id.return-order"])
}

// A slice named `name` over one requirement per subject, `REQ-001` on, its
// specification cut rendering each under its heading as the engine does.
fn slice_for(name: &str, subjects: &[&str]) -> Slice {
    let requirements: Vec<String> =
        (1..=subjects.len()).map(|number| format!("REQ-{number:03}")).collect();
    let spec = subjects
        .iter()
        .zip(&requirements)
        .map(|(subject, id)| {
            format!(
                "\n### Requirement: {subject}\n\nID: {id}\n\nThe behaviour `{subject}` states.\n"
            )
        })
        .fold("# Specification\n".to_owned(), |spec, block| spec + &block);
    Slice {
        id: "SLICE-001".to_owned(),
        name: name.to_owned(),
        base: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        spec,
        design: "# Design\n\n## Type: Order\n\nAn id and its items.\n".to_owned(),
        plan: format!(
            "## Slice: {name}\n\nID: SLICE-001\nRequirements: [{}]\nTypes: [Order]\n",
            requirements.join(", ")
        ),
        requirements,
    }
}

// One `write_files` call laying `files`, as the model would put it.
fn write_files(files: &[(&str, &str)]) -> ToolCall {
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

// One `edit_files` call putting `replace` where `find` stands in each file,
// as the model would put it.
fn edit_files(edits: &[(&str, &str, &str)]) -> ToolCall {
    let edits: Vec<_> = edits
        .iter()
        .map(|(path, find, replace)| {
            serde_json::json!({ "path": path, "find": find, "replace": replace })
        })
        .collect();
    ToolCall {
        id: "call-2".to_owned(),
        name: "edit_files".to_owned(),
        arguments: serde_json::json!({ "edits": edits }).to_string(),
    }
}

// One `write_files` call removing `paths`, as the model would put it.
fn delete_files(paths: &[&str]) -> ToolCall {
    ToolCall {
        id: "call-3".to_owned(),
        name: "write_files".to_owned(),
        arguments: serde_json::json!({ "delete": paths }).to_string(),
    }
}

// The correction the adapter's check put to the model: the one `check`
// exchange the script refused.
fn correction(model: &Scripted) -> String {
    let refused: Vec<String> = model
        .exchanges()
        .into_iter()
        .filter(|exchange| exchange.tool == "check")
        .filter_map(|exchange| exchange.outcome.err())
        .collect();
    let [correction] = refused.as_slice() else {
        panic!("one candidate is refused: {refused:?}");
    };
    correction.clone()
}

fn root(project: &Scratch) -> &str {
    project.path().to_str().expect("a UTF-8 scratch root")
}

// The three root files are laid by the initialisation, before the slice's
// turn, and the slice lays its own directory and lines beside them,
// reporting none of the root.
#[tokio::test]
async fn node() {
    let project = NODE.built(&TYPESCRIPT_WRITTEN, &[], TYPESCRIPT_REPORT).await;

    for (path, content) in NODE_SEED {
        assert_eq!(
            project.read(path).as_deref(),
            Some(content.as_bytes()),
            "the tree holds `{path}` as the initialisation laid it"
        );
    }
    assert_eq!(
        project.read("src/index.ts").as_deref(),
        Some("export * as orders from \"./orders/index.ts\";\n".as_bytes()),
        "the entry holds the stem's one line"
    );
    assert_eq!(
        project.read("src/orders/index.ts").as_deref(),
        Some("export * as reading from \"./reading/index.ts\";\n".as_bytes()),
        "the stem's list holds the part's one line"
    );
}

// The initialisation lays each root file the tree lacks and nothing over
// what it holds: a held manifest stands as it is, a link at a root path is
// left standing, and a tree initialised once is changed by nothing after.
#[tokio::test]
async fn node_init() {
    let project = scratch();
    project.write("package.json", "{ \"name\": \"held\" }\n");
    symlink("../outside", project.path().join(".gitignore")).expect("laying the link");

    NODE.init(&project).await.expect("init over the held tree");

    assert_eq!(
        project.read("tsconfig.json").as_deref(),
        Some(NODE_SEED[2].1.as_bytes()),
        "the file the tree lacked is laid as the crate embeds it"
    );
    assert_eq!(
        project.read("package.json").as_deref(),
        Some(b"{ \"name\": \"held\" }\n".as_slice()),
        "a held manifest stands"
    );
    assert_eq!(
        std::fs::read_link(project.path().join(".gitignore")).expect("the link stands").to_str(),
        Some("../outside"),
        "a link at a root path is left standing"
    );
    assert_eq!(
        files_under(project.path()),
        ["package.json", "tsconfig.json"].map(str::to_owned).into()
    );

    NODE.init(&project).await.expect("init over the initialised tree");
    assert_eq!(
        files_under(project.path()),
        ["package.json", "tsconfig.json"].map(str::to_owned).into()
    );
}

// The rules a wave's slices merge under: the lockfile kept from the
// integrated side, the entry and every list beneath it as a union, and every
// manifest under none.
#[test]
fn node_rules() {
    let declared: Vec<(&str, MergeStrategy)> = emery_node::Adapter::MERGE_RULES
        .iter()
        .map(|rule| (rule.paths.as_ref(), rule.strategy))
        .collect();

    assert_eq!(
        declared,
        [
            ("package-lock.json", MergeStrategy::Ours),
            ("src/index.ts", MergeStrategy::Union),
            ("**/index.ts", MergeStrategy::Union),
        ],
        "the merge rules the adapter declares, in order"
    );
}

// What a build's or verify's worker reaches beyond the tree, as the grant the
// turns carry: the npm registry, a Node installed beneath the home by nvm,
// Volta, or fnm, and Volta's root named for it. The grant is asserted rather
// than the reaches, so the adapter may regroup them while any widening shows.
#[test]
fn node_worker() {
    assert_eq!(
        emery_node::Adapter::WORKER.grant().map(SeenWorker::from),
        Some(SeenWorker {
            egress: vec!["registry.npmjs.org".to_owned()],
            read: vec![
                "~/.nvm/versions/node".to_owned(),
                "~/.volta".to_owned(),
                "~/.local/share/fnm".to_owned(),
            ],
            env: vec![("VOLTA_HOME".to_owned(), "~/.volta".to_owned())],
        }),
        "the worker grant the adapter's turns carry"
    );
    assert_eq!(emery_node::Adapter::WORKER.findings(), Vec::<String>::new());
}

#[tokio::test]
async fn node_verify() {
    let project = scratch();
    project.write("package.json", "{}");
    NODE.failed(&project, TYPESCRIPT_FAILED).await;
}

// The seed is laid by the initialisation, before the slice's turn, and is
// none of the slice's: the slice lays its stem's files and adds its lines
// to the seeded lists in place, the topic table, the route list, the topic
// list, and the root manifest's two lists, every other file of the seed
// standing as the template spells it.
#[tokio::test]
async fn omnia() {
    let table = omnia_seed("crates/common/src/topic.rs");
    let written = [
        OMNIA_WRITTEN[0],
        ("crates/orders/src/lib.rs", OMNIA_ENTRY),
        ("crates/orders/src/reading.rs", OMNIA_PART),
        OMNIA_WRITTEN[3],
    ];
    let edits = [
        ("crates/common/src/topic.rs", &*table, &*format!("{table}{OMNIA_TABLE_LINE}")),
        ("src/routes.rs", OMNIA_ROUTE_ANCHOR, &*format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}")),
        (
            "src/messaging.rs",
            OMNIA_TOPICS_ANCHOR,
            &*format!("{OMNIA_TOPICS_ANCHOR}{OMNIA_TOPICS_LINE}"),
        ),
        (
            "Cargo.toml",
            OMNIA_PATH_CRATE_ANCHOR,
            &*format!("{OMNIA_PATH_CRATE_ANCHOR}{OMNIA_PATH_CRATE_LINE}"),
        ),
        ("Cargo.toml", OMNIA_STEM_ANCHOR, &*format!("{OMNIA_STEM_ANCHOR}{OMNIA_STEM_LINE}")),
    ];
    let project = OMNIA.built(&written, &edits, OMNIA_REPORT).await;

    let edited: Vec<&str> = edits.iter().map(|(path, _, _)| *path).collect();
    for entry in omnia_template::ENTRIES {
        let held = project.read(entry.path);
        if edited.contains(&entry.path) {
            assert!(held.is_some(), "the tree holds the seeded list `{}`", entry.path);
        } else {
            assert_eq!(
                held.as_deref(),
                Some(entry.body.as_bytes()),
                "the tree holds the seed `{}` as the template spells it",
                entry.path
            );
        }
    }
    assert_eq!(
        text(&project, "crates/orders/src/lib.rs"),
        OMNIA_ENTRY,
        "the stem's entry holds the part's line and the two routers the root merges"
    );
    assert_eq!(
        text(&project, "crates/common/src/topic.rs"),
        format!("{table}{OMNIA_TABLE_LINE}"),
        "the topic table holds the topic's constant after its doc"
    );
    let routes = text(&project, "src/routes.rs");
    assert!(
        routes.contains(&format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}    router.with_state")),
        "the route list holds the stem's one statement before the terminal line: {routes}"
    );
    let topics = text(&project, "src/messaging.rs");
    assert!(
        topics.contains(&format!("{OMNIA_TOPICS_ANCHOR}{OMNIA_TOPICS_LINE}    tracing::debug!")),
        "the topic list holds the stem's one statement before its trace: {topics}"
    );
    let manifest = text(&project, "Cargo.toml");
    assert!(
        manifest.contains(&format!("{OMNIA_PATH_CRATE_ANCHOR}{OMNIA_PATH_CRATE_LINE}\n")),
        "the root manifest's path list holds the stem after `common`: {manifest}"
    );
    assert!(
        manifest.ends_with(&format!("{OMNIA_STEM_ANCHOR}{OMNIA_STEM_LINE}")),
        "the root manifest ends with the stem's one line: {manifest}"
    );
}

fn text(project: &Scratch, path: &str) -> String {
    let bytes = project.read(path).unwrap_or_else(|| panic!("the tree holds `{path}`"));
    String::from_utf8(bytes).expect("UTF-8")
}

// A tree that already holds a file of the seed keeps it, the root ignore
// file included, since the initialisation lays what the tree lacks and
// nothing over what it holds; a slice built over it names no file of the
// seed it did not edit.
#[tokio::test]
async fn omnia_init_held() {
    let project = scratch();
    for entry in omnia_template::ENTRIES {
        if entry.path == "Cargo.toml" {
            project.write(entry.path, entry.body.as_bytes());
        } else {
            project.write(entry.path, "# held\n");
        }
    }

    OMNIA.init(&project).await.expect("init over the held tree");

    for entry in omnia_template::ENTRIES {
        let expected = if entry.path == "Cargo.toml" { entry.body.as_bytes() } else { b"# held\n" };
        assert_eq!(
            project.read(entry.path).as_deref(),
            Some(expected),
            "`{}` stands as the tree held it",
            entry.path
        );
    }

    let model = Scripted::answering([OMNIA_STEM_REPORT]).calling(0, [write_files(&OMNIA_WRITTEN)]);
    let report = OMNIA.build(&project, &model).await.expect("build over the held tree");
    model.assert_exhausted();
    assert_eq!(
        report.written,
        OMNIA_WRITTEN.map(|(path, _)| path.to_owned()),
        "the report names the stem's files and no file of the seed"
    );
    assert_eq!(text(&project, ".gitignore"), "# held\n", "the build lays no file of the seed");
}

// The initialisation lays every file of the layout outside `crates/<stem>/`
// into an empty tree, each at a path the engine seals, the root manifest
// pinning omnia at one version so a bump is one edit; the template's ignore
// file keeps the build output and the engine's own directory out of a
// commit.
#[tokio::test]
async fn omnia_init_files() {
    let project = scratch();

    OMNIA.init(&project).await.expect("init over the empty tree");

    let paths = files_under(project.path());
    assert_eq!(
        paths,
        [
            ".github/dependabot.yml",
            ".github/workflows/audit.yaml",
            ".github/workflows/ci.yaml",
            ".github/workflows/patch.yaml",
            ".github/workflows/publish.yaml",
            ".github/workflows/release.yaml",
            ".gitignore",
            ".vscode/settings.json",
            "Cargo.toml",
            "Makefile",
            "clippy.toml",
            "crates/common/Cargo.toml",
            "crates/common/src/config.rs",
            "crates/common/src/lib.rs",
            "crates/common/src/topic.rs",
            "deny.toml",
            "examples/runtime.rs",
            "mise.toml",
            "rust-toolchain.toml",
            "rustfmt.toml",
            "src/lib.rs",
            "src/messaging.rs",
            "src/routes.rs",
            "supply-chain/README.md",
            "supply-chain/audits.toml",
            "supply-chain/config.toml",
            "taplo.toml",
            "tests/messaging.rs",
            "tests/routes.rs",
        ]
        .map(str::to_owned)
        .into_iter()
        .collect()
    );
    for path in &paths {
        emery_sdk::beneath(path).unwrap_or_else(|bad| panic!("the seed path `{path}` {bad}"));
        assert_eq!(
            project.read(path).as_deref(),
            Some(omnia_seed(path).as_bytes()),
            "`{path}` is laid as the template spells it"
        );
    }
    for rule in ["target/", ".emery/"] {
        assert!(
            omnia_seed(".gitignore").lines().any(|line| line == rule),
            "the template's ignore file holds `{rule}`"
        );
    }

    let manifest = omnia_seed("Cargo.toml");
    let pins: Vec<(&str, &str)> = manifest
        .lines()
        .filter(|line| line.starts_with("omnia"))
        .filter_map(|line| {
            let (name, rest) = line.split_once(" = ")?;
            let version = rest
                .split_once("version = \"")
                .map(|(_, version)| version)
                .or_else(|| rest.strip_prefix('"'))?;
            let (version, _) = version.split_once('"')?;
            Some((name.trim(), version))
        })
        .collect();
    assert!(pins.len() >= 2, "the seed pins the sdk and the host crates: {pins:?}");
    let versions: std::collections::BTreeSet<&str> =
        pins.iter().map(|(_, version)| *version).collect();
    assert_eq!(versions.len(), 1, "every omnia pin in the seed is one version: {pins:?}");
    assert!(manifest.contains(OMNIA_PIN), "the scenarios edit the pin the seed holds");
}

// The rules a wave's slices merge under: the lockfile kept from the
// integrated side, every file holding a list as a union, and every other
// path under none, the entry `src/lib.rs` among them.
#[test]
fn omnia_rules() {
    let declared: Vec<(&str, MergeStrategy)> = emery_omnia::Adapter::MERGE_RULES
        .iter()
        .map(|rule| (rule.paths.as_ref(), rule.strategy))
        .collect();

    assert_eq!(
        declared,
        [
            ("Cargo.lock", MergeStrategy::Ours),
            ("Cargo.toml", MergeStrategy::Union),
            ("src/routes.rs", MergeStrategy::Union),
            ("src/messaging.rs", MergeStrategy::Union),
            ("crates/common/src/topic.rs", MergeStrategy::Union),
            ("crates/*/Cargo.toml", MergeStrategy::Union),
            ("crates/*/src/lib.rs", MergeStrategy::Union),
        ],
        "the merge rules the adapter declares, in order"
    );
}

// The rules follow the template: every file the template lays as a list is
// union-merged, every union rule over one path is such a file, and the rules
// over a glob are the two files of a stem's crate that hold its lists, so a
// list the template adds or drops moves the rules with it.
#[test]
fn omnia_rules_follow_template() {
    let union: Vec<&str> = emery_omnia::Adapter::MERGE_RULES
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
        "every union rule over one path is a file the template lays as a list"
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

// What a build's or verify's worker reaches beyond the tree, as the grant the
// turns carry: the crates.io index and its downloads, the toolchain and the
// binaries beneath the home, never `~/.cargo` whole, and `RUSTUP_HOME` named
// for it. The grant is asserted rather than the reaches, so the adapter may
// regroup them while any widening shows.
#[test]
fn omnia_worker() {
    assert_eq!(
        emery_omnia::Adapter::WORKER.grant().map(SeenWorker::from),
        Some(SeenWorker {
            egress: vec!["index.crates.io".to_owned(), "static.crates.io".to_owned()],
            read: vec!["~/.rustup".to_owned(), "~/.cargo/bin".to_owned()],
            env: vec![("RUSTUP_HOME".to_owned(), "~/.rustup".to_owned())],
        }),
        "the worker grant the adapter's turns carry"
    );
    assert_eq!(emery_omnia::Adapter::WORKER.findings(), Vec::<String>::new());
}

#[tokio::test]
async fn omnia_verify() {
    OMNIA.failed(&omnia_tree().await, OMNIA_FAILED).await;
}

// A verify whose repair changed a static file, over a tree that lacks
// another, is refused with each named: the lines to put back for the one,
// the template's body whole for the other, since the turn has no other way
// to lay it. The repair puts both back and the verdict is accepted over the
// tree as the template spells it.
#[tokio::test]
async fn omnia_static_drift_refused() {
    let project = omnia_tree().await;
    std::fs::remove_file(project.path().join("examples/runtime.rs")).expect("the seed laid it");
    let entry = omnia_seed("src/lib.rs");
    let first = entry.lines().next().expect("the entry opens with a line");
    let runtime = omnia_seed("examples/runtime.rs");
    let model = Scripted::answering([PASSED, PASSED])
        .calling(0, [edit_files(&[("src/lib.rs", first, "//! Edited by a repair.")])])
        .calling(
            1,
            [
                write_files(&[("examples/runtime.rs", &runtime)]),
                edit_files(&[("src/lib.rs", "//! Edited by a repair.", first)]),
            ],
        );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    for line in [
        "`src/lib.rs` is a static file of the template and differs from how the template spells it",
        &format!("- {first}"),
        "+ //! Edited by a repair.",
        "`examples/runtime.rs` is a static file of the template the tree lacks",
        runtime.lines().next().expect("the runtime opens with a line"),
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for path in ["src/lib.rs", "examples/runtime.rs"] {
        assert_eq!(text(&project, path), omnia_seed(path), "`{path}` is the template's again");
    }
}

// The root manifest is held to the seed outside its two lists by the
// adapter, not by the merge: a slice that edits a pin beside its stem's
// lines is refused with the pin's lines named, puts it back, and its
// accepted report carries the manifest with the stem's lines kept.
#[tokio::test]
async fn omnia_manifest_refused() {
    let project = omnia_tree().await;
    let edits = [
        (
            "Cargo.toml",
            OMNIA_PATH_CRATE_ANCHOR,
            &*format!("{OMNIA_PATH_CRATE_ANCHOR}{OMNIA_PATH_CRATE_LINE}"),
        ),
        ("Cargo.toml", OMNIA_STEM_ANCHOR, &*format!("{OMNIA_STEM_ANCHOR}{OMNIA_STEM_LINE}")),
        ("Cargo.toml", OMNIA_PIN, OMNIA_PIN_EDITED),
    ];
    let model = Scripted::answering([OMNIA_REPORT, OMNIA_REPORT])
        .calling(0, [write_files(&OMNIA_WRITTEN), edit_files(&edits)])
        .calling(1, [edit_files(&[("Cargo.toml", OMNIA_PIN_EDITED, OMNIA_PIN)])]);

    let report = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "the root `Cargo.toml` changed outside its two stem lists",
        "- omnia-sdk = \"0.36.0\"",
        "+ omnia-sdk = \"0.37.0\"",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert!(
        !correction.contains(OMNIA_STEM_LINE.trim_end()),
        "the stem's lines are the lists' and no drift: {correction}"
    );
    assert!(report.written.iter().any(|path| path == "Cargo.toml"), "the manifest is reported");
    assert_eq!(
        text(&project, "Cargo.toml"),
        omnia_manifest_with_orders(),
        "the pin stands as the seed spells it and the stem's lines are kept"
    );
}

// A `crates/start/` crate and a stem's module that runs the process are
// refused at a wave's verify with every line named, whatever the checks
// answered, a stem's tests unread, and the crate and the module removed are
// the repair. What a module reaches past the host is the workspace's lints'
// to refuse, in the loop, and no rule of the adapter's.
#[tokio::test]
async fn omnia_verify_process() {
    let project = omnia_tree().await;
    project.write("crates/start/Cargo.toml", "[package]\nname = \"start\"\n");
    project.write("crates/start/src/lib.rs", "//! The `start` stem.\n");
    project.write(
        "crates/orders/Cargo.toml",
        "[package]\nname = \"orders\"\n\n[lints]\nworkspace = true\n",
    );
    project.write(
        "crates/orders/src/consuming.rs",
        "#![allow(clippy::pedantic)]\n//! Consumes orders.\n\nuse std::env;\n\npub fn consume() {\n    let _ = env::var(\"PORT\");\n    tokio::spawn(async {});\n    std::thread::sleep(std::time::Duration::from_secs(1));\n}\n",
    );
    project.write(
        "crates/orders/src/main_loop.rs",
        "//! The loop.\n\npub fn run() {\n    loop {}\n}\n",
    );
    project.write("crates/orders/tests/runner.rs", "#[test]\nfn run() {}\n");
    let removed =
        ["crates/start/Cargo.toml", "crates/start/src/lib.rs", "crates/orders/src/main_loop.rs"];
    let model = Scripted::answering([PASSED, PASSED]).calling(1, [delete_files(&removed)]);

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    for line in [
        "`crates/start/` is laid for `start`, which the seed and the host are and no crate of the \
         tree ever is",
        "crates/start/Cargo.toml\ncrates/start/src/lib.rs",
        "a module is the process the source was",
        "crates/orders/src/main_loop.rs:3: a run of its own (`fn run`)",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["main_loop.rs:4", "consuming.rs", "tests/"] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    for path in removed {
        assert!(project.read(path).is_none(), "`{path}` is removed");
    }
}

// A module under a stem's `src/` that runs the process is refused at the
// build rather than left to a wave's verify, the module to remove named
// once; the slice's own turn deletes it, and the accepted report names what
// the tree still holds.
#[tokio::test]
async fn omnia_build_process_refused() {
    let project = omnia_tree().await;
    let written = [
        ("crates/orders/Cargo.toml", "[package]\nname = \"orders\"\n\n[lints]\nworkspace = true\n"),
        (
            "crates/orders/src/lib.rs",
            "//! The `orders` stem.\n\npub mod reading;\npub mod runner;\n",
        ),
        ("crates/orders/src/reading.rs", "pub async fn get_order() {}\n"),
        (
            "crates/orders/src/runner.rs",
            "//! Runs the process (`fn start` in `struct App`).\n\n/// Run it.\npub fn run() {\n    \
             loop {}\n}\n",
        ),
    ];
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/src/runner.rs"]}"#;
    let repaired = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs"]}"#;
    let model = Scripted::answering([first, repaired]).calling(0, [write_files(&written)]).calling(
        1,
        [
            edit_files(&[("crates/orders/src/lib.rs", "pub mod runner;\n", "")]),
            delete_files(&["crates/orders/src/runner.rs"]),
        ],
    );

    let report = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "a module is the process the source was",
        "crates/orders/src/runner.rs:4: a run of its own (`fn run`)",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["runner.rs:1", "runner.rs:5", "reading.rs:1"] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    assert!(project.read("crates/orders/src/runner.rs").is_none(), "the module is removed");
    assert!(
        !report.written.iter().any(|path| path == "crates/orders/src/runner.rs"),
        "a removed module is not reported: {:?}",
        report.written
    );
}

// A stem's manifest that does not inherit the workspace's lints, such as a
// `lints.workspace = true` key nested under `[package]`, is refused at the
// build with the manifest named, since the lints are the host boundary and
// reach a crate through `[lints]` alone. The turn adds the `[lints]` table
// and the report is accepted.
#[tokio::test]
async fn omnia_stem_without_workspace_lints() {
    let project = omnia_tree().await;
    let unlinted = "[package]\nname = \"orders\"\nlints.workspace = true\n\n[dependencies]\nomnia-sdk.workspace = true\n";
    let written = [
        ("crates/orders/Cargo.toml", unlinted),
        ("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n"),
        ("crates/orders/src/reading.rs", "pub async fn get_order() {}\n"),
    ];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs"]}"#;
    let model = Scripted::answering([report, report]).calling(0, [write_files(&written)]).calling(
        1,
        [edit_files(&[(
            "crates/orders/Cargo.toml",
            "lints.workspace = true\n",
            "\n[lints]\nworkspace = true\n",
        )])],
    );

    OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in
        ["a stem's manifest carries `[lints]` with `workspace = true`", "crates/orders/Cargo.toml"]
    {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert!(
        text(&project, "crates/orders/Cargo.toml").contains("[lints]\nworkspace = true"),
        "the manifest inherits the lints"
    );
}

// Laying `crates/start/` (or a `crates/start-*` crate for a slice cut from
// `start`, such as `crates/start-prefetch/`) at a build is refused whole: the
// seed and the host are `start`, so every file under `crates/start` or
// `crates/start-*` is named and removed through `delete` with any `start` line
// dropped from the root `Cargo.toml`, its modules ignored by the other module
// rules so no router, loader, or miss inside it is asked to be merged or
// edited in place.
#[tokio::test]
async fn omnia_build_start_crate_refused() {
    let project = omnia_tree().await;
    let start_path = "start = { path = \"crates/start\" }\n";
    let start_crate = "start.workspace = true\n";
    let written = [
        ("crates/start/Cargo.toml", "[package]\nname = \"start\"\n"),
        (
            "crates/start/src/lib.rs",
            "//! The `start` stem.\n\npub mod start;\n\n/// The stem's routes.\npub fn router<P: \
             common::Capabilities>() -> axum::Router<omnia_sdk::api::Client<P>> {\n    \
             axum::Router::new().merge(start::routes())\n}\n",
        ),
        (
            "crates/start/src/start.rs",
            "//! Startup helper.\n\nuse omnia_sdk::api::Context;\nuse omnia_sdk::{Config, \
             StateStore};\n\npub const START_BROKER_URL: &str = \"START_BROKER_URL\";\npub const \
             TABLE_KEY: &str = \"start/table\";\n\n/// Probe readiness.\npub async fn ready<P: \
             Config + StateStore>(\n    _input: (),\n    context: Context<P>,\n) -> \
             omnia_sdk::Result<Vec<String>> {\n    let _ = Config::get(context.provider(), \
             START_BROKER_URL).await?;\n    match StateStore::get(context.provider(), \
             TABLE_KEY).await? {\n        Some(_) => Ok(Vec::new()),\n        None => \
             Ok(Vec::new()),\n    }\n}\n",
        ),
        (
            "crates/start-prefetch/src/lib.rs",
            "//! Prefetches stops.\n\nuse omnia_sdk::{Identity, StateStore};\n\n/// Load the \
             stops into the store.\npub async fn load_stops<P: StateStore + Identity>(provider: \
             &P, stops: &[u8]) -> omnia_sdk::Result<()> {\n    let _ = \
             Identity::access_token(provider, \"orders\").await?;\n    \
             StateStore::set(provider, \"orders/stops\", stops, None).await\n}\n",
        ),
    ];
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/start/Cargo.toml", "crates/start/src/lib.rs", "crates/start/src/start.rs", "crates/start-prefetch/src/lib.rs", "Cargo.toml"]}"#;
    let repaired = r#"{"covered": [], "written": []}"#;
    let model = Scripted::answering([first, repaired])
        .calling(
            0,
            [
                write_files(&written),
                edit_files(&[
                    (
                        "Cargo.toml",
                        OMNIA_PATH_CRATE_ANCHOR,
                        &format!("{OMNIA_PATH_CRATE_ANCHOR}{start_path}"),
                    ),
                    ("Cargo.toml", OMNIA_STEM_ANCHOR, &format!("{OMNIA_STEM_ANCHOR}{start_crate}")),
                ]),
            ],
        )
        .calling(
            1,
            [
                edit_files(&[("Cargo.toml", start_path, ""), ("Cargo.toml", start_crate, "")]),
                delete_files(&[
                    "crates/start/Cargo.toml",
                    "crates/start/src/lib.rs",
                    "crates/start/src/start.rs",
                    "crates/start-prefetch/src/lib.rs",
                ]),
            ],
        );

    let report = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "`crates/start/` is laid for `start`, which the seed and the host are and no crate of the \
         tree ever is",
        "crates/start-prefetch/src/lib.rs\ncrates/start/Cargo.toml\ncrates/start/src/lib.rs\n\
         crates/start/src/start.rs",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for absent in [
        "`start::router()`",
        "crates/start/src/lib.rs:",
        "crates/start/src/start.rs:",
        "crates/start-prefetch/src/lib.rs:",
    ] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    assert!(project.read("crates/start/src/start.rs").is_none(), "`crates/start/` is removed");
    assert!(
        project.read("crates/start-prefetch/src/lib.rs").is_none(),
        "`crates/start-prefetch/` is removed"
    );
    assert!(
        !report.written.iter().any(|path| path.starts_with("crates/start")),
        "removed `crates/start*` files are not reported: {:?}",
        report.written
    );
}

// A `Deserialize` struct under a stem's `src/` whose body holds a single
// `data` field — wrapping an HTTP response in the source client's
// `response.data` property — is refused at the build with its path and line
// named, while a multi-field `Deserialize` struct and a struct without
// `Deserialize` pass. The slice's own turn removes the wrapper structs, and
// the accepted report names the module.
#[tokio::test]
async fn omnia_build_wrappers_refused() {
    let project = omnia_tree().await;
    let wrapped = "#[derive(Debug, Clone, Deserialize)]\npub struct StopsResponse {\n    \
                   #[serde(default)]\n    pub data: Vec<Stop>,\n}\n\n#[derive(Deserialize)]\n\
                   struct AllocationsResponse {\n    data: Allocations,\n}\n";
    let direct = "/// Vehicles allocated to a trip.\n#[derive(Debug, Clone, Deserialize)]\npub \
                  struct Allocations {\n    /// All vehicle allocations.\n    #[serde(default)]\n    \
                  pub all: Vec<String>,\n}\n\n#[derive(Deserialize)]\nstruct Published {\n    \
                  pubdata: String,\n}\n";
    let module = |body: &str| {
        format!(
            "//! Fetches stops and allocations.\n\nuse serde::Deserialize;\n\n/// A GTFS \
             stop.\n#[derive(Debug, Clone, Deserialize)]\npub struct Stop {{\n    /// The stop \
             code.\n    pub stop_code: String,\n    /// The stop latitude.\n    pub stop_lat: \
             f64,\n}}\n\n/// Raw bytes held in memory.\npub struct Holder {{\n    /// The \
             bytes.\n    pub data: Vec<u8>,\n}}\n\n{body}"
        )
    };
    let first_tree = module(&format!("{wrapped}\n{direct}"));
    let written = [
        ("crates/stops/Cargo.toml", "[package]\nname = \"stops\"\n\n[lints]\nworkspace = true\n"),
        ("crates/stops/src/lib.rs", "//! The `stops` stem.\n\npub mod fetching;\n"),
        ("crates/stops/src/fetching.rs", first_tree.as_str()),
    ];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/stops/Cargo.toml", "crates/stops/src/lib.rs", "crates/stops/src/fetching.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [write_files(&written)])
        .calling(1, [edit_files(&[("crates/stops/src/fetching.rs", &format!("{wrapped}\n"), "")])]);

    let accepted = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "a struct wraps an HTTP response in a single `data` field",
        "crates/stops/src/fetching.rs:21: response wrapper struct `StopsResponse` wraps a single \
         `data` field",
        "crates/stops/src/fetching.rs:27: response wrapper struct `AllocationsResponse` wraps a \
         single `data` field",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["`Stop`", "`Holder`", "`Allocations`", "`Published`"] {
        assert!(!correction.contains(absent), "{absent} is no finding: {correction}");
    }
    assert!(
        !text(&project, "crates/stops/src/fetching.rs").contains("StopsResponse"),
        "the wrapper struct is removed"
    );
    assert!(
        accepted.written.iter().any(|path| path == "crates/stops/src/fetching.rs"),
        "the module is reported: {:?}",
        accepted.written
    );
}

// A read of the store under a constant key whose miss is answered empty
// anywhere in the function that reads — a `None` arm answered as an empty
// map seven lines below it — is refused at the build with the answer named
// at its line and the read's beside it, while a read under a key built from
// the input, one whose miss fetches and stores the table, and a default on
// the hit's own arm, which decodes what the store held, pass. The slice's
// own turn lays the fetch into the read, and the accepted report names the
// module.
#[tokio::test]
async fn omnia_build_empty_miss_refused() {
    let project = omnia_tree().await;
    let empty = "    let cached = StateStore::get(provider, RATES_KEY).await?;\n    let table = \
                 match cached {\n        Some(bytes) => {\n            let decoded: HashMap<String, \
                 u32> = serde_json::from_slice(&bytes).unwrap_or_default();\n            \
                 tracing::debug!(entries = decoded.len(), \"rates read from the store\");\n            \
                 decoded\n        }\n        None => HashMap::new(),\n    };\n    Ok(table)\n";
    let fetched = "    if let Some(bytes) = StateStore::get(provider, RATES_KEY).await? {\n        \
                   return Ok(serde_json::from_slice(&bytes).unwrap_or_default());\n    }\n    let \
                   bytes = fetch_rates(provider).await?;\n    StateStore::set(provider, RATES_KEY, \
                   &bytes, Some(3600)).await?;\n    decode(&bytes)\n";
    let module = |rates: &str| {
        format!(
            "//! Rates by region.\n\nuse std::collections::HashMap;\n\nuse omnia_sdk::{{HttpRequest, \
             StateStore}};\n\n/// The store key the rate table sits under.\npub const RATES_KEY: \
             &str = \"rates/table\";\n\n/// The rate table, fetched when the store holds none.\n///\n\
             /// # Errors\n///\n/// `bad_gateway` when the upstream refuses.\npub async fn rates<P: \
             HttpRequest + StateStore>(provider: &P) -> omnia_sdk::Result<HashMap<String, \
             u32>> {{\n{rates}}}\n\n/// The regions a customer chose, none when none yet.\n///\n/// \
             # Errors\n///\n/// `server_error` when the store refuses.\npub async fn chosen<P: \
             StateStore>(provider: &P, customer: &str) -> omnia_sdk::Result<Vec<String>> {{\n    \
             let key = format!(\"rates/chosen/{{customer}}\");\n    let Some(bytes) = \
             StateStore::get(provider, &key).await? else {{\n        return Ok(Vec::new());\n    \
             }};\n    Ok(serde_json::from_slice(&bytes).unwrap_or_default())\n}}\n\nasync fn \
             fetch_rates<P: HttpRequest>(_provider: &P) -> omnia_sdk::Result<Vec<u8>> {{\n    \
             Ok(Vec::new())\n}}\n\nfn decode(bytes: &[u8]) -> omnia_sdk::Result<HashMap<String, \
             u32>> {{\n    Ok(serde_json::from_slice(bytes).unwrap_or_default())\n}}\n"
        )
    };
    let first_tree = module(empty);
    let written = [
        ("crates/rates/Cargo.toml", "[package]\nname = \"rates\"\n\n[lints]\nworkspace = true\n"),
        ("crates/rates/src/lib.rs", "//! The `rates` stem.\n\npub mod table;\n"),
        ("crates/rates/src/table.rs", first_tree.as_str()),
    ];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/rates/Cargo.toml", "crates/rates/src/lib.rs", "crates/rates/src/table.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [write_files(&written)])
        .calling(1, [edit_files(&[("crates/rates/src/table.rs", empty, fetched)])]);

    let accepted = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "a table read from the store under one key answers empty on a miss",
        "crates/rates/src/table.rs:23: a miss answered as no entries (`None => HashMap::new()`) \
         after the read at line 16",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert_eq!(
        correction.matches("crates/rates/src/table.rs:").count(),
        1,
        "a read under a key built from the input, a default outside the reading function, and \
         one on the hit's own arm are no finding: {correction}"
    );
    assert!(
        text(&project, "crates/rates/src/table.rs").contains("StateStore::set(provider, RATES_KEY"),
        "the miss fetches and stores the table"
    );
    assert!(
        accepted.written.iter().any(|path| path == "crates/rates/src/table.rs"),
        "the module is reported: {:?}",
        accepted.written
    );
}

// A read under a constant key in a crate-private function — `pub(crate)
// async fn`, which `rustfmt` lays with no space after `pub` — is held to
// the function it sits in as a public one is, so a `None` arm answered
// empty seven lines below the read is named rather than passed over. The
// slice's own turn lays the fetch into the read, and the accepted report
// names the module.
#[tokio::test]
async fn omnia_build_crate_private_miss_refused() {
    let project = omnia_tree().await;
    let empty = "    let cached = StateStore::get(provider, ZONES_KEY).await?;\n    let table = \
                 match cached {\n        Some(bytes) => {\n            let decoded: Vec<String> = \
                 serde_json::from_slice(&bytes)?;\n            tracing::debug!(entries = \
                 decoded.len(), \"zones read from the store\");\n            decoded\n        }\n        \
                 None => Vec::new(),\n    };\n    Ok(table)\n";
    let fetched = "    if let Some(bytes) = StateStore::get(provider, ZONES_KEY).await? {\n        \
                   return Ok(serde_json::from_slice(&bytes)?);\n    }\n    let bytes = \
                   fetch_zones(provider).await?;\n    StateStore::set(provider, ZONES_KEY, &bytes, \
                   Some(3600)).await?;\n    Ok(serde_json::from_slice(&bytes)?)\n";
    let module = |zones: &str| {
        format!(
            "//! Zones by region.\n\nuse omnia_sdk::{{HttpRequest, StateStore}};\n\n/// The store \
             key the zone table sits under.\npub const ZONES_KEY: &str = \
             \"zones/table\";\n\npub(crate) async fn zones<P: HttpRequest + StateStore>(provider: \
             &P) -> omnia_sdk::Result<Vec<String>> {{\n{zones}}}\n\nasync fn fetch_zones<P: \
             HttpRequest>(_provider: &P) -> omnia_sdk::Result<Vec<u8>> {{\n    Ok(Vec::new())\n}}\n"
        )
    };
    let first_tree = module(empty);
    let written = [
        ("crates/zones/Cargo.toml", "[package]\nname = \"zones\"\n\n[lints]\nworkspace = true\n"),
        ("crates/zones/src/lib.rs", "//! The `zones` stem.\n\nmod table;\n"),
        ("crates/zones/src/table.rs", first_tree.as_str()),
    ];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/zones/Cargo.toml", "crates/zones/src/lib.rs", "crates/zones/src/table.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [write_files(&written)])
        .calling(1, [edit_files(&[("crates/zones/src/table.rs", empty, fetched)])]);

    let accepted = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    assert!(
        correction.contains(
            "crates/zones/src/table.rs:16: a miss answered as no entries (`None => Vec::new()`) \
             after the read at line 9"
        ),
        "the arm past the window is named: {correction}"
    );
    assert_eq!(
        correction.matches("crates/zones/src/table.rs:").count(),
        1,
        "the fetch helper's own empty answer is no finding: {correction}"
    );
    assert!(
        accepted.written.iter().any(|path| path == "crates/zones/src/table.rs"),
        "the module is reported: {:?}",
        accepted.written
    );
}

// A miss answered empty with no `return` — an `Ok(HashMap::new())` standing
// as the function's tail once the hit has returned above it, and an arm
// naming `None` beside an error, `Ok(None) | Err(_)`, answering
// `Ok(BTreeMap::new())` — is named at its line as the returning spellings
// are, while the fetch helper's own empty tail, in a function of its own,
// is none. The slice's own turn lays the fetch into both reads, and the
// accepted report names the module.
#[tokio::test]
async fn omnia_build_tail_miss_refused() {
    let project = omnia_tree().await;
    let tail = "    if let Some(bytes) = StateStore::get(provider, FEES_KEY).await? {\n        return \
                Ok(serde_json::from_slice(&bytes)?);\n    }\n    Ok(HashMap::new())\n";
    let fetched = "    if let Some(bytes) = StateStore::get(provider, FEES_KEY).await? {\n        \
                   return Ok(serde_json::from_slice(&bytes)?);\n    }\n    let bytes = \
                   fetch_fees(provider).await?;\n    StateStore::set(provider, FEES_KEY, &bytes, \
                   Some(3600)).await?;\n    Ok(serde_json::from_slice(&bytes)?)\n";
    let arm = "        Ok(None) | Err(_) => Ok(BTreeMap::new()),\n";
    let loaded = "        Ok(None) => {\n            let bytes = fetch_fees(provider).await?;\n            \
                  StateStore::set(provider, SURCHARGES_KEY, &bytes, Some(3600)).await?;\n            \
                  Ok(serde_json::from_slice(&bytes)?)\n        }\n        Err(error) => \
                  Err(error),\n";
    let module = |fees: &str, none: &str| {
        format!(
            "//! Fees by region.\n\nuse std::collections::{{BTreeMap, HashMap}};\n\nuse \
             omnia_sdk::{{HttpRequest, StateStore}};\n\n/// The store key the fee table sits \
             under.\npub const FEES_KEY: &str = \"fees/table\";\n\n/// The store key the surcharge \
             table sits under.\npub const SURCHARGES_KEY: &str = \"fees/surcharges\";\n\n/// The fee \
             table, fetched when the store holds none.\n///\n/// # Errors\n///\n/// `bad_gateway` \
             when the upstream refuses.\npub async fn fees<P: HttpRequest + StateStore>(provider: \
             &P) -> omnia_sdk::Result<HashMap<String, u32>> {{\n{fees}}}\n\n/// The surcharge \
             table, fetched when the store holds none.\n///\n/// # Errors\n///\n/// `bad_gateway` \
             when the upstream refuses.\npub async fn surcharges<P: HttpRequest + \
             StateStore>(provider: &P) -> omnia_sdk::Result<BTreeMap<String, u32>> {{\n    match \
             StateStore::get(provider, SURCHARGES_KEY).await {{\n        Ok(Some(bytes)) => \
             Ok(serde_json::from_slice(&bytes)?),\n{none}    }}\n}}\n\nasync fn fetch_fees<P: \
             HttpRequest>(_provider: &P) -> omnia_sdk::Result<Vec<u8>> {{\n    Ok(Vec::new())\n}}\n"
        )
    };
    let first_tree = module(tail, arm);
    let written = [
        ("crates/fees/Cargo.toml", "[package]\nname = \"fees\"\n\n[lints]\nworkspace = true\n"),
        ("crates/fees/src/lib.rs", "//! The `fees` stem.\n\npub mod table;\n"),
        ("crates/fees/src/table.rs", first_tree.as_str()),
    ];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/fees/Cargo.toml", "crates/fees/src/lib.rs", "crates/fees/src/table.rs"]}"#;
    let model = Scripted::answering([report, report]).calling(0, [write_files(&written)]).calling(
        1,
        [edit_files(&[
            ("crates/fees/src/table.rs", tail, fetched),
            ("crates/fees/src/table.rs", arm, loaded),
        ])],
    );

    let accepted = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "crates/fees/src/table.rs:22: a miss answered as no entries (`Ok(HashMap::new())`) after \
         the read at line 19",
        "crates/fees/src/table.rs:33: a miss answered as no entries (`Ok(None) | Err(_) => \
         Ok(BTreeMap::new())`) after the read at line 31",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert_eq!(
        correction.matches("crates/fees/src/table.rs:").count(),
        2,
        "the fetch helper's own empty tail is no finding: {correction}"
    );
    assert_eq!(
        text(&project, "crates/fees/src/table.rs").matches("StateStore::set(provider, ").count(),
        2,
        "both misses fetch and store the table"
    );
    assert!(
        accepted.written.iter().any(|path| path == "crates/fees/src/table.rs"),
        "the module is reported: {:?}",
        accepted.written
    );
}

// A stem is one word, so a crate the turn lays under the head of a
// hyphenated stem of the slice, `crates/order/` holding the rest as a part
// of `order-items`, is refused at the build with the directory named, while
// `common` and a crate the tree held before the turn pass, the head of
// another of the slice's stems among them. The slice's own turn lays the
// stem's crate whole, empties the split one, and puts the stem's lines in
// the root manifest, and the accepted report names what the tree holds.
#[tokio::test]
async fn omnia_build_stem_split() {
    let project = omnia_tree().await;
    project.write(
        "crates/shipment/Cargo.toml",
        "[package]\nname = \"shipment\"\n\n[lints]\nworkspace = true\n",
    );
    project.write("crates/shipment/src/lib.rs", "//! The `shipment` stem.\n");
    let slice = slice_for(
        "order-items",
        &["order-items.add.reject-unknown-sku", "shipment-notes.record.keep-latest"],
    );
    let module = "//! Order items.\n\n/// How many lines an order holds.\n#[must_use]\npub fn \
                  count(items: &[u32]) -> usize {\n    items.len()\n}\n";
    let split = [
        ("crates/order/Cargo.toml", "[package]\nname = \"order\"\n\n[lints]\nworkspace = true\n"),
        ("crates/order/src/lib.rs", "//! The `order` stem.\n\npub mod items;\n"),
        ("crates/order/src/items.rs", module),
    ];
    let whole = [
        (
            "crates/order-items/Cargo.toml",
            "[package]\nname = \"order-items\"\n\n[lints]\nworkspace = true\n",
        ),
        ("crates/order-items/src/lib.rs", "//! The `order-items` stem.\n\npub mod order_items;\n"),
        ("crates/order-items/src/order_items.rs", module),
    ];
    let head_path = "order = { path = \"crates/order\" }\n";
    let head_crate = "order.workspace = true\n";
    let stem_path = "order-items = { path = \"crates/order-items\" }\n";
    let stem_crate = "order-items.workspace = true\n";
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/order/Cargo.toml", "crates/order/src/lib.rs", "crates/order/src/items.rs", "Cargo.toml"]}"#;
    let repaired = r#"{"covered": ["REQ-001"], "written": ["crates/order-items/Cargo.toml", "crates/order-items/src/lib.rs", "crates/order-items/src/order_items.rs", "Cargo.toml"]}"#;
    let model = Scripted::answering([first, repaired])
        .calling(
            0,
            [
                write_files(&split),
                edit_files(&[
                    (
                        "Cargo.toml",
                        OMNIA_PATH_CRATE_ANCHOR,
                        &format!("{OMNIA_PATH_CRATE_ANCHOR}{head_path}"),
                    ),
                    ("Cargo.toml", OMNIA_STEM_ANCHOR, &format!("{OMNIA_STEM_ANCHOR}{head_crate}")),
                ]),
            ],
        )
        .calling(
            1,
            [
                write_files(&whole),
                edit_files(&[
                    ("Cargo.toml", head_path, stem_path),
                    ("Cargo.toml", head_crate, stem_crate),
                ]),
                delete_files(&[
                    "crates/order/Cargo.toml",
                    "crates/order/src/lib.rs",
                    "crates/order/src/items.rs",
                ]),
            ],
        );

    let report =
        OMNIA.build_slice(&project, &model, &slice).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "a stem is one word and is never split at its hyphen",
        "`crates/order/` is laid for `order`, the head of the stem `order-items`, whose crate is \
         `crates/order-items/` (package `order-items`, path `order_items`)",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for named in ["`crates/shipment/`", "`crates/common/`"] {
        assert!(!correction.contains(named), "{named} is no finding: {correction}");
    }
    assert!(
        project.read("crates/order-items/src/order_items.rs").is_some(),
        "the stem's crate holds the module"
    );
    assert!(project.read("crates/order/src/items.rs").is_none(), "the split crate is emptied");
    let manifest = text(&project, "Cargo.toml");
    assert!(
        manifest.contains(stem_path) && manifest.contains(stem_crate),
        "the root manifest names the stem: {manifest}"
    );
    assert!(
        !manifest.contains(head_path) && !manifest.contains(head_crate),
        "the root manifest no longer names the head: {manifest}"
    );
    assert!(
        report.written.iter().any(|path| path == "crates/order-items/src/order_items.rs"),
        "the module is reported: {:?}",
        report.written
    );
    assert!(
        !report.written.iter().any(|path| path.starts_with("crates/order/")),
        "a removed file is not reported: {:?}",
        report.written
    );
}

// A stem whose entry exposes `router()` or `topics(..)` that no statement
// of the root lists merges compiles clean and serves nothing, so the verdict
// is refused with the entry named at the line of each unmerged router,
// whatever the checks answered, a hyphenated stem read as the path it spells
// and a `pub(crate)` router, or a stem exposing neither, being the
// compiler's. The statement added is the repair.
#[tokio::test]
async fn omnia_stem_unmerged() {
    let project = omnia_tree().await;
    project.write("crates/orders/src/lib.rs", OMNIA_ENTRY);
    project.write("crates/orders/src/reading.rs", OMNIA_PART);
    project.write(
        "crates/order-items/src/lib.rs",
        "//! The `order-items` stem.\n\npub mod adding;\n\n/// The stem's routes.\npub fn \
         router<P: common::Capabilities>() -> axum::Router<omnia_sdk::api::Client<P>> {\n    \
         axum::Router::new().merge(adding::routes())\n}\n",
    );
    project.write(
        "crates/billing/src/lib.rs",
        "//! The `billing` stem.\n\nmod charging;\n\npub(crate) fn router<P: \
         common::Capabilities>() -> axum::Router<omnia_sdk::api::Client<P>> {\n    \
         axum::Router::new().merge(charging::routes())\n}\n",
    );
    project.write(
        "crates/auth/src/lib.rs",
        "//! The `auth` stem: the rule every guarded handler calls first.\n\n/// Admit a \
         bearer.\n///\n/// # Errors\n///\n/// `unauthorized` for a bearer the deployment does \
         not know.\npub fn authorise(bearer: &str) -> omnia_sdk::Result<()> {\n    todo!()\n}\n",
    );
    project.write(
        "src/routes.rs",
        omnia_seed("src/routes.rs").replace(
            OMNIA_ROUTE_ANCHOR,
            &format!("{OMNIA_ROUTE_ANCHOR}    let router = router.merge(order_items::router());\n"),
        ),
    );
    project.write(
        "src/messaging.rs",
        omnia_seed("src/messaging.rs")
            .replace(OMNIA_TOPICS_ANCHOR, &format!("{OMNIA_TOPICS_ANCHOR}{OMNIA_TOPICS_LINE}")),
    );
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [edit_files(&[(
            "src/routes.rs",
            OMNIA_ROUTE_ANCHOR,
            &format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}"),
        )])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the merged tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let router_at = line_of(OMNIA_ENTRY, "pub fn router");
    for line in [
        "a stem's `router()` and `topics(..)` are reached through the root lists alone".to_owned(),
        format!(
            "crates/orders/src/lib.rs:{router_at}: `orders::router()` is merged by no statement \
             of `src/routes.rs`"
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["orders::topics", "order_items", "billing", "auth"] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    assert!(
        text(&project, "src/routes.rs").contains(OMNIA_ROUTE_LINE),
        "the route list holds the statement the repair added"
    );
}

// A topic of the shared table no module outside `common` names is published
// and consumed by nothing, so the verdict is refused with the constant named
// at its line, a mention in a comment counting for nothing, while a topic a
// part names passes. Removing the constant is the repair.
#[tokio::test]
async fn omnia_topic_unnamed() {
    let project = omnia_tree().await;
    let returned = "\n/// An order was returned.\npub const ORDER_RETURNED: &str = \
                    \"order-returned\";\n";
    let table = format!("{}{OMNIA_TABLE_LINE}{returned}", omnia_seed("crates/common/src/topic.rs"));
    project.write("crates/common/src/topic.rs", &table);
    project.write("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n");
    project.write(
        "crates/orders/src/reading.rs",
        format!("{OMNIA_PART}\n// A return lands on `topic::ORDER_RETURNED` once stated.\n"),
    );
    let model = Scripted::answering([PASSED, PASSED])
        .calling(1, [edit_files(&[("crates/common/src/topic.rs", returned, "")])]);

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let returned_at = line_of(&table, "pub const ORDER_RETURNED");
    for line in [
        "a topic of the shared table is named by no module".to_owned(),
        format!(
            "crates/common/src/topic.rs:{returned_at}: topic `ORDER_RETURNED` is named by no module"
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    assert!(!correction.contains("ORDER_PLACED"), "`ORDER_PLACED` is no finding: {correction}");
    let table = text(&project, "crates/common/src/topic.rs");
    assert!(table.contains("ORDER_PLACED") && !table.contains("ORDER_RETURNED"), "{table}");
}

// A crate a stem's manifest names that no module or test of the stem spells
// is compiled for nothing, so the verdict is refused with the dependency
// named at its line, while a crate a module uses, one a test alone uses, and
// one of the layout's fixed lines pass, and a mention in a comment counts for
// nothing. Removing the line is the repair.
#[tokio::test]
async fn omnia_dependency_unused() {
    let project = omnia_tree().await;
    let unused = "uuid.workspace = true\n";
    let manifest = format!(
        "[package]\nname = \"orders\"\n\n[lints]\nworkspace = true\n\n[dependencies]\n\
         axum.workspace = true\ncommon.workspace = true\nomnia-sdk.workspace = true\n\
         serde.workspace = true\nserde_json.workspace = true\ntracing.workspace = true\n\
         chrono.workspace = true\nquick-xml = {{ version = \"0.37\", features = \
         [\"serialize\"] }}\nhttp.workspace = true\n{unused}"
    );
    project.write("crates/orders/Cargo.toml", &manifest);
    project.write("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n");
    project.write(
        "crates/orders/src/reading.rs",
        "//! Reads orders.\n\n/// When an order was read.\n#[must_use]\npub fn read_at() -> \
         chrono::DateTime<chrono::Utc> {\n    chrono::Utc::now()\n}\n\n/// An order read from \
         its wire form.\n///\n/// # Errors\n///\n/// `bad_request` for a payload that does not \
         parse.\npub fn parse(payload: &[u8]) -> omnia_sdk::Result<Order> {\n    \
         quick_xml::de::from_reader(payload).map_err(|error| omnia_sdk::bad_request!(\"order: \
         {error}\"))\n}\n\n// An id is the caller's, never a `uuid` minted here.\n/// An \
         order.\n#[derive(serde::Deserialize)]\npub struct Order {\n    /// Its id.\n    pub id: \
         String,\n}\n",
    );
    project.write(
        "crates/orders/tests/reading.rs",
        "//! The `orders` stem's reading.\n\n#[tokio::test]\nasync fn an_order_is_read() {\n    \
         let request = http::Request::builder().body(()).expect(\"a request\");\n    \
         drop(request);\n}\n",
    );
    let model = Scripted::answering([PASSED, PASSED])
        .calling(1, [edit_files(&[("crates/orders/Cargo.toml", unused, "")])]);

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let unused_at = line_of(&manifest, "uuid.workspace");
    for line in [
        "a stem's manifest names the crates its files use and nothing more".to_owned(),
        format!(
            "crates/orders/Cargo.toml:{unused_at}: dependency `uuid` is used by no file of \
             `crates/orders/`"
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["chrono", "quick-xml", "http", "serde_json"] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    let manifest = text(&project, "crates/orders/Cargo.toml");
    assert!(manifest.contains("chrono") && !manifest.contains("uuid"), "{manifest}");
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|line| line.contains(needle))
        .map_or_else(|| panic!("`{needle}` is a line of:\n{text}"), |index| index + 1)
}

// A stem whose modules reach the host through the provider alone passes,
// whatever its tests spell.
#[tokio::test]
async fn omnia_verify_stem_clean() {
    let project = omnia_tree().await;
    project.write(
        "crates/orders/src/reading.rs",
        "//! Reads orders.\n\n/// The key of an order.\n#[must_use]\npub fn key(id: &str) -> String {\n    format!(\"orders/{id}\")\n}\n",
    );
    project.write(
        "crates/orders/tests/reading.rs",
        "#[tokio::test]\nasync fn waits() {\n    tokio::time::sleep(std::time::Duration::from_secs(1)).await;\n}\n",
    );
    let model = Scripted::answering([PASSED]);

    let verdict = OMNIA.verify(&project, &model).await.expect("verify over the clean stem");
    model.assert_exhausted();

    assert!(verdict.passed, "the modules simulate nothing: {:?}", verdict.failures);
}

// A tree holding every file of the seed, as the engine's holds it once the
// adapter has initialised the repository and before any slice builds.
async fn omnia_tree() -> Scratch {
    let project = scratch();
    OMNIA.init(&project).await.expect("init over the empty tree");
    project
}

fn omnia_seed(path: &str) -> String {
    omnia_template::entry(path).unwrap_or_else(|| panic!("the seed lays `{path}`")).body.to_owned()
}

// Every regular file beneath `root`, root-relative, in path order; a link
// is none.
fn files_under(root: &Path) -> BTreeSet<String> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).expect("a readable directory") {
            let path = entry.expect("a readable entry").path();
            let kind = std::fs::symlink_metadata(&path).expect("a readable entry").file_type();
            if kind.is_dir() {
                walk(root, &path, files);
            } else if kind.is_file() {
                let relative = path.strip_prefix(root).expect("beneath the root");
                files.insert(relative.to_str().expect("a UTF-8 path").to_owned());
            }
        }
    }
    let mut files = BTreeSet::new();
    walk(root, root, &mut files);
    files
}

// The seed's root manifest with the `orders` stem's line in each list.
fn omnia_manifest_with_orders() -> String {
    omnia_seed("Cargo.toml").replace(
        OMNIA_PATH_CRATE_ANCHOR,
        &format!("{OMNIA_PATH_CRATE_ANCHOR}{OMNIA_PATH_CRATE_LINE}"),
    ) + OMNIA_STEM_LINE
}
