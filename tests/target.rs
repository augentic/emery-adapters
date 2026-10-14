//! Verifies what every shipped target adapter declares and does, natively.
//!
//! Each target's `Adapter` is called as the component's export calls it, over
//! a scratch tree it writes through the SDK's `write_files` and `edit_files`
//! tools and a strict model script. Assertions use what the adapter declares and what the slice
//! carries rather than SDK prompt wording; the turn's shape, the tool, and the
//! gates are the SDK's, asserted in its own suite. The component boundary
//! itself is `component.rs`'s.

#![cfg(not(target_arch = "wasm32"))]

use std::marker::PhantomData;

use emery_sdk::target::{
    Base, Context, MergeStrategy, Report, Slice, TargetAdapter, Verdict, VerifyContext, Worker,
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
// The same slice's report over a tree that held the seed already, so it
// names its stem's files alone.
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
const OMNIA_FAILED: &str = r#"{"passed": false, "failures": ["check 5 (cargo test --workspace): crates/orders/tests/reading.rs — `an_order_with_no_items_is_refused` fails: expected `bad_request`, got Ok"]}"#;
// A pin of the root manifest and the edit a turn might put to it.
const OMNIA_PIN: &str = "omnia-sdk = \"0.36.0\"\n";
const OMNIA_PIN_EDITED: &str = "omnia-sdk = \"0.37.0\"\n";

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

    // One build of the slice into `project`, as the component's export puts it.
    async fn build(&self, project: &Scratch, model: &Scripted) -> Result<Report, Error> {
        let slice = slice();
        let ctx = Context {
            adapter_id: self.name,
            slice: &slice,
            workspace: root(project),
            worker: &A::WORKER,
            model,
        };
        A::build(&ctx).await
    }

    // One verification of `project`, as the component's export puts it.
    async fn verify(&self, project: &Scratch, model: &Scripted) -> Result<Verdict, Error> {
        let ctx = VerifyContext {
            adapter_id: self.name,
            workspace: root(project),
            worker: &A::WORKER,
            model,
        };
        A::verify(&ctx).await
    }

    // The grant the adapter's `WORKER` puts on a turn, as the script records it.
    fn granted() -> Option<SeenWorker> {
        A::WORKER.grant().map(SeenWorker::from)
    }

    // The prompt at `path` as this build compiled it in.
    fn prompt(&self, path: &str) -> &'static str {
        body(self.docs, path).unwrap_or_else(|| panic!("`{path}` is in `{}`'s PROSE", self.name))
    }

    // One build laying `written` through one `write_files` call, then the
    // `edits` through one `edit_files` call, and answering `report`, then one
    // verify passing, with what every target shares asserted: the build turn
    // opens under the embedded `build.md` with the tree lent and both write
    // tools beside the reference tools, its brief carrying the slice; what the
    // model wrote through the tools is what the tree holds and the report
    // names; the verify turn opens under `verify.md` over the same tree, both
    // tools declared for its repairs; each turn carries the adapter's worker
    // grant. Answers the tree, for what the target's own lists must hold.
    async fn built(
        &self, written: &[(&str, &str)], edits: &[(&str, &str, &str)], report: &str,
    ) -> Scratch {
        let project = scratch();
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
        assert_eq!(seen.len(), 2, "the build opens one completion, the verify one");
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
    Slice {
        id: "SLICE-001".to_owned(),
        name: "orders-reading".to_owned(),
        base: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        requirements: vec!["REQ-001".to_owned()],
        spec: "# Specification\n\n## Requirement: orders.get-id.return-order\n\nID: REQ-001\n\nAn \
               order is returned by its id.\n"
            .to_owned(),
        design: "# Design\n\n## Type: Order\n\nAn id and its items.\n".to_owned(),
        plan: "## Slice: orders-reading\n\nID: SLICE-001\nRequirements: REQ-001\nTypes: \
               Order\nDepends on: none\n"
            .to_owned(),
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

#[tokio::test]
async fn node() {
    let project = NODE.built(&TYPESCRIPT_WRITTEN, &[], TYPESCRIPT_REPORT).await;

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

// The seed is laid before the slice's turn where the tree lacks it and
// counted as the slice's: the slice lays its stem's files and adds its lines
// to the seeded lists in place, the path table, the route list, the route
// rung, and the root manifest's two lists among them.
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
    for seed in emery_omnia::seed() {
        let held = project.read(&seed.path);
        if edited.contains(&seed.path.as_ref()) {
            assert!(held.is_some(), "the tree holds the seeded list `{}`", seed.path);
        } else {
            assert_eq!(
                held.as_deref(),
                Some(seed.content.as_bytes()),
                "the tree holds the seed `{}` as declared",
                seed.path
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

// A tree that already holds the seed keeps it: the seed lays none, and the
// slice's brief is told of none. The root ignore file is the one exception,
// since the adapter puts it back before the turn whatever the tree held,
// and reports it.
#[tokio::test]
async fn omnia_seeded() {
    let project = scratch();
    for seed in emery_omnia::seed() {
        if seed.path == "Cargo.toml" {
            project.write(&seed.path, seed.content.as_bytes());
        } else {
            project.write(&seed.path, "# held\n");
        }
    }
    let model = Scripted::answering([OMNIA_STEM_REPORT]).calling(0, [write_files(&OMNIA_WRITTEN)]);

    let report = OMNIA.build(&project, &model).await.expect("build over the held tree");
    model.assert_exhausted();

    for seed in emery_omnia::seed() {
        let expected = if seed.path == "Cargo.toml" || seed.path == ".gitignore" {
            seed.content.as_bytes()
        } else {
            b"# held\n"
        };
        assert_eq!(
            project.read(&seed.path).as_deref(),
            Some(expected),
            "`{}` stands as the tree held it, the ignore file as the seed spells it",
            seed.path
        );
    }
    let mut written = OMNIA_WRITTEN.map(|(path, _)| path.to_owned()).to_vec();
    written.insert(0, ".gitignore".to_owned());
    assert_eq!(
        report.written, written,
        "the report names the stem's files, the ignore file put back, and no seed the tree held"
    );
}

// The repository the engine lays holds a one-line ignore file at its root
// commit, which the seed never writes over: the template's is put in its
// place before the turn, so the build output the checks write is kept out
// of the slice's commit, and the file is reported as the slice's.
#[tokio::test]
async fn omnia_ignore_restored() {
    let project = scratch();
    project.write(".gitignore", ".emery/\n");
    let model = Scripted::answering([OMNIA_REPORT]).calling(0, [write_files(&OMNIA_WRITTEN)]);

    let report = OMNIA.build(&project, &model).await.expect("build over the laid repository");
    model.assert_exhausted();

    assert_eq!(text(&project, ".gitignore"), omnia_seed(".gitignore"), "the template's stands");
    assert!(
        omnia_seed(".gitignore").lines().any(|line| line == "target/"),
        "the template's ignore file keeps the build output out of the tree"
    );
    assert!(
        report.written.iter().any(|path| path == ".gitignore"),
        "the file put back is reported: {:?}",
        report.written
    );
    let seen = model.seen();
    assert!(
        !seen[0].messages[0].contains(".gitignore"),
        "the brief names no ignore file laid, since the tree held one: {}",
        seen[0].messages[0]
    );
}

// The seed is every file of the layout outside `crates/<stem>/`, each at a
// path the SDK lays, declared once, the root manifest pinning omnia at one
// version so a bump is one edit.
#[test]
fn omnia_seed_files() {
    let seed = emery_omnia::seed();
    let paths: std::collections::BTreeSet<&str> =
        seed.iter().map(|seed| seed.path.as_ref()).collect();
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
        .into_iter()
        .collect()
    );
    assert_eq!(paths.len(), seed.len(), "a seed path is declared once");
    for seed in &seed {
        emery_sdk::beneath(&seed.path)
            .unwrap_or_else(|bad| panic!("the seed path `{}` {bad}", seed.path));
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
    OMNIA.failed(&omnia_tree(), OMNIA_FAILED).await;
}

// A verify whose repair changed a static file, over a tree that lacks
// another, is refused with each named: the lines to put back for the one,
// the template's body whole for the other, since the turn has no other way
// to lay it. The repair puts both back and the verdict is accepted over the
// tree as the template spells it.
#[tokio::test]
async fn omnia_static_drift_refused() {
    let project = omnia_tree();
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
    let project = scratch();
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
    let project = omnia_tree();
    project.write(
        "crates/orders/src/consuming.rs",
        "#![allow(clippy::pedantic)]\n//! Consumes orders.\n\npub fn consume() {\n    tokio::spawn(async {});\n}\n",
    );
    project.write(
        "crates/start/src/main_loop.rs",
        "//! The loop.\n\nuse std::env;\n\npub fn run() {\n    let _ = env::var(\"PORT\");\n    std::thread::sleep(std::time::Duration::from_secs(1));\n}\n",
    );
    project.write(
        "crates/start/src/lifecycle.rs",
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
        "crates/orders/src/consuming.rs",
        "crates/start/src/main_loop.rs",
        "crates/start/src/lifecycle.rs",
    ];
    let model = Scripted::answering([PASSED, PASSED]).calling(1, [delete_files(&removed)]);

    let verdict = OMNIA.verify(&project, &model).await.expect("the repaired tree is accepted");
    model.assert_exhausted();

    assert!(verdict.passed);
    let correction = correction(&model);
    for line in [
        "a module simulates the host",
        "crates/orders/src/consuming.rs:1: the workspace's lints silenced",
        "crates/orders/src/consuming.rs:5: a task of its own",
        "crates/start/src/main_loop.rs:3: the environment read past the host",
        "crates/start/src/main_loop.rs:7: a thread of its own",
        "crates/start/src/lifecycle.rs:4: a start-up flag of its own",
        "crates/start/src/lifecycle.rs:6: a stop request of its own",
        "crates/start/src/lifecycle.rs:8: an exit delay of its own",
        "crates/start/src/lifecycle.rs:12: a step-done write of its own",
    ] {
        assert!(correction.contains(line), "the correction carries `{line}`: {correction}");
    }
    assert!(!correction.contains("tests/"), "a stem's tests are not read: {correction}");
    for path in removed {
        assert!(project.read(path).is_none(), "`{path}` is removed");
    }
}

// A stem whose modules reach the host through the provider alone passes,
// whatever its tests spell.
#[tokio::test]
async fn omnia_verify_stem_clean() {
    let project = omnia_tree();
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
    let project = omnia_tree();
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
    let project = omnia_tree();
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
    let project = omnia_tree();
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

// A tree holding every file of the seed, as an integrated tree does once a
// slice has built, since the adapter holds the static files to the template
// at every verify.
fn omnia_tree() -> Scratch {
    let project = scratch();
    for seed in emery_omnia::seed() {
        project.write(&seed.path, seed.content.as_bytes());
    }
    project
}

fn omnia_seed(path: &str) -> String {
    emery_omnia::seed()
        .into_iter()
        .find(|seed| seed.path == path)
        .unwrap_or_else(|| panic!("the seed lays `{path}`"))
        .content
        .into_owned()
}

// The seed's root manifest with the `orders` stem's line in each list.
fn omnia_manifest_with_orders() -> String {
    omnia_seed("Cargo.toml").replace(
        OMNIA_PATH_CRATE_ANCHOR,
        &format!("{OMNIA_PATH_CRATE_ANCHOR}{OMNIA_PATH_CRATE_LINE}"),
    ) + OMNIA_STEM_LINE
}
