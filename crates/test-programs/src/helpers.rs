//! Helpers the programs under `programs/<group>/` share: the caller side of
//! the `emery:adapter/source` seam and the checks a driver program runs over
//! what crosses it.
//!
//! Compiled only for `wasm32`; the native side of the crate is the generated
//! artifact table. A program traps on the first check that fails — the
//! failure the host suite sees is the panic message.

use emery_adapter::source::{AdapterMetadata, Evidence, Source, SourceContent, SourceInput};

/// The source key a program binds its input under; an adapter's user turn
/// names it.
pub const KEY: &str = "source";

/// The caller side of the seam: the contract's import-side dispatch with no
/// override, so every call crosses the WIT bindings the engine uses.
pub struct Caller;

impl Source for Caller {}

/// Reads the arguments the host passed the program, the guest id stripped.
#[must_use]
pub fn arguments() -> Vec<String> {
    wasip3::cli::environment::get_arguments().into_iter().skip(1).collect()
}

/// A source input lending the mounted project root as a workspace.
#[must_use]
pub fn workspace() -> SourceInput {
    SourceInput {
        key: KEY.to_owned(),
        content: SourceContent::Workspace(".".to_owned()),
    }
}

/// A source input carrying `text` inline, with no filesystem lend.
#[must_use]
pub fn value(text: &str) -> SourceInput {
    SourceInput {
        key: KEY.to_owned(),
        content: SourceContent::Value(text.to_owned()),
    }
}

/// An adapter that pins an emery version pins an exact semver — the version
/// gate the engine runs parses it as one.
pub fn check_metadata(metadata: &AdapterMetadata) {
    if let Some(version) = &metadata.emery_version {
        assert!(
            semver::Version::parse(version).is_ok(),
            "emery-version `{version}` is not an exact semver"
        );
    }
}

/// Evidence that crossed the seam still passes the contract's claim gate —
/// the fail-closed rule the engine re-runs — and every required extra lowered
/// as a string.
pub fn check_evidence(evidence: &Evidence) {
    assert!(!evidence.claims.is_empty(), "evidence carries no claims");

    let findings = evidence.findings();
    assert!(findings.is_empty(), "claim gate findings:\n{}", findings.join("\n"));

    for claim in &evidence.claims {
        for key in claim.kind.required_extras() {
            assert!(
                claim.extras.get(*key).is_some_and(serde_json::Value::is_string),
                "`{}` claim `{}` extra `{key}` is not a string",
                claim.kind,
                claim.id.as_deref().unwrap_or("<unnamed>"),
            );
        }
    }
}
