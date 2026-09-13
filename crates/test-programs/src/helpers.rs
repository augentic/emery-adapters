//! What the programs share: the caller side of `emery:adapter/source`, the
//! checks a driver runs over what crosses it, and the maximal evidence the
//! `echo` probe answers. A program traps on the first check that fails.

use emery_adapter::source::{
    AdapterMetadata, Authority, Backing, Claim, ClaimKind, Evidence, Source, SourceContent,
    SourceInput,
};
use serde_json::json;

/// The source key a program binds its input under.
pub const KEY: &str = "source";

/// The contract's import-side dispatch, so every call crosses the WIT
/// bindings the engine uses.
pub struct Caller;

impl Source for Caller {}

/// The host's arguments, the guest id stripped.
#[must_use]
pub fn arguments() -> Vec<String> {
    wasip3::cli::environment::get_arguments().into_iter().skip(1).collect()
}

/// An input lending the mounted project root.
#[must_use]
pub fn workspace() -> SourceInput {
    SourceInput {
        key: KEY.to_owned(),
        content: SourceContent::Workspace(".".to_owned()),
    }
}

/// An input carrying `text` inline, lending nothing.
#[must_use]
pub fn value(text: &str) -> SourceInput {
    SourceInput {
        key: KEY.to_owned(),
        content: SourceContent::Value(text.to_owned()),
    }
}

/// An `emery-version` pin parses as the exact semver the engine's gate reads.
///
/// # Panics
///
/// Traps when the pin is not an exact semver.
pub fn check_metadata(metadata: &AdapterMetadata) {
    if let Some(version) = &metadata.emery_version {
        assert!(
            semver::Version::parse(version).is_ok(),
            "emery-version `{version}` is not an exact semver"
        );
    }
}

/// Evidence that crossed the seam still passes the claim gate the engine
/// re-runs on receipt.
///
/// # Panics
///
/// Traps on an empty claim set, or naming every finding the gate reports.
pub fn check_evidence(evidence: &Evidence) {
    assert!(!evidence.claims.is_empty(), "evidence carries no claims");

    let findings = evidence.findings();
    assert!(findings.is_empty(), "claim gate findings:\n{}", findings.join("\n"));
}

/// `actual` is `expected` field for field (the records derive no `PartialEq`).
///
/// # Panics
///
/// Traps on the first field that differs, naming the claim and the field.
pub fn check_same(expected: &Evidence, actual: &Evidence) {
    assert_eq!(actual.authority, expected.authority, "authority");
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

/// Gate-valid evidence with every record field populated.
///
/// One claim per kind, every `path` anchor form, both `backing` arms, a
/// `synopsis`, and extras beyond strings — every branch the WIT bindings
/// must conserve.
#[must_use]
pub fn maximal() -> Evidence {
    Evidence {
        authority: Authority::Behaviour,
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

// A claim of `kind` with `extras` and nothing else set.
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
