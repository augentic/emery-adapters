//! TypeScript example — the driver guest.
//!
//! Lends `src/` as the workspace, drives the typescript adapter over
//! `emery:adapter/source` the way the engine does, and prints what it answers
//! with. The claim gate the SDK enforces guest-side and the engine re-runs is
//! reported, not asserted: an example shows the seam, it does not test it.

#![cfg(target_arch = "wasm32")]

use emery_adapter::source::{Claim, Source, SourceContent, SourceInput};

/// The adapter's guest id in `omnia.toml`, the id every call dispatches to.
const ADAPTER: &str = "typescript";

omnia_guest::command!(run);

struct Caller;

impl Source for Caller {}

async fn run() -> Result<(), u8> {
    let metadata = Caller.metadata(ADAPTER);
    eprintln!(
        "{ADAPTER}: emery-version {}",
        metadata.emery_version.as_deref().unwrap_or("[unknown]")
    );

    let input = SourceInput {
        key: "api".to_owned(),
        content: SourceContent::Workspace(".".to_owned()),
    };
    let evidence = Caller.extract(ADAPTER, &input).await.map_err(|error| {
        eprintln!("extract refused `{}`: {}", error.code(), error.description());
        1
    })?;

    println!("authority: {}", evidence.authority);
    for claim in &evidence.claims {
        println!(
            "- {} {} @ {}\n  {}",
            claim.kind,
            claim.id.as_deref().unwrap_or("<unnamed>"),
            claim.path.as_deref().unwrap_or("[unknown]"),
            detail(claim),
        );
    }
    let findings = evidence.findings();
    if !findings.is_empty() {
        println!("claim gate:\n{}", findings.join("\n"));
    }
    Ok(())
}

// The claim's one-line body: its statement, else the extra its kind carries
// instead, else its synopsis.
fn detail(claim: &Claim) -> String {
    let statement = claim.statement();
    if !statement.is_empty() {
        return statement;
    }
    claim
        .extras
        .get("criterion")
        .and_then(|value| value.as_str())
        .or(claim.synopsis.as_deref())
        .unwrap_or("[unknown]")
        .to_owned()
}
