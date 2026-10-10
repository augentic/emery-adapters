//! Verifies what every shipped target adapter declares and does, natively.
//!
//! Each target's `Adapter` is called as the component's export calls it, over
//! a scratch tree it writes through the SDK's `write_files` tool and a strict
//! model script. Assertions use what the adapter declares and what the slice
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

const REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["src/orders/reading/package.json", "src/orders/reading/index.ts", "src/orders/index.ts", "src/index.ts", "test/orders/reading.test.ts"]}"#;
const PASSED: &str = r#"{"passed": true, "failures": []}"#;
const FAILED: &str = r#"{"passed": false, "failures": ["check 3 (npm test): 1 failing: an order with no items is refused"]}"#;

// The files the one scripted `write_files` call lays: a part of the `orders`
// stem in its own directory, with its manifest, its line in the stem's list
// and the stem's in the entry, and its test beside the stem's.
const WRITTEN: [(&str, &str); 5] = [
    ("src/orders/reading/package.json", "{ \"name\": \"@app/orders-reading\" }\n"),
    ("src/orders/reading/index.ts", "export function getOrder() {}\n"),
    ("src/orders/index.ts", "export * as reading from \"./reading/index.ts\";\n"),
    ("src/index.ts", "export * as orders from \"./orders/index.ts\";\n"),
    ("test/orders/reading.test.ts", "import { test } from \"node:test\";\n"),
];

// A shipped target adapter: its name, the prose its turns open under, and
// the `Adapter` every call goes through.
struct Shipped<A> {
    name: &'static str,
    docs: &'static [Doc],
    adapter: PhantomData<fn() -> A>,
}

const TYPESCRIPT_TARGET: Shipped<typescript_target::Adapter> =
    Shipped::new("typescript-target", typescript_target::PROSE);

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

fn root(project: &Scratch) -> &str {
    project.path().to_str().expect("a UTF-8 scratch root")
}

// The build turn opens under the embedded `build.md` with the tree lent and
// `write_files` beside the reference tools, its brief carrying the slice;
// what the model wrote through the tool in one call is what the tree holds
// and the report names. The verify turn then opens under `verify.md` over
// the same tree, `write_files` declared for its repairs.
#[tokio::test]
async fn typescript_target() {
    let project = scratch();
    let model = Scripted::answering([REPORT, PASSED]).calling(0, [write_files(&WRITTEN)]);

    let report = TYPESCRIPT_TARGET.build(&project, &model).await.expect("build over the lent tree");
    let verdict =
        TYPESCRIPT_TARGET.verify(&project, &model).await.expect("verify over the built tree");
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
            .is_some_and(|system| system.starts_with(TYPESCRIPT_TARGET.prompt("build.md"))),
        "the compiled-in build prompt leads the system"
    );
    assert_eq!(build.workspace.as_deref(), Some(root(&project)), "the tree is lent to the build");
    assert!(
        build.tools.iter().any(|tool| tool == "write_files"),
        "the build writes through `write_files`: {:?}",
        build.tools
    );
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
            .is_some_and(|system| system.starts_with(TYPESCRIPT_TARGET.prompt("verify.md"))),
        "the compiled-in verify prompt leads the system"
    );
    assert_eq!(
        verify.workspace.as_deref(),
        Some(root(&project)),
        "the integrated tree is lent to the verify"
    );
    assert!(
        verify.tools.iter().any(|tool| tool == "write_files"),
        "a verify turn repairs through `write_files`: {:?}",
        verify.tools
    );

    let written: Vec<_> =
        model.exchanges().into_iter().filter(|exchange| exchange.tool == "write_files").collect();
    assert_eq!(written.len(), 1, "the one scripted call reached the tool: {written:?}");
    assert!(written[0].outcome.is_ok(), "the call landed: {written:?}");
    for (path, _) in WRITTEN {
        assert!(project.read(path).is_some(), "the tree holds `{path}` from the one call");
        assert!(report.written.iter().any(|written| written == path), "the report names `{path}`");
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

// The rules a wave's slices merge under: the lockfile kept from the
// integrated side, the entry and every list beneath it as a union, and every
// manifest under none.
#[test]
fn typescript_target_rules() {
    let declared: Vec<(&str, MergeStrategy)> = typescript_target::Adapter::MERGE_RULES
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

// A verdict that fails is the adapter's answer, carried whole to the caller.
#[tokio::test]
async fn typescript_target_verify() {
    let project = scratch();
    project.write("package.json", "{}");
    let model = Scripted::answering([FAILED]);

    let verdict = TYPESCRIPT_TARGET
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
            .is_some_and(|system| system.starts_with(TYPESCRIPT_TARGET.prompt("verify.md"))),
        "the compiled-in verify prompt leads the system"
    );
    assert!(
        seen[0].tools.iter().any(|tool| tool == "write_files"),
        "a verify turn repairs through `write_files`: {:?}",
        seen[0].tools
    );
}
