use emery_sdk::{AdapterMetadata, Backing, Claim, ClaimKind, Evidence, Source, SourceInput};
use serde_json::json;

pub const KEY: &str = "source";

pub struct Caller;

impl Source for Caller {}

#[must_use]
pub fn arguments() -> Vec<String> {
    wasip3::cli::environment::get_arguments().into_iter().skip(1).collect()
}

#[must_use]
pub fn workspace() -> SourceInput {
    SourceInput::workspace(KEY, ".")
}

#[must_use]
pub fn value(text: &str) -> SourceInput {
    SourceInput::value(KEY, text)
}

pub fn check_metadata(metadata: &AdapterMetadata) {
    if let Some(version) = &metadata.emery_version {
        assert!(
            semver::Version::parse(version).is_ok(),
            "emery-version `{version}` is not an exact semver"
        );
    }
}

pub fn check_evidence(evidence: &Evidence) {
    assert!(!evidence.claims.is_empty(), "evidence carries no claims");

    let findings = evidence.findings();
    assert!(findings.is_empty(), "claim gate findings:\n{}", findings.join("\n"));
}

pub fn check_same(expected: &Evidence, actual: &Evidence) {
    assert_eq!(actual.claims.len(), expected.claims.len(), "claim count");
    for (index, (want, got)) in expected.claims.iter().zip(&actual.claims).enumerate() {
        assert_eq!(got.kind, want.kind, "claim {index}: kind");
        assert_eq!(got.id, want.id, "claim {index}: id");
        assert_eq!(got.path, want.path, "claim {index}: path");
        assert_eq!(got.synopsis, want.synopsis, "claim {index}: synopsis");
        assert_eq!(got.backing, want.backing, "claim {index}: backing");
        assert_eq!(got.extras, want.extras, "claim {index}: extras");
    }
}

/// Returns valid evidence that exercises every contract field and variant.
#[must_use]
pub fn maximal() -> Evidence {
    Evidence {
        claims: vec![
            claim(ClaimKind::Intent, Some("intent"), None, json!({ "statement": "Ship orders." })),
            Claim {
                synopsis: Some("Orders are created over HTTP".to_owned()),
                backing: Some(Backing::Path("docs/orders.md".to_owned())),
                ..claim(
                    ClaimKind::Requirement,
                    Some("orders.create"),
                    Some("docs/orders.md#L3"),
                    json!({ "statement": "POST /orders creates an order.", "confidence": 0.9 }),
                )
            },
            Claim {
                backing: Some(Backing::Payload("201 Created".to_owned())),
                ..claim(
                    ClaimKind::Criterion,
                    Some("orders.create.status"),
                    Some("docs/orders.md#L3-L5"),
                    json!({ "criterion": "A created order answers 201." }),
                )
            },
            claim(
                ClaimKind::Decision,
                None,
                Some("docs/adr/0001.md"),
                json!({ "decision": "Use the existing store.", "superseded-by": null }),
            ),
            claim(
                ClaimKind::Section,
                None,
                Some("docs/orders.md#L1"),
                json!({ "heading": "Orders", "level": 1 }),
            ),
            claim(
                ClaimKind::Diagram,
                None,
                Some("docs/orders.md#L20-L40"),
                json!({ "format": "mermaid" }),
            ),
            claim(
                ClaimKind::Contract,
                None,
                Some("openapi.yaml"),
                json!({ "operation": "createOrder" }),
            ),
            claim(
                ClaimKind::Example,
                Some("orders.create.example"),
                Some("captures/orders.json"),
                json!({ "replay-digest": { "sha256": "9f86d081884c7d65" } }),
            ),
            claim(
                ClaimKind::Excerpt,
                None,
                Some("src/orders.ts#L12-L34"),
                json!({ "excerpt": "insertOrder(order)" }),
            ),
            claim(
                ClaimKind::Type,
                None,
                Some("src/orders.ts#L1-L4"),
                json!({ "signature": "interface Order { id: string }" }),
            ),
            claim(
                ClaimKind::Call,
                None,
                Some("src/orders.ts#L31"),
                json!({ "callee": "src/repository.ts:insertOrder" }),
            ),
            claim(
                ClaimKind::Region,
                None,
                Some("src/orders.ts#L10-L50"),
                json!({ "tags": ["handler", "http"] }),
            ),
            claim(ClaimKind::Container, None, Some("src/orders"), json!({})),
            claim(ClaimKind::Leaf, None, Some("src/orders.ts"), json!({ "exported": true })),
        ],
    }
}

fn claim(
    kind: ClaimKind, id: Option<&str>, path: Option<&str>, extras: serde_json::Value,
) -> Claim {
    let serde_json::Value::Object(extras) = extras else {
        panic!("extras are a JSON object");
    };
    Claim {
        kind,
        id: id.map(str::to_owned),
        path: path.map(str::to_owned),
        synopsis: None,
        backing: None,
        extras,
    }
}
