//! Verifies every shipped target adapter through the component interface.
//!
//! Each target runs under the omnia runtime behind the `builder_build` driver
//! against a strict model script, over a scratch tree mounted writable.
//! Assertions use what the adapter declares and what the driver's slice
//! carries rather than SDK prompt wording; the turn's shape, the `write_file`
//! tool, and the gates are the SDK's, asserted in its own suite.

#![cfg(not(target_arch = "wasm32"))]

mod support;

use emery_sdk::{Doc, body};
use omnia_test::host::{ScriptedModel, scratch};

// Every `targets/*` component must have a matching test here.
test_programs::foreach_target!();

const REPORT: &str = r#"{"covered": ["REQ-001"], "written": ["src/orders/index.ts", "src/index.ts", "test/orders.test.ts"]}"#;
const PASSED: &str = r#"{"passed": true, "failures": []}"#;
const FAILED: &str = r#"{"passed": false, "failures": ["check 3 (npm test): 1 failing: an order with no items is refused"]}"#;

fn write(path: &str, content: &str) -> String {
    serde_json::json!({ "path": path, "content": content }).to_string()
}

fn prompt(docs: &[Doc], path: &str) -> &'static str {
    body(docs, path).unwrap_or_else(|| panic!("`{path}` is in the adapter's `PROSE`"))
}

// The build turn opens under the embedded `build.md` with the tree lent and
// `write_file` beside the reference tools, its brief carrying the driver's
// slice; what the model wrote through the tool is what the tree holds and the
// report names. The verify turn then opens under `verify.md` over the same
// tree with the reference tools alone.
#[tokio::test]
async fn typescript_target() {
    let project = scratch();
    let model = ScriptedModel::answering([REPORT, PASSED]).calling(
        0,
        [
            ("write_file", write("src/orders/index.ts", "export function createOrder() {}\n")),
            (
                "write_file",
                write("src/index.ts", "export * as orders from \"./orders/index.ts\";\n"),
            ),
            ("write_file", write("test/orders.test.ts", "import { test } from \"node:test\";\n")),
        ],
    );

    let model = support::build(test_programs::TARGET_TYPESCRIPT_TARGET, &project, &[], model).await;

    let seen = model.seen();
    assert_eq!(seen.len(), 2, "metadata opens no completion; the build opens one, the verify one");
    let build = &seen[0];
    assert!(
        build
            .system
            .as_deref()
            .is_some_and(|system| system.starts_with(prompt(typescript_target::PROSE, "build.md"))),
        "the compiled-in build prompt leads the system"
    );
    assert!(build.workspace.is_some(), "the tree is lent to the build");
    assert!(
        build.tools.iter().any(|tool| tool == "write_file"),
        "the build writes through `write_file`: {:?}",
        build.tools
    );
    for carried in [
        "SLICE-001",
        "orders",
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
        verify.system.as_deref().is_some_and(|system| system.starts_with(prompt(typescript_target::PROSE, "verify.md"))),
        "the compiled-in verify prompt leads the system"
    );
    assert!(verify.workspace.is_some(), "the integrated tree is lent to the verify");
    assert!(
        !verify.tools.iter().any(|tool| tool == "write_file"),
        "a verify turn has no `write_file`: {:?}",
        verify.tools
    );

    let written: Vec<_> =
        model.exchanges().into_iter().filter(|exchange| exchange.tool == "write_file").collect();
    assert_eq!(written.len(), 3, "each scripted write reached the tool: {written:?}");
    assert!(
        written.iter().all(|exchange| exchange.outcome.is_ok()),
        "every write landed: {written:?}"
    );
    assert_eq!(
        project.read("src/index.ts").as_deref(),
        Some("export * as orders from \"./orders/index.ts\";\n".as_bytes()),
        "the entry holds the slice's one line"
    );
}

// The metadata declares the rules a wave's slices merge under: the lockfile
// kept from the integrated side, the entry and every barrel as a union, and
// `package.json` under none.
#[tokio::test]
async fn typescript_target_rules() {
    let project = scratch();
    let model = ScriptedModel::default();

    support::build(
        test_programs::TARGET_TYPESCRIPT_TARGET,
        &project,
        &["rules", "package-lock.json=ours", "src/index.ts=union", "**/index.ts=union"],
        model,
    )
    .await;
}

// A verdict that fails is the adapter's answer, carried whole to the caller.
#[tokio::test]
async fn typescript_target_verify() {
    let project = scratch();
    project.write("package.json", "{}");
    let model = ScriptedModel::answering([FAILED]);

    let model = support::build(
        test_programs::TARGET_TYPESCRIPT_TARGET,
        &project,
        &["verify", "failed"],
        model,
    )
    .await;

    let seen = model.seen();
    assert_eq!(seen.len(), 1, "one verify turn");
    assert!(
        seen[0].system.as_deref().is_some_and(|system| system.starts_with(prompt(typescript_target::PROSE, "verify.md"))),
        "the compiled-in verify prompt leads the system"
    );
}
