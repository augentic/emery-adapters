//! The scripted [`MockModel`] provider: FIFO replies and request recording.

use adapter::{Error, Format, JudgmentModel, McpGrant, Message, Reply, Request, Role};
use testkit::MockModel;

fn request(task: &str) -> Request {
    Request {
        model: None,
        system: Some("You are a contracts author.".to_string()),
        messages: vec![Message {
            role: Role::User,
            content: task.to_string(),
        }],
        format: Format::Json,
        mcp: vec![McpGrant {
            name: "contracts-references".to_string(),
            tools: vec![],
            url: "http://127.0.0.1:8080/mcp/contracts".to_string(),
        }],
        lend_workspace: true,
    }
}

#[tokio::test]
async fn scripted_fifo() {
    let mock = MockModel::scripted([
        Ok(Reply {
            answer: "{\"first\":true}".to_string(),
        }),
        Err(Error::InvalidAnswer("second".to_string())),
    ]);

    let first = mock.create(request("author the OpenAPI contract")).await;
    assert_eq!(
        first,
        Ok(Reply {
            answer: "{\"first\":true}".to_string()
        })
    );

    let second = mock.create(request("verify the OpenAPI contract")).await;
    assert_eq!(second, Err(Error::InvalidAnswer("second".to_string())));

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].messages[0].content, "author the OpenAPI contract");
    assert_eq!(requests[1].messages[0].content, "verify the OpenAPI contract");
    assert!(requests[0].lend_workspace, "workspace lend flag survives recording");
    assert_eq!(requests[0].mcp[0].name, "contracts-references");
}

#[tokio::test]
async fn answering_in_order() {
    let mock = MockModel::answering(["one", "two"]);
    assert_eq!(mock.create(request("a")).await.map(|r| r.answer).as_deref(), Ok("one"));
    assert_eq!(mock.create(request("b")).await.map(|r| r.answer).as_deref(), Ok("two"));
}
