//! Provides shared input builders and assertions for probe components.
//!
//! [`Caller`] invokes the adapter registered by the host. The assertion
//! helpers panic on the first contract violation, causing the component to
//! trap.

use emery_sdk::{AdapterMetadata, Backing, Claim, ClaimKind, Evidence, Source, SourceInput};
use serde_json::json;

/// The source key a program binds its input under.
pub const KEY: &str = "source";

/// A [`Source`] client that invokes the adapter registered by the host.
pub struct Caller;

impl Source for Caller {}

/// Returns command-line arguments without the guest identifier.
#[must_use]
pub fn arguments() -> Vec<String> {
    wasip3::cli::environment::get_arguments().into_iter().skip(1).collect()
}

/// Returns an input backed by the mounted project root.
#[must_use]
pub fn workspace() -> SourceInput {
    SourceInput::workspace(KEY, ".")
}

/// Returns an input containing `text` without a workspace.
#[must_use]
pub fn value(text: &str) -> SourceInput {
    SourceInput::value(KEY, text)
}

/// Asserts that any `emery-version` pin is a valid semantic version.
///
/// # Panics
///
/// Panics when the pin is not a valid semantic version.
pub fn check_metadata(metadata: &AdapterMetadata) {
    if let Some(version) = &metadata.emery_version {
        assert!(
            semver::Version::parse(version).is_ok(),
            "emery-version `{version}` is not an exact semver"
        );
    }
}

/// Asserts that `evidence` is nonempty and passes the claim gate.
///
/// # Panics
///
/// Panics when the claim set is empty or [`Evidence::findings`] reports a
/// violation.
pub fn check_evidence(evidence: &Evidence) {
    assert!(!evidence.claims.is_empty(), "evidence carries no claims");

    let findings = evidence.findings();
    assert!(findings.is_empty(), "claim gate findings:\n{}", findings.join("\n"));
}

/// Asserts that two evidence documents contain identical claim fields.
///
/// # Panics
///
/// Panics on the first differing field, naming its claim index and field.
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
///
/// The document includes:
///
/// - One claim of every [`ClaimKind`].
/// - Every supported path-anchor and [`Backing`] form.
/// - Optional fields and non-string extras.
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
