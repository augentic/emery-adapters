//! Verifies what every shipped target adapter declares and does, natively.
//!
//! Each target's `Adapter` is called as the component's export calls it, over
//! a scratch tree its initialisation lays into and its turns write through
//! the SDK's `write_files` and `edit_files` tools under a strict model
//! script. Assertions use what the adapter declares and what the slice
//! carries rather than SDK prompt wording; the turn's shape, the tool, and the
//! gates are the SDK's, asserted in its own suite. The component boundary
//! itself is `component.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeSet;
use std::marker::PhantomData;
use std::os::unix::fs::symlink;
use std::path::Path;

use emery_sdk::target::{
    Base, BuildContext, InitContext, MergeStrategy, Outcome, Report, Slice, TargetAdapter, Task,
    Verdict, VerifyContext, Worker,
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

const OMNIA_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/tests/reading.rs", "crates/common/src/routes/http.rs", "Cargo.toml", "src/routes.rs", "tests/routes.rs"]}"#;
// The same slice's report where its turn edited no list, so it names its
// stem's files alone.
const OMNIA_STEM_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/tests/reading.rs"]}"#;
// The lines the omnia slice adds to the seed's lists, each after the anchor
// the seeded file holds: the path table's constant, the route list's
// statement, the route rung's entry, and the stem in the root manifest's two
// lists.
const OMNIA_PATH_ANCHOR: &str =
    "//! HTTP ingress paths served by the guest: one constant per route, each added at the end.\n";
const OMNIA_PATH_LINE: &str =
    "\n/// Read an order by its id.\npub const ORDER: &str = \"/orders/{id}\";\n";
const OMNIA_ROUTE_ANCHOR: &str = "    let router = axum::Router::new();\n";
const OMNIA_ROUTE_LINE: &str = "    let router = router.route(\n        common::routes::http::ORDER,\n        \
                                omnia_sdk::api::http::get(orders::reading::get_order),\n    );\n";
const OMNIA_RUNG_ANCHOR: &str = "    // one entry per route, each added at the end\n";
const OMNIA_RUNG_LINE: &str = "    (Method::GET, common::routes::http::ORDER),\n";
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
    ("crates/orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
    ("crates/orders/src/lib.rs", "//! The `orders` stem.\n\npub mod reading;\n"),
    ("crates/orders/src/reading.rs", "pub async fn get_order() {}\n"),
    (
        "crates/orders/tests/reading.rs",
        "#[tokio::test]\nasync fn an_order_is_returned_by_its_id() {}\n",
    ),
];

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

// The layout is the whole of what the adapter lays, so it builds into a
// fresh tree and never over a project.
#[test]
fn node_base() {
    assert_eq!(emery_node::Adapter::BASE, Base::Fresh, "the base the adapter declares");
}

// What a build's or verify's worker reaches beyond the tree: the npm
// registry, a Node installed beneath the home by nvm, Volta, or fnm, and
// Volta's root named for it.
#[test]
fn node_worker() {
    assert_eq!(
        emery_node::Adapter::WORKER,
        Worker {
            egress: &["registry.npmjs.org"],
            read: &["~/.nvm/versions/node", "~/.volta", "~/.local/share/fnm"],
            env: &[("VOLTA_HOME", "~/.volta")],
        },
        "the worker grant the adapter declares"
    );
}

#[tokio::test]
async fn node_verify() {
    let project = scratch();
    project.write("package.json", "{}");
    NODE.failed(&project, TYPESCRIPT_FAILED).await;
}

// The seed is laid by the initialisation, before the slice's turn, and is
// none of the slice's: the slice lays its stem's files and adds its lines
// to the seeded lists in place, the path table, the route list, the route
// rung, and the root manifest's two lists among them, every other file of
// the seed standing as the template spells it.
#[tokio::test]
async fn omnia() {
    let edits = [
        (
            "crates/common/src/routes/http.rs",
            OMNIA_PATH_ANCHOR,
            &*format!("{OMNIA_PATH_ANCHOR}{OMNIA_PATH_LINE}"),
        ),
        ("src/routes.rs", OMNIA_ROUTE_ANCHOR, &*format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}")),
        ("tests/routes.rs", OMNIA_RUNG_ANCHOR, &*format!("{OMNIA_RUNG_ANCHOR}{OMNIA_RUNG_LINE}")),
        (
            "Cargo.toml",
            OMNIA_PATH_CRATE_ANCHOR,
            &*format!("{OMNIA_PATH_CRATE_ANCHOR}{OMNIA_PATH_CRATE_LINE}"),
        ),
        ("Cargo.toml", OMNIA_STEM_ANCHOR, &*format!("{OMNIA_STEM_ANCHOR}{OMNIA_STEM_LINE}")),
    ];
    let project = OMNIA.built(&OMNIA_WRITTEN, &edits, OMNIA_REPORT).await;

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
        project.read("crates/orders/src/lib.rs").as_deref(),
        Some(b"//! The `orders` stem.\n\npub mod reading;\n".as_slice()),
        "the stem's list holds the part's one line"
    );
    let table = text(&project, "crates/common/src/routes/http.rs");
    assert_eq!(
        table,
        format!("{OMNIA_PATH_ANCHOR}{OMNIA_PATH_LINE}"),
        "the path table holds the route's constant after its doc"
    );
    let routes = text(&project, "src/routes.rs");
    assert!(
        routes.contains(&format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}    router.with_state")),
        "the route list holds the route's one statement before the terminal line: {routes}"
    );
    let rung = text(&project, "tests/routes.rs");
    assert!(
        rung.contains(&format!("{OMNIA_RUNG_ANCHOR}{OMNIA_RUNG_LINE}];")),
        "the route rung's table holds the route's one entry before its close: {rung}"
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
            "crates/common/src/routes.rs",
            "crates/common/src/routes/http.rs",
            "crates/common/src/routes/topic.rs",
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
            ("clippy.toml", MergeStrategy::Union),
            ("src/routes.rs", MergeStrategy::Union),
            ("src/messaging.rs", MergeStrategy::Union),
            ("tests/routes.rs", MergeStrategy::Union),
            ("tests/messaging.rs", MergeStrategy::Union),
            ("crates/common/src/routes/*.rs", MergeStrategy::Union),
            ("crates/common/src/config.rs", MergeStrategy::Union),
            ("crates/*/Cargo.toml", MergeStrategy::Union),
            ("crates/*/src/lib.rs", MergeStrategy::Union),
        ],
        "the merge rules the adapter declares, in order"
    );
}

// The seed and the slices are the whole of what the adapter lays, so it
// builds into a fresh tree and never over a project.
#[test]
fn omnia_base() {
    assert_eq!(emery_omnia::Adapter::BASE, Base::Fresh, "the base the adapter declares");
}

// What a build's or verify's worker reaches beyond the tree: the crates.io
// index and its downloads, the toolchain and the binaries beneath the home,
// and `RUSTUP_HOME` named for it.
#[test]
fn omnia_worker() {
    assert_eq!(
        emery_omnia::Adapter::WORKER,
        Worker {
            egress: &["index.crates.io", "static.crates.io"],
            read: &["~/.rustup", "~/.cargo/bin"],
            env: &[("RUSTUP_HOME", "~/.rustup")],
        },
        "the worker grant the adapter declares"
    );
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

// A stem's module that silences the lints, spawns a task, reaches the
// system, or keeps a flag machine in the state store is a simulated host:
// the verdict is refused with every line named, whatever the checks
// answered, a stem's tests unread, and the modules removed are the repair.
#[tokio::test]
async fn omnia_verify_simulated_host() {
    let project = omnia_tree().await;
    project.write("crates/start/Cargo.toml", "[package]\nname = \"start\"\n");
    project.write("crates/start/src/lib.rs", "//! The `start` stem.\n");
    project.write(
        "crates/orders/src/consuming.rs",
        "#![allow(clippy::pedantic)]\n//! Consumes orders.\n\nuse std::env;\n\npub fn consume() {\n    let _ = env::var(\"PORT\");\n    tokio::spawn(async {});\n    std::thread::sleep(std::time::Duration::from_secs(1));\n}\n",
    );
    project.write(
        "crates/orders/src/main_loop.rs",
        "//! The loop.\n\npub fn run() {\n    loop {}\n}\n",
    );
    project.write(
        "crates/orders/src/lifecycle.rs",
        "//! The lifecycle.\n\n/// Whether start ran.\npub const START_STARTED_KEY: &str = \"start/started\";\n\
         /// Whether a stop was asked for.\npub const STOP_REQUESTED: &str = \"start/stop-requested\";\n\
         /// How long to wait before exiting.\npub const EXIT_DELAY: u64 = 5;\n\n\
         /// Records that start ran.\npub async fn started<P: omnia_sdk::StateStore>(provider: &P) -> omnia_sdk::Result<()> {\n\
         \x20   provider.set(START_STARTED_KEY, b\"1\", None).await?;\n    Ok(())\n}\n",
    );
    project.write(
        "crates/orders/tests/consuming.rs",
        "#[tokio::test]\nasync fn waits() {\n    tokio::time::sleep(std::time::Duration::from_secs(1)).await;\n}\n",
    );
    let removed = [
        "crates/start/Cargo.toml",
        "crates/start/src/lib.rs",
        "crates/orders/src/consuming.rs",
        "crates/orders/src/main_loop.rs",
        "crates/orders/src/lifecycle.rs",
    ];
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
        "a module simulates the host",
        "crates/orders/src/consuming.rs:1: the workspace's lints silenced",
        "crates/orders/src/consuming.rs:4: the environment read past the host",
        "crates/orders/src/consuming.rs:8: a task of its own",
        "crates/orders/src/consuming.rs:9: a thread of its own",
        "crates/orders/src/lifecycle.rs:4: a start-up flag of its own",
        "crates/orders/src/lifecycle.rs:6: a stop request of its own",
        "crates/orders/src/lifecycle.rs:8: an exit delay of its own",
        "crates/orders/src/lifecycle.rs:12: a step-done write of its own",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert!(
        !correction.contains("main_loop.rs:4"),
        "a module removed for its lifecycle is not also edited in place: {correction}"
    );
    assert!(!correction.contains("tests/"), "a stem's tests are not read: {correction}");
    for path in removed {
        assert!(project.read(path).is_none(), "`{path}` is removed");
    }
}

// A module under a stem's `src/` that retries an upstream in a `loop {`,
// and one that runs the process, are refused at the build rather than left
// to a wave's verify, each under its own finding: the loop's names the
// line to remove from the module, the run's the module to remove. The
// slice's own turn edits the loop into one fetch and deletes the module,
// and the accepted report names what the tree still holds.
#[tokio::test]
async fn omnia_build_process_refused() {
    let project = omnia_tree().await;
    let looped = "    let mut attempt = 0;\n    loop {\n        attempt += 1;\n        match \
                  fetch(provider).await {\n            Ok(stops) => return Ok(stops),\n            \
                  Err(error) if attempt < 3 => tracing::warn!(\"retry loop {error}\"),\n            \
                  Err(error) => return Err(error),\n        }\n    }\n";
    let once =
        "    fetch(provider).await.inspect_err(|error| tracing::warn!(\"retry loop {error}\"))\n";
    let prefetch = format!(
        "//! Prefetches the stop table (`fn start` in `struct App`).\n\nuse \
         omnia_sdk::HttpRequest;\n\n/// Readiness of the stop table.\npub struct Readiness {{\n    \
         /// Whether the table is ready.\n    pub ready: bool,\n}}\n\n/// Load the stops.\n///\n\
         /// # Errors\n///\n/// `bad_gateway` when the upstream fails.\npub async fn \
         load_stops<P: HttpRequest>(provider: &P) -> omnia_sdk::Result<Vec<String>> {{\n{looped}}}\n"
    );
    let written = [
        ("crates/orders/Cargo.toml", "[package]\nname = \"orders\"\n"),
        (
            "crates/orders/src/lib.rs",
            "//! The `orders` stem.\n\npub mod prefetch;\npub mod runner;\n",
        ),
        ("crates/orders/src/prefetch.rs", prefetch.as_str()),
        (
            "crates/orders/src/runner.rs",
            "//! Runs the process.\n\n/// Run it.\npub fn run() {\n    loop {}\n}\n",
        ),
    ];
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/prefetch.rs", "crates/orders/src/runner.rs"]}"#;
    let repaired = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/prefetch.rs"]}"#;
    let model = Scripted::answering([first, repaired]).calling(0, [write_files(&written)]).calling(
        1,
        [
            edit_files(&[
                ("crates/orders/src/prefetch.rs", looped, once),
                ("crates/orders/src/lib.rs", "pub mod runner;\n", ""),
            ]),
            delete_files(&["crates/orders/src/runner.rs"]),
        ],
    );

    let report = OMNIA.build(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    let correction = correction(&model);
    for line in [
        "a module is the process the source was",
        "crates/orders/src/runner.rs:4: a run of its own (`fn run`)",
        "a module simulates the host",
        "crates/orders/src/prefetch.rs:18: a loop that polls or retries (`loop {`)",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for absent in ["prefetch.rs:8", "prefetch.rs:16", "prefetch.rs:22", "runner.rs:5"] {
        assert!(!correction.contains(absent), "`{absent}` is no finding: {correction}");
    }
    assert!(
        !text(&project, "crates/orders/src/prefetch.rs").contains("    loop {"),
        "the loop is edited into one fetch"
    );
    assert!(project.read("crates/orders/src/runner.rs").is_none(), "the module is removed");
    assert!(
        report.written.iter().any(|path| path == "crates/orders/src/prefetch.rs"),
        "the loader is reported: {:?}",
        report.written
    );
    assert!(
        !report.written.iter().any(|path| path == "crates/orders/src/runner.rs"),
        "a removed module is not reported: {:?}",
        report.written
    );
}

// Laying `crates/start/` (or a `crates/start-*` crate for a slice cut from
// `start`, such as `crates/start-prefetch/`) at a build is refused whole: the
// seed and the host are `start`, so every file under `crates/start` or
// `crates/start-*` is named and removed through `delete` with any `start` line
// dropped from the root `Cargo.toml`, its modules ignored by the other module
// rules so a setting read only there is flagged unread in the same round and no
// handler, loader, or miss inside it is asked to be wired or edited in place.
#[tokio::test]
async fn omnia_build_start_crate_refused() {
    let project = omnia_tree().await;
    let start_path = "start = { path = \"crates/start\" }\n";
    let start_crate = "start.workspace = true\n";
    let env_line = "pub const ENV: &str = \"ENV\";\n";
    let start_setting = "\n/// Broker address read only at start.\npub const START_BROKER_URL: \
                         &str = \"START_BROKER_URL\";\n";
    let written = [
        ("crates/start/Cargo.toml", "[package]\nname = \"start\"\n"),
        ("crates/start/src/lib.rs", "//! The `start` stem.\n\npub mod start;\n"),
        (
            "crates/start/src/start.rs",
            "//! Startup helper.\n\nuse common::config;\nuse omnia_sdk::api::Context;\nuse \
             omnia_sdk::{Config, StateStore};\n\npub const TABLE_KEY: &str = \"start/table\";\n\n\
             /// Probe readiness.\npub async fn ready<P: Config + StateStore>(\n    _input: \
             (),\n    context: Context<P>,\n) -> omnia_sdk::Result<Vec<String>> {\n    let _ = \
             Config::get(context.provider(), config::START_BROKER_URL).await?;\n    match \
             StateStore::get(context.provider(), TABLE_KEY).await? {\n        Some(_) => \
             Ok(Vec::new()),\n        None => Ok(Vec::new()),\n    }\n}\n",
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
    let first = r#"{"covered": ["REQ-001"], "written": ["crates/start/Cargo.toml", "crates/start/src/lib.rs", "crates/start/src/start.rs", "crates/start-prefetch/src/lib.rs", "crates/common/src/config.rs", "Cargo.toml"]}"#;
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
                    (
                        "crates/common/src/config.rs",
                        env_line,
                        &format!("{env_line}{start_setting}"),
                    ),
                ]),
            ],
        )
        .calling(
            1,
            [
                edit_files(&[
                    ("Cargo.toml", start_path, ""),
                    ("Cargo.toml", start_crate, ""),
                    ("crates/common/src/config.rs", start_setting, ""),
                ]),
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
        "setting `START_BROKER_URL` is read by no module",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for absent in
        ["handler `ready`", "crates/start/src/start.rs:", "crates/start-prefetch/src/lib.rs:"]
    {
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
        ("crates/stops/Cargo.toml", "[package]\nname = \"stops\"\n"),
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
        ("crates/rates/Cargo.toml", "[package]\nname = \"rates\"\n"),
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
        ("crates/zones/Cargo.toml", "[package]\nname = \"zones\"\n"),
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
        ("crates/fees/Cargo.toml", "[package]\nname = \"fees\"\n"),
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
    project.write("crates/shipment/Cargo.toml", "[package]\nname = \"shipment\"\n");
    project.write("crates/shipment/src/lib.rs", "//! The `shipment` stem.\n");
    let slice = slice_for(
        "order-items",
        &["order-items.add.reject-unknown-sku", "shipment-notes.record.keep-latest"],
    );
    let module = "//! Order items.\n\n/// How many lines an order holds.\n#[must_use]\npub fn \
                  count(items: &[u32]) -> usize {\n    items.len()\n}\n";
    let split = [
        ("crates/order/Cargo.toml", "[package]\nname = \"order\"\n"),
        ("crates/order/src/lib.rs", "//! The `order` stem.\n\npub mod items;\n"),
        ("crates/order/src/items.rs", module),
    ];
    let whole = [
        ("crates/order-items/Cargo.toml", "[package]\nname = \"order-items\"\n"),
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

// The two tables are held to their lists at every verify: a route's
// constant no statement or no rung entry names, a consumed topic's no rung
// entry names, a topic the rung lists and no statement consumes, a topic
// nothing consumes or publishes, and a statement over a literal are each
// named at their line, while a topic a module alone publishes to stands
// with no statement and no entry. The lines added, and the entries and
// constants removed, are the repair.
#[tokio::test]
async fn omnia_tables_held() {
    let (project, http, topics, routes) = omnia_tables_tree().await;
    let orders_statement = "    let router = router.route(\n        \
                            common::routes::http::ORDERS,\n        \
                            omnia_sdk::api::http::get(orders::reading::list_orders),\n    );\n";
    let orders_entry = "    (Method::GET, common::routes::http::ORDERS),\n";
    let cancelled_entry = "    common::routes::topic::ORDER_CANCELLED,\n";
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [edit_files(&[
            ("src/routes.rs", OMNIA_ROUTE_LINE, &format!("{OMNIA_ROUTE_LINE}{orders_statement}")),
            ("src/routes.rs", LITERAL_ROUTE, ""),
            ("tests/routes.rs", OMNIA_RUNG_LINE, &format!("{OMNIA_RUNG_LINE}{orders_entry}")),
            ("crates/common/src/routes/topic.rs", RETURNED_CONST, ""),
            ("crates/common/src/routes/topic.rs", BILLED_CONST, ""),
            ("tests/messaging.rs", BILLED_ENTRY, cancelled_entry),
        ])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let orders_at = line_of(&http, "pub const ORDERS");
    let returned_at = line_of(&topics, "pub const ORDER_RETURNED");
    let billed_at = line_of(&topics, "pub const ORDER_BILLED");
    let cancelled_at = line_of(&topics, "pub const ORDER_CANCELLED");
    let literal_at = line_of(&routes, "\"/health\"");
    for line in [
        "a table and its lists disagree".to_owned(),
        format!(
            "crates/common/src/routes/http.rs:{orders_at}: route `ORDERS` is named by no \
             statement of `src/routes.rs`"
        ),
        format!(
            "crates/common/src/routes/http.rs:{orders_at}: route `ORDERS` is named by no entry \
             of `ROUTES` in `tests/routes.rs`"
        ),
        format!(
            "crates/common/src/routes/topic.rs:{cancelled_at}: topic `ORDER_CANCELLED` is \
             consumed by `src/messaging.rs` and named by no entry of `TOPICS` in \
             `tests/messaging.rs`"
        ),
        format!(
            "crates/common/src/routes/topic.rs:{billed_at}: topic `ORDER_BILLED` is named by an \
             entry of `TOPICS` in `tests/messaging.rs` and consumed by no statement of \
             `src/messaging.rs`"
        ),
        format!(
            "crates/common/src/routes/topic.rs:{returned_at}: topic `ORDER_RETURNED` is consumed \
             by no statement of `src/messaging.rs` and published by no module"
        ),
        format!(
            "src/routes.rs:{literal_at}: a statement names no constant of \
             `crates/common/src/routes/http.rs`"
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    for named in ["route `ORDER` ", "ORDER_SHIPPED", "ORDER_PLACED"] {
        assert!(!correction.contains(named), "{named} is no finding: {correction}");
    }
    let routes = text(&project, "src/routes.rs");
    assert!(routes.contains(orders_statement) && !routes.contains(LITERAL_ROUTE), "{routes}");
    let topics = text(&project, "crates/common/src/routes/topic.rs");
    assert!(topics.contains("ORDER_SHIPPED") && !topics.contains("ORDER_RETURNED"), "{topics}");
}

// A `pub const` of the configuration catalog no module outside `common`
// reads is named at its line, a mention in a doc comment counting for
// nothing, while a key a module reads and the template's own `ENV` pass.
// Removing the constant is the repair.
#[tokio::test]
async fn omnia_setting_unread() {
    let project = omnia_tree().await;
    let env_line = "pub const ENV: &str = \"ENV\";\n";
    let unread = "\n/// Minutes between refreshes of the rate table.\npub const \
                  RATES_REFRESH_MINUTES: &str = \"RATES_REFRESH_MINUTES\";\n";
    let catalog = omnia_seed("crates/common/src/config.rs").replace(
        env_line,
        &format!(
            "{env_line}\n/// Base URL of the rates upstream.\npub const RATES_URL: &str = \
             \"RATES_URL\";\n{unread}"
        ),
    );
    project.write("crates/common/src/config.rs", &catalog);
    project.write(
        "crates/rates/src/table.rs",
        "//! The rate table, refreshed every `RATES_REFRESH_MINUTES`.\n\nuse common::config;\nuse \
         omnia_sdk::Config;\n\n/// The upstream the rates come from.\n///\n/// # Errors\n///\n/// \
         `bad_request` when the deployment sets none.\npub async fn upstream<P: \
         Config>(provider: &P) -> omnia_sdk::Result<String> {\n    Config::get(provider, \
         config::RATES_URL).await\n}\n",
    );
    let model = Scripted::answering([PASSED, PASSED])
        .calling(1, [edit_files(&[("crates/common/src/config.rs", unread, "")])]);

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    let unread_at = line_of(&catalog, "pub const RATES_REFRESH_MINUTES");
    for line in [
        "a setting of the catalog is read by no module".to_owned(),
        format!(
            "crates/common/src/config.rs:{unread_at}: setting `RATES_REFRESH_MINUTES` is read by \
             no module"
        ),
    ] {
        assert!(correction.contains(&line), "the correction carries `{line}`: {correction}");
    }
    for named in ["`RATES_URL`", "`ENV`"] {
        assert!(!correction.contains(named), "{named} is no finding: {correction}");
    }
    assert!(!text(&project, "crates/common/src/config.rs").contains("RATES_REFRESH_MINUTES"));
}

// A route list statement over a literal path rather than a table constant.
const LITERAL_ROUTE: &str = "    let router = router.route(\"/health\", \
                             omnia_sdk::api::http::get(start::health::health));\n";
// Two topic constants nothing consumes: one no module publishes to either,
// one the messaging rung lists.
const RETURNED_CONST: &str = "\n/// An order was returned.\npub const ORDER_RETURNED: &str = \
                              \"orders-returned.v1\";\n";
const BILLED_CONST: &str =
    "\n/// An order was billed.\npub const ORDER_BILLED: &str = \"orders-billed.v1\";\n";
const BILLED_ENTRY: &str = "    common::routes::topic::ORDER_BILLED,\n";

// The seeded tree with both tables filled and their lists disagreeing: the
// path table's `ORDER` wired and `ORDERS` wired nowhere, the route list
// holding a statement over a literal beside `ORDER`'s; the topic table's
// `ORDER_PLACED` consumed and listed, `ORDER_CANCELLED` consumed and
// unlisted, `ORDER_SHIPPED` published by a module of `orders` alone,
// `ORDER_RETURNED` named nowhere, and `ORDER_BILLED` listed by the rung
// alone. Answers the tree with the texts of the path table, the topic
// table, and the route list, for the lines a finding names.
async fn omnia_tables_tree() -> (Scratch, String, String, String) {
    let project = omnia_tree().await;
    let http = format!(
        "{OMNIA_PATH_ANCHOR}{OMNIA_PATH_LINE}\n/// List the orders.\npub const ORDERS: &str = \
         \"/orders\";\n"
    );
    project.write("crates/common/src/routes/http.rs", &http);
    let routes = omnia_seed("src/routes.rs").replace(
        OMNIA_ROUTE_ANCHOR,
        &format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}{LITERAL_ROUTE}"),
    );
    project.write("src/routes.rs", &routes);
    project.write(
        "tests/routes.rs",
        omnia_seed("tests/routes.rs")
            .replace(OMNIA_RUNG_ANCHOR, &format!("{OMNIA_RUNG_ANCHOR}{OMNIA_RUNG_LINE}")),
    );
    let topics = format!(
        "{}\n/// An order was placed.\npub const ORDER_PLACED: &str = \"orders-placed.v1\";\n\n\
         /// An order was cancelled.\npub const ORDER_CANCELLED: &str = \"orders-cancelled.v1\";\n\n\
         /// An order was shipped.\npub const ORDER_SHIPPED: &str = \
         \"orders-shipped.v1\";\n{RETURNED_CONST}{BILLED_CONST}",
        omnia_seed("crates/common/src/routes/topic.rs")
    );
    project.write("crates/common/src/routes/topic.rs", &topics);
    let topic_anchor = "    let router = messaging::Router::new(Client::new(OWNER, provider));\n";
    let consumed = "    let router = router.route(\n        \
                    config::topic_for(&env, common::routes::topic::ORDER_PLACED),\n        \
                    messaging::consume(orders::fulfilment::on_placed),\n    );\n    \
                    let router = router.route(\n        \
                    config::topic_for(&env, common::routes::topic::ORDER_CANCELLED),\n        \
                    messaging::consume(orders::fulfilment::on_cancelled),\n    );\n";
    project.write(
        "src/messaging.rs",
        omnia_seed("src/messaging.rs").replace(topic_anchor, &format!("{topic_anchor}{consumed}")),
    );
    let rung_anchor = "    // one entry per topic, each added at the end\n";
    let placed_entry = "    common::routes::topic::ORDER_PLACED,\n";
    project.write(
        "tests/messaging.rs",
        omnia_seed("tests/messaging.rs")
            .replace(rung_anchor, &format!("{rung_anchor}{placed_entry}{BILLED_ENTRY}")),
    );
    project.write(
        "crates/orders/src/shipping.rs",
        "//! Ships orders.\n\nuse common::{config, routes};\nuse omnia_sdk::{Config, Message, \
         Publish};\n\n/// Announce a shipped order.\n///\n/// # Errors\n///\n/// Fails when the \
         host cannot publish.\npub async fn announce<P: Config + Publish>(provider: &P, payload: \
         &[u8]) -> omnia_sdk::Result<()> {\n    let topic = config::topic(provider, \
         routes::topic::ORDER_SHIPPED).await;\n    Publish::send(provider, &topic, \
         &Message::new(payload)).await?;\n    Ok(())\n}\n",
    );
    (project, http, topics, routes)
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

// A handler no statement of the two root lists reaches is code the host
// never runs: the verdict is refused with the handler named at its line,
// whatever its tests pass, including one sharing a function name with a
// handler another module lists, while the listed handler, and a helper
// handed the provider rather than the `Context`, pass. The statement added
// is the repair.
#[tokio::test]
async fn omnia_handler_unlisted() {
    let project = omnia_tree().await;
    project.write(
        "src/routes.rs",
        omnia_seed("src/routes.rs")
            .replace(OMNIA_ROUTE_ANCHOR, &format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}")),
    );
    project.write(
        "crates/orders/src/reading.rs",
        "//! Reads orders.\n\nuse omnia_sdk::api::Context;\n\n\
         /// Read one order.\npub async fn get_order<P>(input: GetOrder, context: Context<P>) -> omnia_sdk::Result<Order> {\n    todo!()\n}\n\n\
         /// List the orders.\npub async fn list_orders<P>(\n    input: ListOrders,\n    context: Context<P>,\n) -> omnia_sdk::Result<Vec<Order>> {\n    todo!()\n}\n\n\
         /// The key of an order.\npub async fn key<P: omnia_sdk::StateStore>(provider: &P, id: &str) -> String {\n    format!(\"orders/{id}\")\n}\n",
    );
    project.write(
        "crates/payments/src/reading.rs",
        "//! Reads payments.\n\nuse omnia_sdk::api::Context;\n\n\
         /// Read one order's payment.\npub async fn get_order<P>(input: GetOrder, context: Context<P>) -> omnia_sdk::Result<Order> {\n    todo!()\n}\n",
    );
    let listing = "    let router = router.route(\n        common::routes::http::ORDERS,\n        \
                   omnia_sdk::api::http::get(orders::reading::list_orders),\n    );\n";
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [
            edit_files(&[(
                "src/routes.rs",
                OMNIA_ROUTE_LINE,
                &format!("{OMNIA_ROUTE_LINE}{listing}"),
            )]),
            delete_files(&["crates/payments/src/reading.rs"]),
        ],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the listed tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    for line in [
        "crates/orders/src/reading.rs:11: handler `list_orders` is reached by no list statement",
        "crates/payments/src/reading.rs:6: handler `get_order` is reached by no list statement",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for named in ["crates/orders/src/reading.rs:6", "`key`"] {
        assert!(!correction.contains(named), "{named} is no finding: {correction}");
    }
    assert!(
        text(&project, "src/routes.rs").contains(listing),
        "the route list holds the statement the repair added"
    );
}

// A statement reaches the one handler it hands, never the method, the
// crate, the module, or the constant it spells beside it: a handler named
// `get`, `http`, `orders`, or `consume` is listed by no statement that
// merely uses the word, and is refused at its line until a statement hands
// it.
#[tokio::test]
async fn omnia_handler_named_as_method() {
    let project = omnia_tree().await;
    project.write(
        "src/routes.rs",
        omnia_seed("src/routes.rs")
            .replace(OMNIA_ROUTE_ANCHOR, &format!("{OMNIA_ROUTE_ANCHOR}{OMNIA_ROUTE_LINE}")),
    );
    let topic_anchor = "    let router = messaging::Router::new(Client::new(OWNER, provider));\n";
    let topic_line = "    let router = router.route(\n        \
                      config::topic_for(&env, common::routes::topic::ORDER_PLACED),\n        \
                      messaging::consume(orders::fulfilment::on_placed),\n    );\n";
    project.write(
        "src/messaging.rs",
        omnia_seed("src/messaging.rs")
            .replace(topic_anchor, &format!("{topic_anchor}{topic_line}")),
    );
    project.write(
        "crates/orders/src/reading.rs",
        "//! Reads orders.\n\nuse omnia_sdk::api::Context;\n\n\
         /// Read one order.\npub async fn get_order<P>(input: GetOrder, context: Context<P>) -> omnia_sdk::Result<Order> {\n    todo!()\n}\n\n\
         /// Read the orders.\npub async fn get<P>(input: ListOrders, context: Context<P>) -> omnia_sdk::Result<Vec<Order>> {\n    todo!()\n}\n\n\
         /// Read over HTTP.\npub async fn http<P>(input: ListOrders, context: Context<P>) -> omnia_sdk::Result<Vec<Order>> {\n    todo!()\n}\n\n\
         /// Read every order.\npub async fn orders<P>(input: ListOrders, context: Context<P>) -> omnia_sdk::Result<Vec<Order>> {\n    todo!()\n}\n",
    );
    project.write(
        "crates/orders/src/fulfilment.rs",
        "//! Fulfils orders.\n\nuse omnia_sdk::api::Context;\n\n\
         /// Fulfil a placed order.\npub async fn on_placed<P>(input: Order, context: Context<P>) -> omnia_sdk::Result<()> {\n    todo!()\n}\n\n\
         /// Consume an order.\npub async fn consume<P>(input: Order, context: Context<P>) -> omnia_sdk::Result<()> {\n    todo!()\n}\n",
    );
    let model = Scripted::answering([PASSED, PASSED]).calling(
        1,
        [delete_files(&["crates/orders/src/reading.rs", "crates/orders/src/fulfilment.rs"])],
    );

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    for line in [
        "crates/orders/src/fulfilment.rs:11: handler `consume` is reached by no list statement",
        "crates/orders/src/reading.rs:11: handler `get` is reached by no list statement",
        "crates/orders/src/reading.rs:16: handler `http` is reached by no list statement",
        "crates/orders/src/reading.rs:21: handler `orders` is reached by no list statement",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    for named in ["`get_order`", "`on_placed`"] {
        assert!(!correction.contains(named), "{named} is no finding: {correction}");
    }
}

// A statement that calls a route function of the stem with no argument
// reaches the handler that function hands `handle_with`; a handler handed
// by a route function no statement calls is still reached by nothing.
#[tokio::test]
async fn omnia_handler_behind_route_function() {
    let project = omnia_tree().await;
    let listing = "    let router = router.route(common::routes::http::ORDERS, orders::placing::place_order_route());\n";
    project.write(
        "src/routes.rs",
        omnia_seed("src/routes.rs")
            .replace(OMNIA_ROUTE_ANCHOR, &format!("{OMNIA_ROUTE_ANCHOR}{listing}")),
    );
    project.write(
        "crates/orders/src/placing.rs",
        "//! Places orders.\n\nuse axum::routing::{MethodFilter, MethodRouter};\n\
         use omnia_sdk::api::http::{DecodeError, RawRequest, handle_with};\nuse omnia_sdk::api::{Client, Context};\n\n\
         /// The `POST /orders` route: a placed order is answered `201`.\n#[must_use]\n\
         pub fn place_order_route<P>() -> MethodRouter<Client<P>>\nwhere\n    P: Send + Sync + 'static,\n{\n    \
         handle_with(\n        MethodFilter::POST,\n        place_order,\n        \
         |raw: RawRequest<'_>| -> Result<PlaceOrder, DecodeError> { todo!() },\n        \
         |order: Order| todo!(),\n    )\n}\n\n\
         /// Place an order.\npub async fn place_order<P>(input: PlaceOrder, context: Context<P>) -> omnia_sdk::Result<Order> {\n    todo!()\n}\n\n\
         /// The `POST /orders/{id}/refund` route.\n#[must_use]\n\
         pub fn refund_route<P>() -> MethodRouter<Client<P>>\nwhere\n    P: Send + Sync + 'static,\n{\n    \
         handle_with(MethodFilter::POST, refund, |raw: RawRequest<'_>| -> Result<Refund, DecodeError> { todo!() }, |(): ()| todo!())\n}\n\n\
         /// Refund an order.\npub async fn refund<P>(input: Refund, context: Context<P>) -> omnia_sdk::Result<()> {\n    todo!()\n}\n",
    );
    let refund = "    let router = router.route(common::routes::http::REFUND, orders::placing::refund_route());\n";
    let model = Scripted::answering([PASSED, PASSED])
        .calling(1, [edit_files(&[("src/routes.rs", listing, &format!("{listing}{refund}"))])]);

    let verdict = OMNIA.verify(&project, &model).await.expect("the listed tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    assert!(
        correction.contains(
            "crates/orders/src/placing.rs:36: handler `refund` is reached by no list statement"
        ),
        "the handler behind the uncalled route function is named at its line: {correction}"
    );
    assert!(!correction.contains("`place_order`"), "`place_order` is no finding: {correction}");
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
