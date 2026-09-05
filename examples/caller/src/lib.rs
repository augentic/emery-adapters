//! The conformance caller: a `wasi:cli/run` guest that drives one adapter
//! component over the `emery:adapter/source` seam the way the engine
//! does — `metadata`, then `extract` — and asserts the shape that comes
//! back across the wire. It links the contract crate's import-side
//! `Source` defaults only (`emery-source`), never the SDK or an engine
//! crate.
//!
//! ```text
//! caller <adapter-id> <key> <workspace|value:TEXT> [expect-error:<variant>]
//! ```
//!
//! Exit `0` means every assertion held; a failed assertion prints its
//! reason on stderr and exits non-zero.

#![cfg(target_arch = "wasm32")]

use emery_source::types::{
    ClaimKind, Error, Evidence, SourceContent, SourceInput, SourceWorkspace,
};
use emery_source::{DispatchError, Source};

struct Caller;

impl Source for Caller {}

struct Cli;

wasip3::cli::command::export!(Cli);

impl wasip3::exports::cli::run::Guest for Cli {
    async fn run() -> Result<(), ()> {
        let args = wasip3::cli::environment::get_arguments();
        match drive(&args).await {
            Ok(summary) => {
                println!("{summary}");
                Ok(())
            }
            Err(reason) => {
                eprintln!("conformance: {reason}");
                Err(())
            }
        }
    }
}

async fn drive(args: &[String]) -> Result<String, String> {
    let [_, id, key, content, rest @ ..] = args else {
        return Err(format!(
            "usage: caller <adapter-id> <key> <workspace|value:TEXT> [expect-error:<variant>]; \
             got {args:?}"
        ));
    };
    let expected = match rest {
        [] => None,
        [flag] => Some(
            flag.strip_prefix("expect-error:").ok_or_else(|| format!("unknown flag `{flag}`"))?,
        ),
        _ => return Err(format!("too many arguments: {rest:?}")),
    };
    let input = SourceInput {
        key: key.clone(),
        content: parse_content(key, content)?,
    };

    // Resolve-time metadata is a synchronous export; a declared floor is
    // an exact semver.
    let metadata = Caller.metadata(id);
    if let Some(floor) = &metadata.emery_floor
        && floor.split('.').count() != 3
    {
        return Err(format!("`emery-floor` is not an exact semver: {floor}"));
    }

    match (Caller.extract(id, &input).await, expected) {
        (Ok(evidence), None) => {
            check_evidence(&evidence)?;
            Ok(format!(
                "{id}: authority {}, {} claim(s)",
                evidence.authority,
                evidence.claims.len()
            ))
        }
        (Err(err), Some(variant)) => {
            let got = variant_of(&err);
            if got == variant {
                Ok(format!("{id}: extract refused `{variant}`: {err}"))
            } else {
                Err(format!("expected error `{variant}`, got `{got}`: {err}"))
            }
        }
        (Ok(_), Some(variant)) => Err(format!("expected error `{variant}`, extract succeeded")),
        (Err(err), None) => Err(format!("extract failed: {err}")),
    }
}

fn parse_content(key: &str, content: &str) -> Result<SourceContent, String> {
    if content == "workspace" {
        return Ok(SourceContent::Workspace(SourceWorkspace {
            id: key.to_string(),
            root: ".".to_string(),
        }));
    }
    content
        .strip_prefix("value:")
        .map(|value| SourceContent::Value(value.to_string()))
        .ok_or_else(|| format!("content is `workspace` or `value:TEXT`, got `{content}`"))
}

fn variant_of(err: &DispatchError) -> &'static str {
    match err {
        DispatchError::Call(Error::InvalidRequest(_)) => "invalid-request",
        DispatchError::Call(Error::Io(_)) => "io",
        DispatchError::Call(Error::Internal(_)) => "internal",
        DispatchError::Extras { .. } => "extras",
    }
}

// The engine's fail-closed extras gate: every claim of a kind with a
// required extra carries it as a string, intact across the wire.
fn check_evidence(evidence: &Evidence) -> Result<(), String> {
    if evidence.claims.is_empty() {
        return Err("evidence carries no claims".to_string());
    }
    for claim in &evidence.claims {
        let required = match claim.kind {
            ClaimKind::Requirement => "statement",
            ClaimKind::Criterion => "criterion",
            ClaimKind::Example => "replay-digest",
            _ => continue,
        };
        if !claim.extras.get(required).is_some_and(serde_json::Value::is_string) {
            return Err(format!(
                "claim `{}` ({:?}) lacks its required `{required}` extra",
                claim.id.as_deref().unwrap_or("<unnamed>"),
                claim.kind
            ));
        }
    }
    Ok(())
}
