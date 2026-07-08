//! The scripted [`MockModel`] provider: FIFO replies and request recording.

use omnia_guest::Model;
use omnia_guest::model::{Error, Format, McpGrant, Message, Reply, Request, Role, Tool};
use testkit::{MockModel, mcp_grants};

fn request(task: &str) -> Request {
    Request::builder()
        .system("You are a contracts author.")
        .messages(vec![Message {
            role: Role::User,
            content: task.to_string(),
        }])
        .format(Format::Json)
        .tools(vec![Tool::Mcp(
            McpGrant::builder()
                .name("contracts-references")
                .url("http://127.0.0.1:8080/mcp/contracts")
                .build(),
        )])
        .lend_workspace(true)
        .build()
}

#[tokio::test]
async fn scripted_fifo() {
    let mock = MockModel::scripted([
        Ok(Reply {
            answer: "{\"first\":true}".to_string(),
            usage: None,
        }),
        Err(Error::InvalidAnswer("second".to_string())),
    ]);

    let first = mock.create(request("author the OpenAPI contract")).await;
    assert_eq!(
        first,
        Ok(Reply {
            answer: "{\"first\":true}".to_string(),
            usage: None,
        })
    );

    let second = mock.create(request("verify the OpenAPI contract")).await;
    assert_eq!(second, Err(Error::InvalidAnswer("second".to_string())));

    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].messages[0].content, "author the OpenAPI contract");
    assert_eq!(requests[1].messages[0].content, "verify the OpenAPI contract");
    assert!(requests[0].lend_workspace, "workspace lend flag survives recording");
    assert_eq!(mcp_grants(&requests[0])[0].name, "contracts-references");
}

#[tokio::test]
async fn answering_in_order() {
    let mock = MockModel::answering(["one", "two"]);
    assert_eq!(mock.create(request("a")).await.map(|r| r.answer).as_deref(), Ok("one"));
    assert_eq!(mock.create(request("b")).await.map(|r| r.answer).as_deref(), Ok("two"));
}
