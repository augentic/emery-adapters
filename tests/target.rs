//! Tests each shipped target adapter natively against a scripted model.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::os::unix::fs::symlink;
use std::path::Path;

use emery_sdk::target::{
    BuildContext, DesignType, Grant, InitContext, MergeStrategy, Outcome, Report, Requirement,
    Scenario, Slice, TargetAdapter, Task, Verdict, VerifyContext,
};
use emery_sdk::{Doc, Error, body};
use omnia_sdk::model::ToolCall;
use omnia_test::SeenWorker;
use omnia_test::guest::Scripted;
use omnia_test::host::{Scratch, scratch};

const PASSED: &str = r#"{"passed": true, "failures": []}"#;

const TYPESCRIPT_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["src/orders/reading/package.json", "src/orders/reading/index.ts", "src/orders/index.ts", "src/index.ts", "test/orders/reading.test.ts"]}"#;
const TYPESCRIPT_FAILED: &str = r#"{"passed": false, "failures": ["check 3 (npm test): 1 failing: an order with no items is refused"]}"#;

// The files the one scripted `write_files` call writes for the TypeScript
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

// The report of the routed slice's turn: the handler and the test the
// adapter scaffolded, filled, and the topic table the turn added a constant to.
const OMNIA_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs", "crates/orders/tests/reading.rs", "crates/common/src/topic.rs"]}"#;
// The marker the adapter scaffolds for the default slice's requirement, which
// states no route, and the body a turn puts in its place.
const OMNIA_FUNCTION_MARKER: &str =
    "    todo!(\"REQ-001 orders.get-id.return-order: the behaviour\")\n";
const OMNIA_FUNCTION_FILLED: &str = "    Ok(())\n";
// The handler and test markers the adapter scaffolds for the routed slice, and
// the bodies a turn puts in their place, the test's `#[ignore]` line gone.
const OMNIA_HANDLER_MARKER: &str =
    "    let _ = (input, context);\n    todo!(\"REQ-001 orders.read: the handler\")\n";
const OMNIA_HANDLER_FILLED: &str = "    let ReadInput { id } = input;\n    let _ = (id, context, \
                                    common::topic::ORDER_PLACED);\n    Ok(ReadOutput {})\n";
const OMNIA_TEST_MARKER: &str = "#[ignore = \"REQ-001 An order is returned by its id: \
                                 unfilled\"]\nasync fn an_order_is_returned_by_its_id() {\n    \
                                 todo!(\"REQ-001 An order is returned by its id: arrange, act, \
                                 assert\")\n}\n";
const OMNIA_TEST_FILLED: &str =
    "async fn an_order_is_returned_by_its_id() {\n    assert_eq!(1 + 1, 2);\n}\n";
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

// The three root files the node target's initialisation scaffolds, as the crate
// embeds them.
const NODE_SEED: [(&str, &str); 3] = [
    (".gitignore", include_str!("../adapters/node/seed/.gitignore")),
    ("package.json", include_str!("../adapters/node/seed/package.json")),
    ("tsconfig.json", include_str!("../adapters/node/seed/tsconfig.json")),
];

// The `orders` stem's manifest as the adapter scaffolds it for the stem's first
// slice: the layout's one text, its `[dependencies]` the six fixed lines.
const OMNIA_MANIFEST: &str = "[package]\nname = \"orders\"\ndescription = \"The `orders` \
                              stem\"\npublish.workspace = true\nedition.workspace = \
                              true\nrust-version.workspace = true\nversion.workspace = \
                              true\n\n[lints]\nworkspace = true\n\n# Target-gated so the guest's \
                              wasm32 dependency graph never sees the doubles.\n[target.'cfg(not(\
                              target_arch = \"wasm32\"))'.dev-dependencies]\nomnia-test = { \
                              workspace = true, features = [\"guest\"] }\ntokio.workspace = \
                              true\ntower.workspace = true\n\n# One line per crate the stem's \
                              files use, each added at the end.\n[dependencies]\naxum.workspace = \
                              true\ncommon.workspace = true\nomnia-sdk.workspace = \
                              true\nserde.workspace = true\nserde_json.workspace = \
                              true\ntracing.workspace = true\n";
// The `orders` stem's entry as the adapter scaffolds it for the routed slice: the
// part's line, then the `router()` the root route list merges, the part's
// routes its one entry, and the `topics(..)` the root topic list registers,
// its array empty since the adapter derives no topic.
const OMNIA_ENTRY: &str = "//! # orders\n//!\n//! The `orders` stem.\n\npub mod reading;\n\nuse \
                           common::Capabilities;\nuse omnia_sdk::api::{Client, messaging};\n\n/// \
                           The stem's routes, merged onto the root router.\npub fn router<P: \
                           Capabilities>() -> axum::Router<Client<P>> {\n    common::routers([\n        \
                           // one entry per part with routes, each added at the end\n        \
                           reading::routes(),\n    ])\n}\n\n/// The stem's topics, registered on \
                           the root messaging router under `env`.\n#[must_use]\npub fn topics<P: \
                           Capabilities>(router: messaging::Router<P>, env: &str) -> \
                           messaging::Router<P> {\n    common::topics(router, env).parts([\n        \
                           // one entry per part with topics, each added at the end\n    ])\n}\n";
// The routed slice's part as the adapter scaffolds it: the module doc from the
// plan's brief, the imports its handler needs, the path constant and the
// `routes()` over it, the owned design type with its signature fenced in
// its doc, the handler's input and output over the route, and the handler
// under its requirement, its body the marker.
const OMNIA_MODULE: &str = "//! Reads an order by its id.\n\nuse common::Capabilities;\nuse \
                            omnia_sdk::api::http::get;\nuse omnia_sdk::api::{Client, \
                            Context};\nuse serde::{Deserialize, Serialize};\n\n// The paths the \
                            part serves.\npub(crate) const ORDERS_ID: &str = \
                            \"/orders/{id}\";\n\npub(crate) fn routes<P: Capabilities>() -> \
                            axum::Router<Client<P>> {\n    axum::Router::new().route(ORDERS_ID, \
                            get(read))\n}\n\n/// The design type `orders.Order`.\n///\n/// \
                            ```text\n/// interface Order {\n///   id: string;\n/// }\n/// \
                            ```\n#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct \
                            Order {}\n\n/// The input of `GET /orders/{id}`.\n#[derive(Debug, \
                            Deserialize)]\npub struct ReadInput {\n    /// The `id` path \
                            parameter.\n    pub id: String,\n}\n\n/// The output of `GET \
                            /orders/{id}`.\n#[derive(Debug, Serialize)]\npub struct ReadOutput \
                            {}\n\n// REQ-001 orders.read: GET /orders/{id} answers the order by \
                            its id.\npub(crate) async fn read<P: Capabilities>(\n    input: \
                            ReadInput, context: Context<P>,\n) -> omnia_sdk::Result<ReadOutput> \
                            {\n    let _ = (input, context);\n    todo!(\"REQ-001 orders.read: \
                            the handler\")\n}\n";
// The routed slice's tests as the adapter scaffolds them: one ignored test per
// scenario, the scenario's steps in a comment above it.
const OMNIA_TESTS: &str = "//! The acceptance scenarios of the `reading` part of the `orders` \
                           stem, over the guest doubles.\n\n// REQ-001 orders.read: An order is \
                           returned by its id\n//   Given an order 42\n//   When GET /orders/42 is \
                           called\n//   Then the order is answered\n#[tokio::test]\n#[ignore = \
                           \"REQ-001 An order is returned by its id: unfilled\"]\nasync fn \
                           an_order_is_returned_by_its_id() {\n    todo!(\"REQ-001 An order is \
                           returned by its id: arrange, act, assert\")\n}\n";
// The default slice's part as the adapter scaffolds it: its requirement states no
// route, so the part is one `pub fn` with a marker and no `routes()`.
const OMNIA_FUNCTION_MODULE: &str = "//! The `reading` part of the `orders` stem.\n\n// REQ-001 \
                                     orders.get-id.return-order: The behaviour \
                                     `orders.get-id.return-order` states.\n/// The behaviour \
                                     `orders.get-id.return-order` states.\n///\n/// # \
                                     Errors\n///\n/// As the requirement states.\npub fn \
                                     get_id_return_order() -> omnia_sdk::Result<()> {\n    \
                                     todo!(\"REQ-001 orders.get-id.return-order: the \
                                     behaviour\")\n}\n";
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
            grants: A::metadata().grants,
            model,
            scaffold: None,
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
            grants: A::metadata().grants,
            model,
        };
        match A::run(Task::Verify(ctx)).await? {
            Outcome::Verified(verdict) => Ok(verdict),
            outcome => panic!("a verify answers a verdict, not {outcome}"),
        }
    }

    // The grant the adapter's `grants` put on a turn, as the script records it.
    fn granted() -> Option<SeenWorker> {
        Grant::union(A::metadata().grants).map(SeenWorker::from)
    }

    // The prompt at `path` as this build compiled it in.
    fn prompt(&self, path: &str) -> &'static str {
        body(self.docs, path).unwrap_or_else(|| panic!("`{path}` is in `{}`'s PROSE", self.name))
    }

    // One initialisation of an empty tree, then one build of `slice` writing
    // `written` through one `write_files` call where there is any, then the
    // `edits` through one `edit_files` call, and answering `report`, then
    // one verify passing, with what every target shares asserted: the init
    // puts no turn; the build turn opens under the embedded `build.md` with
    // the tree lent and both write tools beside the reference tools, its
    // brief carrying the slice; what the model wrote through the tools is
    // what the tree holds and the report names; the verify turn opens under
    // `verify.md` over the same tree, both tools declared for its repairs;
    // each turn carries the adapter's worker grant. Answers the tree, for
    // what the target's own layout must hold.
    async fn built(
        &self, slice: &Slice, written: &[(&str, &str)], edits: &[(&str, &str, &str)], report: &str,
    ) -> Scratch {
        let project = scratch();
        self.init(&project).await.expect("init over the empty tree");
        let mut calls = Vec::new();
        if !written.is_empty() {
            calls.push(write_files(written));
        }
        if !edits.is_empty() {
            calls.push(edit_files(edits));
        }
        let calls_made = calls.len();
        let model = Scripted::answering([report, PASSED]).calling(0, calls);

        let report =
            self.build_slice(&project, &model, slice).await.expect("build over the lent tree");
        let verdict = self.verify(&project, &model).await.expect("verify over the built tree");
        model.assert_exhausted();

        let findings = report.findings(slice);
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
// stating no route, in a slice named for a part of it.
fn slice() -> Slice {
    slice_for("orders-reading", &["orders.get-id.return-order"])
}

// The slice the omnia scaffold is asserted over: one requirement under the
// `orders` stem stating a route in its statement, with one scenario, owning
// one design type, with a one-paragraph brief.
fn omnia_slice() -> Slice {
    let mut slice = slice_for("orders-reading", &["orders.read"]);
    slice.requirements[0].statement =
        vec!["GET /orders/{id} answers the order by its id.".to_owned()];
    slice.requirements[0].scenarios = vec![Scenario {
        name: "An order is returned by its id".to_owned(),
        given: vec!["an order 42".to_owned()],
        when: "GET /orders/42 is called".to_owned(),
        then: "the order is answered".to_owned(),
        and: Vec::new(),
    }];
    slice.types = vec![DesignType {
        key: "orders.Order".to_owned(),
        signature: "interface Order {\n  id: string;\n}".to_owned(),
    }];
    slice.brief = vec!["Reads an order by its id.".to_owned()];
    slice
}

// A slice named `name` over one requirement per subject, `REQ-001` on, each
// stating the behaviour and no route, with no scenario, its specification
// cut rendering each under its heading as the engine does.
fn slice_for(name: &str, subjects: &[&str]) -> Slice {
    let requirements: Vec<Requirement> = subjects
        .iter()
        .enumerate()
        .map(|(index, subject)| Requirement {
            id: format!("REQ-{:03}", index + 1),
            subject: (*subject).to_owned(),
            statement: vec![format!("The behaviour `{subject}` states.")],
            scenarios: Vec::new(),
        })
        .collect();
    let spec = requirements
        .iter()
        .map(|requirement| {
            format!(
                "\n### Requirement: {}\n\nID: {}\n\n{}\n",
                requirement.subject, requirement.id, requirement.statement[0]
            )
        })
        .fold("# Specification\n".to_owned(), |spec, block| spec + &block);
    let ids: Vec<&str> = requirements.iter().map(|requirement| requirement.id.as_str()).collect();
    Slice {
        id: "SLICE-001".to_owned(),
        name: name.to_owned(),
        base: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        spec,
        design: "# Design\n\n## Type: Order\n\nAn id and its items.\n".to_owned(),
        plan: format!(
            "## Slice: {name}\n\nID: SLICE-001\nRequirements: [{}]\nTypes: [Order]\n",
            ids.join(", ")
        ),
        requirements,
        types: Vec::new(),
        brief: Vec::new(),
    }
}

// The edit that fills the function the adapter scaffolded for the default slice.
const fn fill_function() -> (&'static str, &'static str, &'static str) {
    ("crates/orders/src/reading.rs", OMNIA_FUNCTION_MARKER, OMNIA_FUNCTION_FILLED)
}

// One `write_files` call writing `files`, as the model would put it.
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

// The three root files are scaffolded by the initialisation, before the
// slice's turn, and the slice writes its own directory and lines beside them,
// reporting none of the root.
#[tokio::test]
async fn node() {
    let project = NODE.built(&slice(), &TYPESCRIPT_WRITTEN, &[], TYPESCRIPT_REPORT).await;

    for (path, content) in NODE_SEED {
        assert_eq!(
            project.read(path).as_deref(),
            Some(content.as_bytes()),
            "the tree holds `{path}` as the initialisation scaffolded it"
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

// The initialisation scaffolds each root file the tree lacks and nothing over
// what it holds: a held manifest stands as it is, a link at a root path is
// left standing, and a tree initialised once is changed by nothing after.
#[tokio::test]
async fn node_init() {
    let project = scratch();
    project.write("package.json", "{ \"name\": \"held\" }\n");
    symlink("../outside", project.path().join(".gitignore")).expect("planting the link");

    NODE.init(&project).await.expect("init over the held tree");

    assert_eq!(
        project.read("tsconfig.json").as_deref(),
        Some(NODE_SEED[2].1.as_bytes()),
        "the file the tree lacked is scaffolded as the crate embeds it"
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
    let declared: Vec<(&str, MergeStrategy)> = emery_node::Adapter::metadata()
        .rules
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
// Volta, or fnm, and Volta's root named for it. The union is asserted rather
// than the grants, so the adapter may regroup them while any widening shows.
#[test]
fn node_worker() {
    let grants = emery_node::Adapter::metadata().grants;
    assert_eq!(
        Grant::union(grants).map(SeenWorker::from),
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
    assert_eq!(Grant::findings(grants), Vec::<String>::new());
}

#[tokio::test]
async fn node_verify() {
    let project = scratch();
    project.write("package.json", "{}");
    NODE.failed(&project, TYPESCRIPT_FAILED).await;
}

// The seed is scaffolded by the initialisation and the slice's skeleton by
// the adapter, both before the slice's turn, and neither is the turn's: the
// turn fills the handler and the test the adapter scaffolded and adds the
// constant of a topic the adapter does not derive to the seeded table, the
// stem's lines standing in the route list, the topic list, and the root
// manifest's two lists as the adapter added them, every other file of the
// seed as the template spells it.
#[tokio::test]
async fn omnia() {
    let table = omnia_seed("crates/common/src/topic.rs");
    let edits = [
        ("crates/orders/src/reading.rs", OMNIA_HANDLER_MARKER, OMNIA_HANDLER_FILLED),
        ("crates/orders/tests/reading.rs", OMNIA_TEST_MARKER, OMNIA_TEST_FILLED),
        ("crates/common/src/topic.rs", &*table, &*format!("{table}{OMNIA_TABLE_LINE}")),
    ];
    let project = OMNIA.built(&omnia_slice(), &[], &edits, OMNIA_REPORT).await;

    let lists = ["Cargo.toml", "src/routes.rs", "src/messaging.rs", "crates/common/src/topic.rs"];
    for entry in omnia_template::ENTRIES {
        let held = project.read(entry.path);
        if lists.contains(&entry.path) {
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
        text(&project, "crates/orders/Cargo.toml"),
        OMNIA_MANIFEST,
        "the stem's manifest stands as the adapter scaffolded it"
    );
    assert_eq!(
        text(&project, "crates/orders/src/lib.rs"),
        OMNIA_ENTRY,
        "the stem's entry holds the part's line and the two routers the root merges"
    );
    assert_eq!(
        text(&project, "crates/orders/src/reading.rs"),
        OMNIA_MODULE.replace(OMNIA_HANDLER_MARKER, OMNIA_HANDLER_FILLED),
        "the part stands as the adapter scaffolded it with the handler filled"
    );
    assert_eq!(
        text(&project, "crates/orders/tests/reading.rs"),
        OMNIA_TESTS.replace(OMNIA_TEST_MARKER, OMNIA_TEST_FILLED),
        "the tests stand as the adapter scaffolded them with the one test filled and run"
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
// file included, since the initialisation scaffolds what the tree lacks and
// nothing over what it holds; a slice built over it has its stem's lines
// added where the held file holds the list's anchor and nowhere else, the
// brief naming each line the adapter could not place for the turn to add,
// and its report names no file of the seed it did not edit. A held module
// is a comment, since one that does not parse is a finding of the build.
#[tokio::test]
async fn omnia_init_held() {
    let project = scratch();
    let held = |path: &str| {
        if Path::new(path).extension().is_some_and(|ext| ext == "rs") {
            "// held\n"
        } else {
            "# held\n"
        }
    };
    for entry in omnia_template::ENTRIES {
        if entry.path == "Cargo.toml" {
            project.write(entry.path, entry.body.as_bytes());
        } else {
            project.write(entry.path, held(entry.path));
        }
    }

    OMNIA.init(&project).await.expect("init over the held tree");

    for entry in omnia_template::ENTRIES {
        let expected = if entry.path == "Cargo.toml" {
            entry.body.as_bytes()
        } else {
            held(entry.path).as_bytes()
        };
        assert_eq!(
            project.read(entry.path).as_deref(),
            Some(expected),
            "`{}` stands as the tree held it",
            entry.path
        );
    }

    let report = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs", "src/routes.rs", "src/messaging.rs"]}"#;
    let model = Scripted::answering([report]).calling(
        0,
        [edit_files(&[
            fill_function(),
            ("src/routes.rs", "// held\n", &format!("// held\n{OMNIA_ROUTE_LINE}")),
            ("src/messaging.rs", "// held\n", &format!("// held\n{OMNIA_TOPICS_LINE}")),
        ])],
    );
    let report = OMNIA.build(&project, &model).await.expect("build over the held tree");
    model.assert_exhausted();
    assert_eq!(
        report.written,
        ["crates/orders/src/reading.rs", "src/messaging.rs", "src/routes.rs"],
        "the report names the module the turn filled and the lists it added to, no other seed \
         file"
    );
    assert_eq!(text(&project, ".gitignore"), "# held\n", "the build writes no file of the seed");
    assert_eq!(
        text(&project, "src/routes.rs"),
        format!("// held\n{OMNIA_ROUTE_LINE}"),
        "a held list without its anchor takes no line from the adapter, the turn's alone"
    );
    assert_eq!(
        text(&project, "Cargo.toml"),
        omnia_manifest_with_orders(),
        "the held root manifest takes the stem's lines at its anchors"
    );
    let brief = &model.seen()[0].messages[0];
    for carried in [
        "`src/routes.rs` holds no anchor for `let router = router.merge(orders::router());`, so \
         the line was not added",
        "`src/messaging.rs` holds no anchor for `let router = orders::topics(router, &env);`, \
         so the line was not added",
    ] {
        assert!(brief.contains(carried), "the brief carries `{carried}`: {brief}");
    }
}

// The slice's skeleton is the adapter's, scaffolded from the typed slice before
// the turn and with no turn spent on it: the stem's manifest, its entry
// listing the part, the part with the design type, the route, the handler
// over its input and output, and its marker, the test file with one
// ignored test per scenario, and the stem's lines in the seed's lists, each
// byte for byte, the brief naming each file and marker under the SDK's
// heading; a turn that writes nothing covers nothing and reports no file.
#[tokio::test]
async fn omnia_scaffold() {
    let project = omnia_tree().await;
    let before = files_under(project.path());
    let model = Scripted::answering([r#"{"covered": [], "written": []}"#]);

    let report =
        OMNIA.build_slice(&project, &model, &omnia_slice()).await.expect("a turn filling nothing");
    model.assert_exhausted();

    assert!(report.covered.is_empty() && report.written.is_empty(), "nothing is reported");
    let scaffolded: Vec<String> =
        files_under(project.path()).difference(&before).cloned().collect();
    assert_eq!(
        scaffolded,
        [
            "crates/orders/Cargo.toml",
            "crates/orders/src/lib.rs",
            "crates/orders/src/reading.rs",
            "crates/orders/tests/reading.rs",
        ],
        "the adapter scaffolds the stem's four files and nothing else"
    );
    for (path, expected) in [
        ("crates/orders/Cargo.toml", OMNIA_MANIFEST),
        ("crates/orders/src/lib.rs", OMNIA_ENTRY),
        ("crates/orders/src/reading.rs", OMNIA_MODULE),
        ("crates/orders/tests/reading.rs", OMNIA_TESTS),
    ] {
        assert_eq!(
            text(&project, path),
            expected,
            "`{path}` is scaffolded as the layout spells it"
        );
    }
    assert_eq!(text(&project, "Cargo.toml"), omnia_manifest_with_orders());
    assert_eq!(
        text(&project, "src/routes.rs"),
        omnia_seed("src/routes.rs")
            .replace(OMNIA_ROUTE_ANCHOR, &format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}")),
        "the route list merges the stem before the terminal line"
    );
    assert_eq!(
        text(&project, "src/messaging.rs"),
        omnia_seed("src/messaging.rs")
            .replace(OMNIA_TOPICS_ANCHOR, &format!("{OMNIA_TOPICS_ANCHOR}{OMNIA_TOPICS_LINE}")),
        "the topic list registers the stem before its trace"
    );
    assert_eq!(
        text(&project, "crates/common/src/topic.rs"),
        omnia_seed("crates/common/src/topic.rs"),
        "the adapter derives no topic, so the table stands"
    );

    let brief = &model.seen()[0].messages[0];
    let (_, scaffolded) = brief
        .split_once("What the adapter scaffolded for this slice before this turn:\n\n")
        .expect("the brief carries the scaffold section after the design");
    for carried in [
        "Stem `orders`, part `reading`:\n\n- `crates/orders/Cargo.toml`: scaffolded.",
        "- `crates/orders/src/lib.rs`: scaffolded, `pub mod reading;` and `reading::routes(),` in \
         its lists.",
        "- `crates/orders/src/reading.rs`: scaffolded. Fill its markers, listed below.",
        "- `crates/orders/tests/reading.rs`: scaffolded, one ignored test per scenario. Fill each \
         and remove its `#[ignore = ..]` line.",
        "- `Cargo.toml`, `src/routes.rs`, `src/messaging.rs`: the stem's lines added.",
        "Markers, in fill order:\n\n1. `pub struct Order {}` in `crates/orders/src/reading.rs`: \
         the design type `orders.Order`. Declare its fields from the signature in its doc.\n2. \
         `todo!(\"REQ-001 orders.read: the handler\")` in `crates/orders/src/reading.rs`: the \
         handler `read` of `GET /orders/{id}`. Fill `ReadInput` and `ReadOutput` with the wire's \
         fields, write the body, and drop the `let _ = (input, context);` line.\n3. \
         `todo!(\"REQ-001 An order is returned by its id: arrange, act, assert\")` in \
         `crates/orders/tests/reading.rs`: the test `an_order_is_returned_by_its_id`. Fill it \
         and remove the `#[ignore = \"REQ-001 An order is returned by its id: unfilled\"]` line \
         above it.",
    ] {
        assert!(
            scaffolded.contains(carried),
            "the scaffold section carries `{carried}`: {scaffolded}"
        );
    }
    assert!(
        !scaffolded.contains("`start`"),
        "no `start` note for a slice with no `start` id: {scaffolded}"
    );
}

// A stem the tree already holds from an earlier slice keeps its manifest
// and its entry, the part's `pub mod` line and `routes()` entry added to
// the entry's lists, the module and the tests scaffolded fresh beside the
// held part, and the root lists, which already name the stem, left as they
// stand; the brief says which file was held and which scaffolded.
#[tokio::test]
async fn omnia_scaffold_held_stem() {
    let project = omnia_tree().await;
    let first = Scripted::answering([r#"{"covered": [], "written": []}"#]);
    OMNIA.build_slice(&project, &first, &omnia_slice()).await.expect("the first slice's turn");
    first.assert_exhausted();
    let manifest_edited = OMNIA_MANIFEST.replace("The `orders` stem", "The `orders` stem, held");
    project.write("crates/orders/Cargo.toml", manifest_edited.as_bytes());
    let before = files_under(project.path());
    let root_lists =
        ["Cargo.toml", "src/routes.rs", "src/messaging.rs"].map(|path| text(&project, path));

    let mut slice = omnia_slice();
    slice.name = "orders-paying".to_owned();
    slice.requirements[0].subject = "orders.pay".to_owned();
    slice.requirements[0].statement = vec!["POST /orders/{id}/pay settles the order.".to_owned()];
    slice.requirements[0].scenarios[0].name = "An order is paid".to_owned();
    slice.types = Vec::new();
    slice.brief = vec!["Pays an order.".to_owned()];
    let model = Scripted::answering([r#"{"covered": [], "written": []}"#]);
    OMNIA.build_slice(&project, &model, &slice).await.expect("the second slice's turn");
    model.assert_exhausted();

    let scaffolded: Vec<String> =
        files_under(project.path()).difference(&before).cloned().collect();
    assert_eq!(
        scaffolded,
        ["crates/orders/src/paying.rs", "crates/orders/tests/paying.rs"],
        "the second part is scaffolded beside the held one"
    );
    assert_eq!(
        text(&project, "crates/orders/Cargo.toml"),
        manifest_edited,
        "the held manifest stands as the earlier slice left it"
    );
    assert_eq!(
        text(&project, "crates/orders/src/lib.rs"),
        OMNIA_ENTRY.replace("pub mod reading;\n", "pub mod reading;\npub mod paying;\n").replace(
            "        reading::routes(),\n",
            "        reading::routes(),\n        paying::routes(),\n"
        ),
        "the held entry takes the part's two lines at the end of its lists"
    );
    assert_eq!(
        text(&project, "crates/orders/src/reading.rs"),
        OMNIA_MODULE,
        "the earlier part stands"
    );
    let paying = text(&project, "crates/orders/src/paying.rs");
    for carried in [
        "//! Pays an order.\n",
        "pub(crate) const ORDERS_ID_PAY: &str = \"/orders/{id}/pay\";\n",
        "axum::Router::new().route(ORDERS_ID_PAY, post(pay))\n",
        "pub(crate) async fn pay<P: Capabilities>(",
        "    todo!(\"REQ-001 orders.pay: the handler\")\n",
    ] {
        assert!(paying.contains(carried), "the part carries `{carried}`: {paying}");
    }
    assert!(!paying.contains("pub struct Order"), "the type is the first slice's: {paying}");
    for (path, held) in ["Cargo.toml", "src/routes.rs", "src/messaging.rs"].iter().zip(root_lists) {
        assert_eq!(text(&project, path), held, "`{path}` already names the stem and stands");
    }

    let brief = &model.seen()[0].messages[0];
    for carried in [
        "- `crates/orders/Cargo.toml`: held.",
        "- `crates/orders/src/lib.rs`: held, `pub mod paying;` and `paying::routes(),` added to \
         its lists.",
        "- `crates/orders/src/paying.rs`: scaffolded. Fill its markers, listed below.",
        "- `Cargo.toml`, `src/routes.rs`, `src/messaging.rs`: the stem's lines already held.",
    ] {
        assert!(brief.contains(carried), "the brief carries `{carried}`: {brief}");
    }
}

// Two requirements stating one route share one handler, its marker naming
// both, and two under different routes of one path share the path's
// constant; a scenario's test and a handler's name that would collide take
// the requirement's number as a suffix.
#[tokio::test]
async fn omnia_scaffold_twins() {
    let project = omnia_tree().await;
    let mut slice = slice_for(
        "orders-reading",
        &["orders.get-id.return-order", "orders.get-id.refuse-unknown", "orders.delete-id"],
    );
    for (requirement, statement) in slice.requirements.iter_mut().zip([
        "GET /orders/{id} answers the order by its id.",
        "GET /orders/{id} answers `not_found` for an id no order holds.",
        "DELETE /orders/{id} removes the order.",
    ]) {
        requirement.statement = vec![statement.to_owned()];
        requirement.scenarios = vec![Scenario {
            name: "The order is answered".to_owned(),
            given: Vec::new(),
            when: "the route is called".to_owned(),
            then: "it answers".to_owned(),
            and: Vec::new(),
        }];
    }
    let model = Scripted::answering([r#"{"covered": [], "written": []}"#]);

    OMNIA.build_slice(&project, &model, &slice).await.expect("a turn filling nothing");
    model.assert_exhausted();

    let module = text(&project, "crates/orders/src/reading.rs");
    for carried in [
        "pub(crate) const ORDERS_ID: &str = \"/orders/{id}\";\n",
        "    axum::Router::new().route(ORDERS_ID, get(get_id)).route(ORDERS_ID, delete(delete_id))\n",
        "// REQ-001 orders.get-id.return-order: GET /orders/{id} answers the order by its id.\n\
         // REQ-002 orders.get-id.refuse-unknown: GET /orders/{id} answers `not_found` for an id \
         no order holds.\npub(crate) async fn get_id<P: Capabilities>(",
        "    todo!(\"REQ-001 orders.get-id.return-order, REQ-002 orders.get-id.refuse-unknown: \
         the handler\")\n",
        "// REQ-003 orders.delete-id: DELETE /orders/{id} removes the order.\npub(crate) async \
         fn delete_id<P: Capabilities>(",
        "    todo!(\"REQ-003 orders.delete-id: the handler\")\n",
    ] {
        assert!(module.contains(carried), "the part carries `{carried}`: {module}");
    }
    assert_eq!(module.matches("const ORDERS_ID").count(), 1, "one constant per path: {module}");
    let tests = text(&project, "crates/orders/tests/reading.rs");
    for carried in [
        "#[ignore = \"REQ-001 The order is answered: unfilled\"]\nasync fn the_order_is_answered() {",
        "#[ignore = \"REQ-002 The order is answered: unfilled\"]\nasync fn \
         the_order_is_answered_002() {",
        "#[ignore = \"REQ-003 The order is answered: unfilled\"]\nasync fn \
         the_order_is_answered_003() {",
    ] {
        assert!(tests.contains(carried), "the tests carry `{carried}`: {tests}");
    }
}

// A requirement under the `start` stem scaffolds nothing, since the seed and
// the host are `start`: the brief names its id under the `start` note, and
// the slice's other stem is scaffolded as any is.
#[tokio::test]
async fn omnia_scaffold_start() {
    let project = omnia_tree().await;
    let slice = slice_for("start", &["start.refuse-without-secret", "orders.read"]);
    let model = Scripted::answering([r#"{"covered": [], "written": []}"#]);

    OMNIA.build_slice(&project, &model, &slice).await.expect("a turn filling nothing");
    model.assert_exhausted();

    assert!(project.read("crates/start/Cargo.toml").is_none(), "no `start` crate is scaffolded");
    assert!(
        project.read("crates/orders/src/start.rs").is_some(),
        "the other stem's part is named for the slice"
    );
    let brief = &model.seen()[0].messages[0];
    assert!(
        brief.contains(
            "The `start` stem (REQ-001): the seed and the host are `start`, so nothing was \
             scaffolded for it; cover what they satisfy and leave the rest out, as the \
             guardrails' table says."
        ),
        "the brief carries the `start` note: {brief}"
    );
    assert!(!brief.contains("Stem `start`"), "no `start` stem is briefed: {brief}");
}

// A requirement whose statement and scenarios state no route is scaffolded as a
// `pub fn` with a marker, the part declaring no `routes()`, the entry's
// routes array empty, and no test file where no scenario states one.
#[tokio::test]
async fn omnia_scaffold_no_route() {
    let project = omnia_tree().await;
    let model = Scripted::answering([r#"{"covered": [], "written": []}"#]);

    OMNIA.build_slice(&project, &model, &slice()).await.expect("a turn filling nothing");
    model.assert_exhausted();

    assert_eq!(text(&project, "crates/orders/src/reading.rs"), OMNIA_FUNCTION_MODULE);
    assert_eq!(
        text(&project, "crates/orders/src/lib.rs"),
        OMNIA_ENTRY.replace("        reading::routes(),\n", ""),
        "the entry's routes array is empty"
    );
    assert!(project.read("crates/orders/tests/reading.rs").is_none(), "no test file is scaffolded");
    let brief = &model.seen()[0].messages[0];
    for carried in [
        "- `crates/orders/src/lib.rs`: scaffolded, `pub mod reading;` in its lists.",
        "- No scenario states a test, so no test file was scaffolded.",
        "1. `todo!(\"REQ-001 orders.get-id.return-order: the behaviour\")` in \
         `crates/orders/src/reading.rs`: the function `get_id_return_order`, since the \
         requirement states no route. Give it the signature the behaviour needs and write the \
         body.",
    ] {
        assert!(brief.contains(carried), "the brief carries `{carried}`: {brief}");
    }
}

// The route is read from the statement first and from a scenario's `when`
// where the statement spells none, an Express `:id` parameter normalised to
// `{id}`; a method the wire lacks is no route.
#[tokio::test]
async fn omnia_route_from_statement() {
    let project = omnia_tree().await;
    let mut slice = slice_for("orders-items", &["orders.items", "orders.lines", "orders.sum"]);
    slice.requirements[0].statement = vec!["Adding an item appends it to the order.".to_owned()];
    slice.requirements[0].scenarios = vec![Scenario {
        name: "An item is added".to_owned(),
        given: vec!["an order 42".to_owned()],
        when: "POST /orders/:id/items is called with a sku".to_owned(),
        then: "the item is appended".to_owned(),
        and: Vec::new(),
    }];
    slice.requirements[1].statement =
        vec!["PATCH /orders/:orderId/lines/:lineId changes a line's quantity.".to_owned()];
    slice.requirements[2].statement = vec!["FETCH /orders/{id}/sum is no route.".to_owned()];
    let model = Scripted::answering([r#"{"covered": [], "written": []}"#]);

    OMNIA.build_slice(&project, &model, &slice).await.expect("a turn filling nothing");
    model.assert_exhausted();

    let module = text(&project, "crates/orders/src/items.rs");
    for carried in [
        "pub(crate) const ORDERS_ID_ITEMS: &str = \"/orders/{id}/items\";\n",
        "pub(crate) const ORDERS_ORDER_ID_LINES_LINE_ID: &str = \
         \"/orders/{orderId}/lines/{lineId}\";\n",
        ".route(ORDERS_ID_ITEMS, post(items))",
        ".route(ORDERS_ORDER_ID_LINES_LINE_ID, patch(lines))",
        "    /// The `orderId` path parameter.\n    #[serde(rename = \"orderId\")]\n    pub \
         order_id: String,\n",
        "pub fn sum() -> omnia_sdk::Result<()> {\n    todo!(\"REQ-003 orders.sum: the \
         behaviour\")\n}\n",
    ] {
        assert!(module.contains(carried), "the part carries `{carried}`: {module}");
    }
    assert!(!module.contains("ORDERS_ID_SUM"), "a method the wire lacks is no route: {module}");
}

// A turn that covers a requirement whose marker still stands is refused
// with the marker's line named, fills it, and is accepted; a requirement
// left out of `covered` keeps its marker unrefused.
#[tokio::test]
async fn omnia_marker_left() {
    let project = omnia_tree().await;
    let slice = slice_for("orders-reading", &["orders.get-id.return-order", "orders.count"]);
    let first = r#"{"covered": ["REQ-001", "REQ-002"], "written": []}"#;
    let repaired = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs"]}"#;
    let model = Scripted::answering([first, repaired]).calling(1, [edit_files(&[fill_function()])]);

    let report =
        OMNIA.build_slice(&project, &model, &slice).await.expect("the filled tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "a covered requirement is implemented, so no marker of its stands",
        "crates/orders/src/reading.rs:10: REQ-001 is covered and its marker stands",
        "crates/orders/src/reading.rs:20: REQ-002 is covered and its marker stands",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert_eq!(report.covered, ["REQ-001"], "the requirement left out keeps its marker");
    assert!(
        text(&project, "crates/orders/src/reading.rs").contains("todo!(\"REQ-002 orders.count"),
        "the uncovered requirement's marker stands"
    );
}

// A turn that covers a requirement whose scenario's test is still ignored,
// or still holds its marker, is refused with the test named, fills it and
// drops the `#[ignore]` line, and is accepted.
#[tokio::test]
async fn omnia_scenario_ignored() {
    let project = omnia_tree().await;
    let half_filled = "#[ignore = \"REQ-001 An order is returned by its id: unfilled\"]\nasync fn \
                       an_order_is_returned_by_its_id() {\n    assert_eq!(1 + 1, 2);\n}\n";
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs"]}"#;
    let repaired = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs", "crates/orders/tests/reading.rs"]}"#;
    let model = Scripted::answering([first, first, repaired])
        .calling(
            0,
            [edit_files(&[(
                "crates/orders/src/reading.rs",
                OMNIA_HANDLER_MARKER,
                OMNIA_HANDLER_FILLED,
            )])],
        )
        .calling(
            1,
            [edit_files(&[("crates/orders/tests/reading.rs", OMNIA_TEST_MARKER, half_filled)])],
        )
        .calling(
            2,
            [edit_files(&[("crates/orders/tests/reading.rs", half_filled, OMNIA_TEST_FILLED)])],
        );

    let report = OMNIA
        .build_slice(&project, &model, &omnia_slice())
        .await
        .expect("the filled tree is accepted");
    model.assert_exhausted();

    let refused: Vec<String> = model
        .exchanges()
        .into_iter()
        .filter(|exchange| exchange.tool == "check")
        .filter_map(|exchange| exchange.outcome.err())
        .collect();
    let [scaffolded, half] = refused.as_slice() else {
        panic!("two candidates are refused: {refused:?}");
    };
    let ignored = "crates/orders/tests/reading.rs:9: REQ-001 is covered and the test \
                   `an_order_is_returned_by_its_id` of its scenario `An order is returned by its \
                   id` is ignored";
    let marker = "crates/orders/tests/reading.rs:10: REQ-001 is covered and the test \
                  `an_order_is_returned_by_its_id` of its scenario `An order is returned by its \
                  id` holds its marker";
    for correction in [scaffolded, half] {
        assert!(
            correction.contains("a covered requirement's scenarios are tests that run and pass")
                && correction.contains(ignored),
            "the correction carries `{ignored}`: {correction}"
        );
    }
    assert!(scaffolded.contains(marker), "the scaffolded test holds its marker: {scaffolded}");
    assert!(!half.contains("holds its marker"), "the half-filled test has none: {half}");
    assert_eq!(report.covered, ["REQ-001"]);
    assert_eq!(
        text(&project, "crates/orders/tests/reading.rs"),
        OMNIA_TESTS.replace(OMNIA_TEST_MARKER, OMNIA_TEST_FILLED),
        "the test runs"
    );
}

// The initialisation scaffolds every file of the layout outside `crates/<stem>/`
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
            "`{path}` is scaffolded as the template spells it"
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
    let declared: Vec<(&str, MergeStrategy)> = emery_omnia::Adapter::metadata()
        .rules
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

// The rules follow the template: every file the template scaffolds as a list is
// union-merged, every union rule over one path is such a file, and the rules
// over a glob are the two files of a stem's crate that hold its lists, so a
// list the template adds or drops moves the rules with it.
#[test]
fn omnia_rules_follow_template() {
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

// What a build's or verify's worker reaches beyond the tree, as the grant the
// turns carry: the crates.io index and its downloads, the toolchain and the
// binaries beneath the home, never `~/.cargo` whole, and `RUSTUP_HOME` named
// for it. The union is asserted rather than the grants, so the adapter may
// regroup them while any widening shows.
#[test]
fn omnia_worker() {
    let grants = emery_omnia::Adapter::metadata().grants;
    assert_eq!(
        Grant::union(grants).map(SeenWorker::from),
        Some(SeenWorker {
            egress: vec!["index.crates.io".to_owned(), "static.crates.io".to_owned()],
            read: vec!["~/.rustup".to_owned(), "~/.cargo/bin".to_owned()],
            env: vec![("RUSTUP_HOME".to_owned(), "~/.rustup".to_owned())],
        }),
        "the worker grant the adapter's turns carry"
    );
    assert_eq!(Grant::findings(grants), Vec::<String>::new());
}

#[tokio::test]
async fn omnia_verify() {
    OMNIA.failed(&omnia_tree().await, OMNIA_FAILED).await;
}

// A verify whose repair changed a static file, over a tree that lacks
// another, is refused with each named: the lines to put back for the one,
// the template's body whole for the other, since the turn has no other way
// to write it. The repair puts both back and the verdict is accepted over the
// tree as the template spells it.
#[tokio::test]
async fn omnia_static_drift_refused() {
    let project = omnia_tree().await;
    std::fs::remove_file(project.path().join("examples/runtime.rs"))
        .expect("the seed scaffolded it");
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
// adapter, not by the merge: a slice that edits a pin beside the stem's
// lines the adapter added is refused with the pin's lines named, puts it
// back, and its accepted report carries the manifest with the stem's lines
// kept.
#[tokio::test]
async fn omnia_manifest_refused() {
    let project = omnia_tree().await;
    let report =
        r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs", "Cargo.toml"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [edit_files(&[fill_function(), ("Cargo.toml", OMNIA_PIN, OMNIA_PIN_EDITED)])])
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
        "`crates/start/` is written for `start`, which the seed and the host are and no crate of \
         the tree ever is",
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
    let written = [(
        "crates/orders/src/runner.rs",
        "//! Runs the process (`fn start` in `struct App`).\n\n/// Run it.\npub fn run() {\n    \
         loop {}\n}\n",
    )];
    let listed =
        ("crates/orders/src/lib.rs", "pub mod reading;\n", "pub mod reading;\npub mod runner;\n");
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/src/runner.rs"]}"#;
    let repaired = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/lib.rs", "crates/orders/src/reading.rs"]}"#;
    let model = Scripted::answering([first, repaired])
        .calling(0, [write_files(&written), edit_files(&[fill_function(), listed])])
        .calling(
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

// A stem's manifest that does not inherit the workspace's lints, as one the
// turn edited from the `[lints]` table the adapter scaffolded to a
// `lints.workspace = true` key under `[package]`, is refused at the build
// with the manifest named, since the lints are the host boundary and reach a
// crate through `[lints]` alone. The turn puts the `[lints]` table back and
// the report is accepted.
#[tokio::test]
async fn omnia_stem_without_workspace_lints() {
    let project = omnia_tree().await;
    let linted = "version.workspace = true\n\n[lints]\nworkspace = true\n";
    let unlinted = "version.workspace = true\nlints.workspace = true\n";
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/reading.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(
            0,
            [edit_files(&[fill_function(), ("crates/orders/Cargo.toml", linted, unlinted)])],
        )
        .calling(1, [edit_files(&[("crates/orders/Cargo.toml", unlinted, linted)])]);

    OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in
        ["a stem's manifest carries `[lints]` with `workspace = true`", "crates/orders/Cargo.toml"]
    {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert_eq!(
        text(&project, "crates/orders/Cargo.toml"),
        OMNIA_MANIFEST,
        "the manifest inherits the lints as the adapter scaffolded it"
    );
}

// Writing `crates/start/` (or a `crates/start-*` crate for a slice cut from
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
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs", "crates/start/Cargo.toml", "crates/start/src/lib.rs", "crates/start/src/start.rs", "crates/start-prefetch/src/lib.rs", "Cargo.toml"]}"#;
    let repaired = r#"{"covered": ["REQ-001"], "written": ["crates/orders/src/reading.rs"]}"#;
    let model = Scripted::answering([first, repaired])
        .calling(
            0,
            [
                write_files(&written),
                edit_files(&[
                    fill_function(),
                    (
                        "Cargo.toml",
                        OMNIA_PATH_CRATE_ANCHOR,
                        &format!("{OMNIA_PATH_CRATE_ANCHOR}{start_path}"),
                    ),
                    ("Cargo.toml", OMNIA_STEM_LINE, &format!("{OMNIA_STEM_LINE}{start_crate}")),
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
        "`crates/start/` is written for `start`, which the seed and the host are and no crate of \
         the tree ever is",
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
    let written = [("crates/stops/src/fetching.rs", first_tree.as_str())];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/stops/src/fetching.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [write_files(&written)])
        .calling(1, [edit_files(&[("crates/stops/src/fetching.rs", &format!("{wrapped}\n"), "")])]);

    let slice = slice_for("stops-fetching", &["stops.fetch"]);
    let accepted =
        OMNIA.build_slice(&project, &model, &slice).await.expect("the repaired tree is accepted");
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
// own turn writes the fetch into the read, and the accepted report names the
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
    let written = [("crates/rates/src/table.rs", first_tree.as_str())];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/rates/src/table.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [write_files(&written)])
        .calling(1, [edit_files(&[("crates/rates/src/table.rs", empty, fetched)])]);

    let slice = slice_for("rates-table", &["rates.table"]);
    let accepted =
        OMNIA.build_slice(&project, &model, &slice).await.expect("the repaired tree is accepted");
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
// async fn`, which `rustfmt` writes with no space after `pub` — is held to
// the function it sits in as a public one is, so a `None` arm answered
// empty seven lines below the read is named rather than passed over. The
// slice's own turn writes the fetch into the read, and the accepted report
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
    let written = [("crates/zones/src/table.rs", first_tree.as_str())];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/zones/src/table.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [write_files(&written)])
        .calling(1, [edit_files(&[("crates/zones/src/table.rs", empty, fetched)])]);

    let slice = slice_for("zones-table", &["zones.table"]);
    let accepted =
        OMNIA.build_slice(&project, &model, &slice).await.expect("the repaired tree is accepted");
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
// is none. The slice's own turn writes the fetch into both reads, and the
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
    let written = [("crates/fees/src/table.rs", first_tree.as_str())];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/fees/src/table.rs"]}"#;
    let model = Scripted::answering([report, report]).calling(0, [write_files(&written)]).calling(
        1,
        [edit_files(&[
            ("crates/fees/src/table.rs", tail, fetched),
            ("crates/fees/src/table.rs", arm, loaded),
        ])],
    );

    let slice = slice_for("fees-table", &["fees.table"]);
    let accepted =
        OMNIA.build_slice(&project, &model, &slice).await.expect("the repaired tree is accepted");
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

// A stem is one word, so a crate the turn writes under the head of a
// hyphenated stem of the slice, `crates/order/` holding the rest as a part
// of `order-items`, is refused at the build with the directory named beside
// the stem's crate the adapter scaffolded, while `common`, the slice's own stems,
// and a crate the tree held before the turn pass, the head of another of
// the slice's stems among them. The slice's own turn empties the split
// crate and drops its lines from the root manifest, and the accepted report
// names what the tree holds.
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
    let head_path = "order = { path = \"crates/order\" }\n";
    let head_crate = "order.workspace = true\n";
    let stem_path = "order-items = { path = \"crates/order-items\" }\n";
    let stem_crate = "order-items.workspace = true\n";
    let first = r#"{"covered": [], "written": ["crates/order/Cargo.toml", "crates/order/src/lib.rs", "crates/order/src/items.rs", "Cargo.toml"]}"#;
    let repaired = r#"{"covered": [], "written": ["Cargo.toml"]}"#;
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
                    ("Cargo.toml", stem_crate, &format!("{stem_crate}{head_crate}")),
                ]),
            ],
        )
        .calling(
            1,
            [
                edit_files(&[("Cargo.toml", head_path, ""), ("Cargo.toml", head_crate, "")]),
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
        "`crates/order/` is written for `order`, the head of the stem `order-items`, whose crate \
         is `crates/order-items/` (package `order-items`, path `order_items`)",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for named in ["`crates/shipment/`", "`crates/common/`", "`crates/shipment-notes/`"] {
        assert!(!correction.contains(named), "{named} is no finding: {correction}");
    }
    assert_eq!(correction.matches("is written for").count(), 1, "one finding: {correction}");
    for scaffolded in
        ["crates/order-items/src/order_items.rs", "crates/shipment-notes/src/order_items.rs"]
    {
        assert!(
            project.read(scaffolded).is_some(),
            "the adapter scaffolded `{scaffolded}` for the slice's stem"
        );
    }
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
    assert_eq!(report.written, ["Cargo.toml"], "a removed file is not reported");
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

// A part that publishes in a stem whose entry registers no part, and whose
// crate no other stem calls, is never run, so the verdict is refused with
// the part named at its first `Publish` in code, a mention in its doc
// counting for nothing, while a publishing stem whose entry registers a
// part and one another stem's module reaches pass. The repair writes the
// trigger: the topic's constant, the `.route(..)` in the part's
// `topics(..)`, and the part's entry in the stem's array.
#[tokio::test]
async fn omnia_publisher_unreached() {
    let project = omnia_tree().await;
    let charged =
        "\n/// A charge was taken.\npub const BILLING_CHARGED: &str = \"billing-charged\";\n";
    project.write(
        "crates/common/src/topic.rs",
        format!("{}{charged}", omnia_seed("crates/common/src/topic.rs")),
    );
    let topics_anchor = "        // one entry per part with topics, each added at the end\n";
    let entry = OMNIA_ENTRY
        .replace("pub mod reading;", "pub mod publishing;")
        .replace("        reading::routes(),\n", "")
        .replace("        reading::topics,\n", "");
    let handler = "pub(crate) async fn on_placed<P: Config + Publish>(\n    input: Order, context: \
                   Context<P>,\n) -> omnia_sdk::Result<()> {\n    todo!()\n}\n";
    let part = format!(
        "//! Publishes placed orders.\n\nuse common::{{Capabilities, config, topic}};\nuse \
         omnia_sdk::api::{{Context, messaging}};\nuse omnia_sdk::{{Config, Publish}};\n\n/// An \
         order as the wire carries it.\n#[derive(serde::Deserialize)]\npub struct Order {{\n    \
         /// Its id.\n    pub id: String,\n}}\n\n{handler}"
    );
    project.write("crates/orders/src/lib.rs", &entry);
    project.write("crates/orders/src/publishing.rs", &part);
    reached_publishers(&project);
    let route = "pub(crate) fn topics<P: Capabilities>(\n    router: messaging::Router<P>, env: \
                 &str,\n) -> messaging::Router<P> {\n    router.route(config::topic_for(env, \
                 topic::ORDER_PLACED), messaging::consume(on_placed))\n}\n\n";
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [edit_files(&[
            ("crates/common/src/topic.rs", charged, &format!("{charged}{OMNIA_TABLE_LINE}")),
            ("crates/orders/src/publishing.rs", handler, &format!("{route}{handler}")),
            (
                "crates/orders/src/lib.rs",
                topics_anchor,
                &format!("{topics_anchor}        publishing::topics,\n"),
            ),
        ])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let publish_at = line_of(&part, "use omnia_sdk::{Config, Publish};");
    for line in [
        "the host runs a stem through its `routes()` and `topics(..)` alone".to_owned(),
        format!(
            "crates/orders/src/publishing.rs:{publish_at}: `Publish` in a stem no route or topic \
             reaches"
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["billing", "audit"] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    assert!(
        text(&project, "crates/orders/src/lib.rs").contains("publishing::topics,"),
        "the entry's array holds the part the repair added"
    );
    assert!(
        text(&project, "crates/orders/src/publishing.rs").contains("topic::ORDER_PLACED"),
        "the part consumes the topic the repair wrote"
    );
}

// Two publishing stems the check credits as reached: `billing`, whose entry
// registers the part that publishes and which the root lists name, and
// `audit`, which no entry registers and `billing`'s handler calls.
fn reached_publishers(project: &Scratch) {
    project.write(
        "crates/billing/src/lib.rs",
        OMNIA_ENTRY.replace("reading", "charging").replace("orders", "billing"),
    );
    project.write(
        "crates/billing/src/charging.rs",
        "//! Takes charges.\n\nuse audit::recording::record;\nuse common::{Capabilities, config, \
         topic};\nuse omnia_sdk::api::{Client, Context, messaging};\nuse omnia_sdk::Publish;\n\n\
         pub(crate) fn routes<P: Capabilities>() -> axum::Router<Client<P>> {\n    \
         axum::Router::new()\n}\n\npub(crate) fn topics<P: Capabilities>(\n    router: \
         messaging::Router<P>, env: &str,\n) -> messaging::Router<P> {\n    \
         router.route(config::topic_for(env, topic::BILLING_CHARGED), \
         messaging::consume(on_charged))\n}\n\npub(crate) async fn on_charged<P: Publish>(input: \
         String, context: Context<P>) -> omnia_sdk::Result<()> {\n    record(context.provider(), \
         &input).await\n}\n",
    );
    project.write("crates/audit/src/lib.rs", "//! # audit\n\npub mod recording;\n");
    project.write(
        "crates/audit/src/recording.rs",
        "//! Records what the stems did.\n\nuse omnia_sdk::Publish;\n\n/// Record one \
         action.\n///\n/// # Errors\n///\n/// `bad_gateway` when the stream refuses.\npub async \
         fn record<P: Publish>(provider: &P, action: &str) -> omnia_sdk::Result<()> {\n    \
         todo!()\n}\n",
    );
    project.write(
        "src/routes.rs",
        omnia_seed("src/routes.rs").replace(
            OMNIA_ROUTE_ANCHOR,
            &format!(
                "{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}    let router = \
                 router.merge(billing::router());\n"
            ),
        ),
    );
    project.write(
        "src/messaging.rs",
        omnia_seed("src/messaging.rs").replace(
            OMNIA_TOPICS_ANCHOR,
            &format!(
                "{OMNIA_TOPICS_ANCHOR}{OMNIA_TOPICS_LINE}    let router = billing::topics(router, \
                 &env);\n"
            ),
        ),
    );
}

// A refusal answered `Ok(())` — in a `let Ok(..) = .. else` block, an
// `Err(..) =>` arm, or an `is_err()` branch — is a delivery or a call lost
// as processed, so the verdict is refused with each answer named at its
// line, while a `let Some(..) = .. else` answering an empty list and an
// `Err(..)` arm returning the error pass. Returning the error is the
// repair.
#[tokio::test]
async fn omnia_rejection_swallowed() {
    let project = omnia_tree().await;
    let invalid = "        tracing::warn!(\"invalid order\");\n        return Ok(());\n";
    let unstored = "    if remember(provider, &order).await.is_err() {\n        return \
                    Ok(());\n    }\n";
    let unsent = "        Err(_) => Ok(()),\n";
    let module = format!(
        "//! Reads orders.\n\nuse omnia_sdk::api::Context;\nuse omnia_sdk::{{HttpRequest, \
         StateStore, bad_request}};\n\n/// An order as the wire carries \
         it.\n#[derive(serde::Deserialize)]\npub struct Order {{\n    /// Its id.\n    pub id: \
         String,\n}}\n\npub(crate) async fn on_placed<P: HttpRequest + StateStore>(\n    input: \
         Order, context: Context<P>,\n) -> omnia_sdk::Result<()> {{\n    let provider = \
         context.provider();\n    let Ok(order) = validate(&input) else {{\n{invalid}    \
         }};\n{unstored}    match notify(provider, &order).await {{\n        Ok(()) => \
         Ok(()),\n{unsent}    }}\n}}\n\npub(crate) async fn lines<P: StateStore>(provider: &P, \
         id: &str) -> omnia_sdk::Result<Vec<String>> {{\n    let Some(bytes) = \
         StateStore::get(provider, &format!(\"orders/{{id}}/lines\")).await? else {{\n        \
         return Ok(vec![]);\n    }};\n    match serde_json::from_slice(&bytes) {{\n        \
         Ok(lines) => Ok(lines),\n        Err(error) => return Err(bad_request!(\"lines: \
         {{error}}\")),\n    }}\n}}\n"
    );
    project.write("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n");
    project.write("crates/orders/src/reading.rs", &module);
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [edit_files(&[
            (
                "crates/orders/src/reading.rs",
                invalid,
                "        return Err(bad_request!(\"invalid order\"));\n",
            ),
            ("crates/orders/src/reading.rs", unstored, "    remember(provider, &order).await?;\n"),
            ("crates/orders/src/reading.rs", unsent, "        Err(error) => Err(error),\n"),
        ])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let let_else_at = line_of(&module, "let Ok(order)");
    let is_err_at = line_of(&module, ".is_err() {");
    for line in [
        "a delivery or a call the code refused is answered `Ok(())`, as processed".to_owned(),
        format!(
            "crates/orders/src/reading.rs:{}: `Ok(())` answers the refusal the `let Ok(..) = .. \
             else` block at line {let_else_at} caught",
            line_of(&module, "tracing::warn!") + 1
        ),
        format!(
            "crates/orders/src/reading.rs:{}: `Ok(())` answers the refusal the `is_err()` branch \
             at line {is_err_at} caught",
            is_err_at + 1
        ),
        format!(
            "crates/orders/src/reading.rs:{}: `Ok(())` answers the refusal the `Err(..) =>` arm \
             caught",
            line_of(&module, "Err(_) => Ok(())")
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    assert_eq!(
        correction.matches("crates/orders/src/reading.rs:").count(),
        3,
        "an empty list answered where the store holds none and an arm returning its error are \
         no finding: {correction}"
    );
    let repaired = text(&project, "crates/orders/src/reading.rs");
    assert!(
        repaired.contains("Err(bad_request!(\"invalid order\"))")
            && repaired.contains("Err(error) => Err(error)")
            && !repaired.contains("Err(_) => Ok(())"),
        "{repaired}"
    );
}

// A refusal caught and answered `Ok(())` on one line — the `let Ok(..) = ..
// else { return Ok(()) };` the translation reference names, an `is_err()`
// branch, and an `if let Err(..)` branch closed on their own line — is the
// loss the block shapes are, so each is named at its line, while a one-line
// `let Ok(()) = .. else` returning the error passes. Propagating the error
// is the repair.
#[tokio::test]
async fn omnia_rejection_swallowed_on_one_line() {
    let project = omnia_tree().await;
    let invalid = "    let Ok(order) = validate(&input) else { return Ok(()) };\n";
    let unstored = "    if remember(provider, &order).await.is_err() { return Ok(()); }\n";
    let unsent = "    if let Err(error) = notify(provider, &order).await { \
                  tracing::warn!(%error); return Ok(()); }\n";
    let module = format!(
        "//! Reads orders.\n\nuse omnia_sdk::api::Context;\nuse omnia_sdk::{{HttpRequest, \
         StateStore, bad_request}};\n\n/// An order as the wire carries \
         it.\n#[derive(serde::Deserialize)]\npub struct Order {{\n    /// Its id.\n    pub id: \
         String,\n}}\n\npub(crate) async fn on_placed<P: HttpRequest + StateStore>(\n    input: \
         Order, context: Context<P>,\n) -> omnia_sdk::Result<()> {{\n    let provider = \
         context.provider();\n{invalid}{unstored}{unsent}    let Ok(()) = audit(provider, \
         &order).await else {{ return Err(bad_request!(\"unaudited\")) }};\n    Ok(())\n}}\n"
    );
    project.write("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n");
    project.write("crates/orders/src/reading.rs", &module);
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [edit_files(&[
            ("crates/orders/src/reading.rs", invalid, "    let order = validate(&input)?;\n"),
            ("crates/orders/src/reading.rs", unstored, "    remember(provider, &order).await?;\n"),
            ("crates/orders/src/reading.rs", unsent, "    notify(provider, &order).await?;\n"),
        ])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    for line in [
        "a delivery or a call the code refused is answered `Ok(())`, as processed".to_owned(),
        format!(
            "crates/orders/src/reading.rs:{}: `Ok(())` answers the refusal the `let Ok(..) = .. \
             else` block caught",
            line_of(&module, "let Ok(order)")
        ),
        format!(
            "crates/orders/src/reading.rs:{}: `Ok(())` answers the refusal the `is_err()` branch \
             caught",
            line_of(&module, ".is_err() {")
        ),
        format!(
            "crates/orders/src/reading.rs:{}: `Ok(())` answers the refusal the `if let Err(..)` \
             branch caught",
            line_of(&module, "if let Err(error)")
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    assert_eq!(
        correction.matches("crates/orders/src/reading.rs:").count(),
        3,
        "a one-line `let Ok(()) = .. else` returning the error and the handler's own tail are \
         no finding: {correction}"
    );
    let repaired = text(&project, "crates/orders/src/reading.rs");
    assert!(
        repaired.contains("let order = validate(&input)?;")
            && repaired.contains("notify(provider, &order).await?;")
            && !repaired.contains("return Ok(())"),
        "{repaired}"
    );
}

// A setting whose name spells `FORMAT` reads a parser's syntax from the
// deployment, so the verdict is refused with the setting named at its
// line, while the settings beside it and a format constant holding the
// syntax itself pass. Holding the syntax in the constant is the repair.
#[tokio::test]
async fn omnia_format_setting() {
    let project = omnia_tree().await;
    let setting = "pub(crate) const DATE_FORMAT: &str = \"DATE_FORMAT\";\n";
    let module = format!(
        "//! Reads orders.\n\npub(crate) const STATIONS: &str = \"STATIONS\";\npub(crate) const \
         BLOCK_MGT_URL: &str = \"BLOCK_MGT_URL\";\npub(crate) const TIMEZONE: &str = \
         \"TIMEZONE\";\n{setting}pub(crate) const ISSUE_FORMAT: &str = \"%d/%m/%Y\";\n"
    );
    project.write("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n");
    project.write("crates/orders/src/reading.rs", &module);
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [edit_files(&[(
            "crates/orders/src/reading.rs",
            setting,
            "pub(crate) const DATE_FORMAT: &str = \"%d/%m/%Y\";\n",
        )])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let setting_at = line_of(&module, "DATE_FORMAT");
    for line in [
        "a format is the parser's syntax and never a deployment value".to_owned(),
        format!("crates/orders/src/reading.rs:{setting_at}: setting `DATE_FORMAT` names a format"),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["STATIONS", "BLOCK_MGT_URL", "TIMEZONE", "ISSUE_FORMAT"] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    let repaired = text(&project, "crates/orders/src/reading.rs");
    assert!(
        repaired.contains("\"%d/%m/%Y\"") && !repaired.contains("\"DATE_FORMAT\""),
        "{repaired}"
    );
}

// A module that does not parse is refused with its path and the line the
// parser stopped at, read by no other rule, since the compiler refuses it
// too; repairing its syntax is the repair, and the repaired tree is
// accepted.
#[tokio::test]
async fn omnia_module_unparsed() {
    let project = omnia_tree().await;
    let unterminated = "    let key = format!(\"orders/{id}\")\n";
    let module = format!(
        "//! Reads orders.\n\n/// The key of an order.\n#[must_use]\npub fn key(id: &str) -> \
         String {{\n{unterminated}    key\n}}\n\npub fn run() {{}}\n"
    );
    project.write("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n");
    project.write("crates/orders/src/reading.rs", &module);
    let model = Scripted::answering([PASSED, PASSED, PASSED])
        .calling(
            1,
            [edit_files(&[(
                "crates/orders/src/reading.rs",
                unterminated,
                "    let key = format!(\"orders/{id}\");\n",
            )])],
        )
        .calling(2, [edit_files(&[("crates/orders/src/reading.rs", "\npub fn run() {}\n", "")])]);

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let corrections: Vec<String> = model
        .exchanges()
        .into_iter()
        .filter(|exchange| exchange.tool == "check")
        .filter_map(|exchange| exchange.outcome.err())
        .collect();
    let [unparsed, lifecycle] = corrections.as_slice() else {
        panic!("two checks are refused: {corrections:?}");
    };
    for line in [
        "a module that does not parse is read by no check and refused by the compiler; repair \
         its syntax through `edit_files`:"
            .to_owned(),
        format!("crates/orders/src/reading.rs:{}: ", line_of(&module, "    key")),
    ] {
        assert!(unparsed.contains(&line), "the correction carries `{line}`: {unparsed}");
    }
    assert!(
        !unparsed.contains("a run of its own"),
        "a module that does not parse is read by no other rule: {unparsed}"
    );
    assert!(
        lifecycle.contains(&format!(
            "crates/orders/src/reading.rs:{}: a run of its own (`fn run`)",
            line_of(&module, "pub fn run")
        )),
        "the repaired module is read by every rule: {lifecycle}"
    );
    assert!(
        text(&project, "crates/orders/src/reading.rs").contains("format!(\"orders/{id}\");"),
        "the syntax is repaired"
    );
}

// A refusal caught by a `let Ok(..) = .. else` whose `else` opens on a
// line of its own, as a long initialiser is formatted, is named at the
// `Ok(())` with the `let` line beside it, as one opened on the `let` line
// is. Propagating the error is the repair.
#[tokio::test]
async fn omnia_rejection_swallowed_across_lines() {
    let project = omnia_tree().await;
    let invalid = "    let Ok(order) = validate(&input)\n        .await\n    else {\n        return \
                   Ok(());\n    };\n";
    let module = format!(
        "//! Reads orders.\n\nuse omnia_sdk::api::Context;\nuse omnia_sdk::{{HttpRequest, \
         StateStore}};\n\n/// An order as the wire carries it.\n#[derive(serde::Deserialize)]\npub \
         struct Order {{\n    /// Its id.\n    pub id: String,\n}}\n\npub(crate) async fn \
         on_placed<P: HttpRequest + StateStore>(\n    input: Order, context: Context<P>,\n) -> \
         omnia_sdk::Result<()> {{\n    let provider = context.provider();\n{invalid}    \
         remember(provider, &order).await?;\n    Ok(())\n}}\n"
    );
    project.write("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n");
    project.write("crates/orders/src/reading.rs", &module);
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [edit_files(&[(
            "crates/orders/src/reading.rs",
            invalid,
            "    let order = validate(&input).await?;\n",
        )])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let expected = format!(
        "crates/orders/src/reading.rs:{}: `Ok(())` answers the refusal the `let Ok(..) = .. \
         else` block at line {} caught",
        line_of(&module, "return Ok(())"),
        line_of(&module, "let Ok(order)")
    );
    assert!(correction.contains(&expected), "the correction carries `{expected}`: {correction}");
    assert_eq!(
        correction.matches("crates/orders/src/reading.rs:").count(),
        1,
        "the handler's own tail is no finding: {correction}"
    );
    assert!(
        text(&project, "crates/orders/src/reading.rs").contains("validate(&input).await?;"),
        "the error is propagated"
    );
}

// A read of the store under a constant key broken over lines, as a long
// call is formatted, is the read its miss is held to, so a `None` arm
// answered empty after it is named with the line the call opens on. The
// slice's own turn writes the fetch into the read.
#[tokio::test]
async fn omnia_empty_miss_read_across_lines() {
    let project = omnia_tree().await;
    let empty = "    let cached = StateStore::get(\n        provider,\n        RATES_KEY,\n    )\n    \
                 .await?;\n    let table = match cached {\n        Some(bytes) => \
                 serde_json::from_slice(&bytes)?,\n        None => HashMap::new(),\n    };\n    \
                 Ok(table)\n";
    let fetched = "    if let Some(bytes) = StateStore::get(provider, RATES_KEY).await? {\n        \
                   return Ok(serde_json::from_slice(&bytes)?);\n    }\n    let bytes = \
                   fetch_rates(provider).await?;\n    StateStore::set(provider, RATES_KEY, \
                   &bytes, Some(3600)).await?;\n    Ok(serde_json::from_slice(&bytes)?)\n";
    let module = |rates: &str| {
        format!(
            "//! Rates by region.\n\nuse std::collections::HashMap;\n\nuse omnia_sdk::{{HttpRequest, \
             StateStore}};\n\n/// The store key the rate table sits under.\npub const RATES_KEY: \
             &str = \"rates/table\";\n\n/// The rate table, fetched when the store holds none.\n///\n\
             /// # Errors\n///\n/// `bad_gateway` when the upstream refuses.\npub async fn rates<P: \
             HttpRequest + StateStore>(provider: &P) -> omnia_sdk::Result<HashMap<String, \
             u32>> {{\n{rates}}}\n\nasync fn fetch_rates<P: HttpRequest>(_provider: &P) -> \
             omnia_sdk::Result<Vec<u8>> {{\n    Ok(Vec::new())\n}}\n"
        )
    };
    let first_tree = module(empty);
    let written = [("crates/rates/src/table.rs", first_tree.as_str())];
    let report = r#"{"covered": ["REQ-001"], "written": ["crates/rates/src/table.rs"]}"#;
    let model = Scripted::answering([report, report])
        .calling(0, [write_files(&written)])
        .calling(1, [edit_files(&[("crates/rates/src/table.rs", empty, fetched)])]);

    let slice = slice_for("rates-table", &["rates.table"]);
    OMNIA.build_slice(&project, &model, &slice).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    let expected = format!(
        "crates/rates/src/table.rs:{}: a miss answered as no entries (`None => \
         HashMap::new()`) after the read at line {}",
        line_of(&first_tree, "None => HashMap::new()"),
        line_of(&first_tree, "StateStore::get(")
    );
    assert!(correction.contains(&expected), "the correction carries `{expected}`: {correction}");
    assert_eq!(
        correction.matches("crates/rates/src/table.rs:").count(),
        1,
        "the fetch helper's own empty answer is no finding: {correction}"
    );
    assert!(
        text(&project, "crates/rates/src/table.rs").contains("StateStore::set(provider, RATES_KEY"),
        "the miss fetches and stores the table"
    );
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
    omnia_template::entry(path).unwrap_or_else(|| panic!("the seed holds `{path}`")).body.to_owned()
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
