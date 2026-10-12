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
    Context, MergeStrategy, Report, Slice, TargetAdapter, Verdict, VerifyContext,
};
use emery_sdk::{Doc, Error, body};
use omnia_sdk::model::ToolCall;
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

const OMNIA_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/tests/reading.rs", "app/Cargo.toml", "app/src/routes.rs"]}"#;
// The same slice's report over a tree that held the root files already, so
// it names its stem's files alone.
const OMNIA_STEM_REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["crates/orders/Cargo.toml", "crates/orders/src/lib.rs", "crates/orders/src/reading.rs", "crates/orders/tests/reading.rs"]}"#;
// The lines the omnia slice adds to the root crate's two lists, each after
// the anchor the seeded file holds.
const OMNIA_STEM_LINE: &str = "app-orders = { path = \"../crates/orders\" }\n";
const OMNIA_ROUTE_LINE: &str = "    let router = app_orders::reading::mount(router);\n";
const OMNIA_FAILED: &str = r#"{"passed": false, "failures": ["check 2 (cargo test --workspace): crates/orders/tests/reading.rs — `an_order_with_no_items_is_refused` fails: expected 400, got 201"]}"#;

// The files the one scripted `write_files` call lays for the omnia target: a
// part of the `orders` stem as a module of the stem's crate, with the stem's
// manifest, its line in the stem's list, and its test in the stem's
// `tests/`; the root crate's two lists are the seed's, edited in place.
const OMNIA_WRITTEN: [(&str, &str); 4] = [
    ("crates/orders/Cargo.toml", "[package]\nname = \"app-orders\"\n"),
    ("crates/orders/src/lib.rs", "pub mod reading;\n"),
    ("crates/orders/src/reading.rs", "pub fn mount() {}\n"),
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

const NODE: Shipped<tgt_node::Adapter> = Shipped::new("node", tgt_node::PROSE);

const OMNIA: Shipped<tgt_omnia::Adapter> = Shipped::new("omnia", tgt_omnia::PROSE);

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
            model,
        };
        A::build(&ctx).await
    }

    // One verification of `project`, as the component's export puts it.
    async fn verify(&self, project: &Scratch, model: &Scripted) -> Result<Verdict, Error> {
        let ctx = VerifyContext {
            adapter_id: self.name,
            workspace: root(project),
            model,
        };
        A::verify(&ctx).await
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
    // tools declared for its repairs. Answers the tree, for what the target's
    // own lists must hold.
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

    // One verify over a tree seeded with `manifest`, answering `failed`: a
    // verdict that fails is the adapter's answer, carried whole to the caller.
    async fn failed(&self, manifest: (&str, &str), failed: &str) {
        let project = scratch();
        project.write(manifest.0, manifest.1);
        let model = Scripted::answering([failed]);

        let verdict = self
            .verify(&project, &model)
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
    let declared: Vec<(&str, MergeStrategy)> = tgt_node::Adapter::MERGE_RULES
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

#[tokio::test]
async fn node_verify() {
    NODE.failed(("package.json", "{}"), TYPESCRIPT_FAILED).await;
}

// The root files are the adapter's seed, laid before the slice's turn where
// the tree lacks them and counted as the slice's: the slice lays its stem's
// files and adds its lines to the two seeded lists in place.
#[tokio::test]
async fn omnia() {
    let router_anchor = "    let router = axum::Router::new();\n";
    let stem_anchor = "omnia-sdk.workspace = true\n";
    let edits = [
        ("app/src/routes.rs", router_anchor, &*format!("{router_anchor}{OMNIA_ROUTE_LINE}")),
        ("app/Cargo.toml", stem_anchor, &*format!("{stem_anchor}{OMNIA_STEM_LINE}")),
    ];
    let project = OMNIA.built(&OMNIA_WRITTEN, &edits, OMNIA_REPORT).await;

    for seed in tgt_omnia::SEED {
        let held = project.read(&seed.path);
        if seed.path == "app/src/routes.rs" || seed.path == "app/Cargo.toml" {
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
        Some(b"pub mod reading;\n".as_slice()),
        "the stem's list holds the part's one line"
    );
    let routes = project.read("app/src/routes.rs").expect("the router list");
    let routes = String::from_utf8(routes).expect("UTF-8");
    assert!(
        routes.contains(&format!("{router_anchor}{OMNIA_ROUTE_LINE}    router.with_state")),
        "the router list holds the part's one line after the seed's anchor: {routes}"
    );
    let manifest = project.read("app/Cargo.toml").expect("the root crate's manifest");
    let manifest = String::from_utf8(manifest).expect("UTF-8");
    assert!(
        manifest.ends_with(&format!("{stem_anchor}{OMNIA_STEM_LINE}")),
        "the root crate's manifest ends with the stem's one line: {manifest}"
    );
}

// A tree that already holds the root files keeps them: the seed lays none,
// and the slice's brief is told of none.
#[tokio::test]
async fn omnia_seeded() {
    let project = scratch();
    for seed in tgt_omnia::SEED {
        project.write(&seed.path, "# held\n");
    }
    let model = Scripted::answering([OMNIA_STEM_REPORT]).calling(0, [write_files(&OMNIA_WRITTEN)]);

    let report = OMNIA.build(&project, &model).await.expect("build over the held tree");
    model.assert_exhausted();

    for seed in tgt_omnia::SEED {
        assert_eq!(
            project.read(&seed.path).as_deref(),
            Some(b"# held\n".as_slice()),
            "`{}` stands as the tree held it",
            seed.path
        );
    }
    assert_eq!(
        report.written,
        OMNIA_WRITTEN.map(|(path, _)| path.to_owned()),
        "the report names the stem's files and no seed the tree held"
    );
}

// The rules a wave's slices merge under: the lockfile kept from the
// integrated side, the root crate's manifest and every stem's as a union over
// their dependency lists, the router list and every stem's module list as a
// union, and every other path under none, the root manifest holding the pins
// and the application's `lib.rs` among them.
#[test]
fn omnia_rules() {
    let declared: Vec<(&str, MergeStrategy)> = tgt_omnia::Adapter::MERGE_RULES
        .iter()
        .map(|rule| (rule.paths.as_ref(), rule.strategy))
        .collect();

    assert_eq!(
        declared,
        [
            ("Cargo.lock", MergeStrategy::Ours),
            ("app/Cargo.toml", MergeStrategy::Union),
            ("crates/*/Cargo.toml", MergeStrategy::Union),
            ("app/src/routes.rs", MergeStrategy::Union),
            ("crates/*/src/lib.rs", MergeStrategy::Union),
        ],
        "the merge rules the adapter declares, in order"
    );
}

#[tokio::test]
async fn omnia_verify() {
    OMNIA.failed(("Cargo.toml", "[workspace]\n"), OMNIA_FAILED).await;
}
