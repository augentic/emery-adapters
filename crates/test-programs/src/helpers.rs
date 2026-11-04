use emery_sdk::target::capability::Target;
use emery_sdk::{Backing, Claim, ClaimKind, Evidence, Source};
use serde_json::{Value, json};

/// The mount point of the lent workspace inside a driver guest.
pub const WORKSPACE: &str = ".";

/// The caller every driver dispatches through: a [`Source`] and a [`Target`] with no state.
#[derive(Debug, Clone, Copy, Default)]
pub struct Caller;

impl Source for Caller {}

impl Target for Caller {}

/// Returns the program's arguments past its own name.
#[must_use]
pub fn arguments() -> Vec<String> {
    wasip3::cli::environment::get_arguments().into_iter().skip(1).collect()
}

/// Asserts an `emery-version` pin, where there is one, is exact semver.
///
/// # Panics
///
/// Panics when `version` is present and not an exact semver.
pub fn check_version(version: Option<&str>) {
    if let Some(version) = version {
        assert!(
            semver::Version::parse(version).is_ok(),
            "emery-version `{version}` is not an exact semver"
        );
    }
}

/// Returns valid evidence that exercises every contract field and variant.
#[must_use]
pub fn maximal() -> Evidence {
    Evidence {
        claims: vec![
            Claim::new(ClaimKind::Intent)
                .with_id("intent")
                .with_extra(Claim::STATEMENT, "Ship orders."),
            Claim::new(ClaimKind::Requirement)
                .with_id("orders.create")
                .at("docs/orders.md#L3")
                .with_synopsis("Orders are created over HTTP")
                .with_backing(Backing::Path("docs/orders.md".to_owned()))
                .with_extra(Claim::STATEMENT, "POST /orders creates an order.")
                .with_extra("confidence", 0.9),
            Claim::new(ClaimKind::Criterion)
                .with_id("orders.create.status")
                .at("docs/orders.md#L3-L5")
                .with_backing(Backing::Payload("201 Created".to_owned()))
                .with_extra(Claim::CRITERION, "A created order answers 201."),
            Claim::new(ClaimKind::Retire)
                .with_id("orders.cancel")
                .at("docs/orders.md#L6")
                .with_extra(Claim::STATEMENT, "Order cancellation is withdrawn."),
            Claim::new(ClaimKind::Decision)
                .at("docs/adr/0001.md")
                .with_extra(Claim::DECISION, "Use the existing store.")
                .with_extra("superseded-by", Value::Null),
            Claim::new(ClaimKind::Section)
                .at("docs/orders.md#L1")
                .with_extra("heading", "Orders")
                .with_extra("level", 1),
            Claim::new(ClaimKind::Diagram)
                .at("docs/orders.md#L20-L40")
                .with_extra("format", "mermaid"),
            Claim::new(ClaimKind::Contract)
                .at("openapi.yaml")
                .with_extra(Claim::ROUTE, "POST /orders")
                .with_extra("operation", "createOrder"),
            Claim::new(ClaimKind::Example)
                .with_id("orders.create.example")
                .at("captures/orders.json")
                .with_extra(Claim::REPLAY_DIGEST, json!({ "sha256": "9f86d081884c7d65" })),
            Claim::new(ClaimKind::Excerpt)
                .at("src/orders.ts#L12-L34")
                .with_extra("excerpt", "insertOrder(order)"),
            Claim::type_decl("Order", "interface Order { id: string }").at("src/orders.ts#L1-L4"),
            Claim::new(ClaimKind::Call)
                .at("src/orders.ts#L31")
                .with_extra(Claim::CALLEE, "src/repository.ts:insertOrder"),
            Claim::new(ClaimKind::Region)
                .at("src/orders.ts#L10-L50")
                .with_extra("tags", json!(["handler", "http"])),
            Claim::new(ClaimKind::Container).at("src/orders"),
            Claim::new(ClaimKind::Leaf).at("src/orders.ts").with_extra("exported", true),
        ],
    }
}
