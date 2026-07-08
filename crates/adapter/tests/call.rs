//! The shared judgment-call helper: request assembly, grant and lend
//! wiring, and error mapping.

use std::path::Path;

use adapter::seam::{Context, Error, WorkingTree};
use adapter::{Error as ModelError, Format, judgment};
use serde::Deserialize;
use testkit::MockModel;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Answer {
    done: bool,
}

const fn ctx<'a>(mcp_url: Option<&'a str>, root: &'a Path) -> Context<'a> {
    Context {
        adapter_id: "target:contracts",
        project_root: root,
        mcp_url,
    }
}

#[tokio::test]
async fn assembles_and_parses() {
    let model = MockModel::answering([r#"{"done":true}"#]);

    let answer: Answer = judgment(
        &model,
        &ctx(Some("http://references/mcp"), Path::new(".")),
        "SYSTEM".to_string(),
        "USER".to_string(),
        "probe",
        r#"{"type":"object"}"#,
    )
    .await
    .expect("scripted answer deserializes");
    assert_eq!(answer, Answer { done: true });

    let requests = model.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.system.as_deref(), Some("SYSTEM"));
    assert_eq!(request.messages[0].content, "USER");
    match &request.format {
        Format::Schema(schema) => {
            assert_eq!(schema.name, "probe");
            assert_eq!(schema.schema, r#"{"type":"object"}"#);
        }
        other => panic!("expected schema format, got {other:?}"),
    }
    assert!(request.lend_workspace, "every judgment leg lends the workspace");
    assert_eq!(request.mcp.len(), 1);
    assert_eq!(request.mcp[0].name, "contracts-references", "grant named after the adapter");
    assert_eq!(request.mcp[0].url, "http://references/mcp");
}

// Without a resolved MCP URL the leg runs grant-free rather than failing.
#[tokio::test]
async fn no_mcp_no_grant() {
    let model = MockModel::answering([r#"{"done":true}"#]);

    let _: Answer = judgment(
        &model,
        &ctx(None, Path::new(".")),
        String::new(),
        "USER".to_string(),
        "probe",
        "{}",
    )
    .await
    .expect("grant-free leg succeeds");

    assert!(model.requests()[0].mcp.is_empty());
}

#[tokio::test]
async fn error_mapping() {
    let model = MockModel::scripted([
        Err(ModelError::InvalidRequest("messages must not be empty".to_string())),
        Ok(adapter::Reply {
            answer: "this is not json".to_string(),
        }),
    ]);
    let context = ctx(None, Path::new("."));

    let invalid: Result<Answer, Error> =
        judgment(&model, &context, String::new(), "a".to_string(), "probe", "{}").await;
    assert!(matches!(invalid, Err(Error::InvalidRequest(_))));

    let malformed: Result<Answer, Error> =
        judgment(&model, &context, String::new(), "b".to_string(), "probe", "{}").await;
    match malformed {
        Err(Error::Internal(detail)) => {
            assert!(detail.contains("probe answer did not deserialize"), "detail: {detail}");
        }
        other => panic!("expected internal error, got {other:?}"),
    }
}

#[test]
fn tree_root() {
    let context = ctx(None, Path::new("/mnt"));
    let bare = WorkingTree {
        base: "rev-1".to_string(),
        subpath: None,
    };
    let scoped = WorkingTree {
        base: "rev-1".to_string(),
        subpath: Some("proj".to_string()),
    };
    assert_eq!(context.tree_root(&bare), Path::new("/mnt"));
    assert_eq!(context.tree_root(&scoped), Path::new("/mnt/proj"));
}
