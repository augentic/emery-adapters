//! The conformance caller: a `wasi:cli/run` guest that drives one adapter
//! over the `emery:adapter/source` seam (`metadata`, then `extract`) and
//! asserts the WIT bindings shape. Exit `0` means every assertion held.
//!
//! ```text
//! caller <adapter-id> <key> <workspace|value:TEXT> [expect-error:<code>]
//! ```

/// The caller's argv vocabulary, shared with the native conformance suite
/// so the two sides never drift.
pub mod protocol {
    /// Content word lending the mounted project root as the workspace.
    pub const WORKSPACE: &str = "workspace";
    /// Content prefix carrying an inline value: `value:TEXT`.
    pub const VALUE: &str = "value:";
    /// Flag prefix naming the Omnia error code (`bad_request`,
    /// `bad_gateway`) the run must refuse with, as lifted from the WIT bindings.
    pub const EXPECT_ERROR: &str = "expect-error:";

    /// The `expect-error:<code>` flag for `code`.
    #[must_use]
    pub fn expect_error(code: &str) -> String {
        format!("{EXPECT_ERROR}{code}")
    }
}

#[cfg(target_arch = "wasm32")]
mod guest {
    use emery_source::{Evidence, Source, SourceContent, SourceInput};

    use crate::protocol;

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
                "usage: caller <adapter-id> <key> <workspace|value:TEXT> [expect-error:<code>]; \
                 got {args:?}"
            ));
        };
        let expected = match rest {
            [] => None,
            [flag] => Some(
                flag.strip_prefix(protocol::EXPECT_ERROR)
                    .ok_or_else(|| format!("unknown flag `{flag}`"))?,
            ),
            _ => return Err(format!("too many arguments: {rest:?}")),
        };
        let input = SourceInput {
            key: key.clone(),
            content: parse_content(content)?,
        };

        // A declared version is an exact semver.
        let metadata = Source::metadata(&Caller, id);
        if let Some(version) = &metadata.emery_version
            && semver::Version::parse(version).is_err()
        {
            return Err(format!("`emery-version` is not an exact semver: {version}"));
        }

        match (Source::extract(&Caller, id, &input).await, expected) {
            (Ok(evidence), None) => {
                check_evidence(&evidence)?;
                Ok(format!(
                    "{id}: authority {}, {} claim(s)",
                    evidence.authority,
                    evidence.claims.len()
                ))
            }
            (Err(err), Some(code)) => {
                let (got, detail) = (err.code(), err.description());
                if got == code {
                    Ok(format!("{id}: extract refused `{code}`: {detail}"))
                } else {
                    Err(format!("expected error `{code}`, got `{got}`: {detail}"))
                }
            }
            (Ok(_), Some(code)) => Err(format!("expected error `{code}`, extract succeeded")),
            (Err(err), None) => Err(format!("extract failed: {}", err.description())),
        }
    }

    fn parse_content(content: &str) -> Result<SourceContent, String> {
        if content == protocol::WORKSPACE {
            return Ok(SourceContent::Workspace(".".to_string()));
        }
        content
            .strip_prefix(protocol::VALUE)
            .map(|value| SourceContent::Value(value.to_string()))
            .ok_or_else(|| {
                format!(
                    "content is `{}` or `{}TEXT`, got `{content}`",
                    protocol::WORKSPACE,
                    protocol::VALUE
                )
            })
    }

    // The contract's fail-closed gate holds across the WIT bindings, and every
    // required extra lowers as a string rather than a re-encoded value.
    fn check_evidence(evidence: &Evidence) -> Result<(), String> {
        if evidence.claims.is_empty() {
            return Err("evidence carries no claims".to_string());
        }
        let findings = evidence.findings();
        if !findings.is_empty() {
            return Err(findings.join("; "));
        }
        for claim in &evidence.claims {
            for key in claim.kind.required_extras() {
                if !claim.extras.get(*key).is_some_and(serde_json::Value::is_string) {
                    return Err(format!(
                        "claim `{}` ({}) carries a non-string `{key}` extra",
                        claim.id.as_deref().unwrap_or("<unnamed>"),
                        claim.kind
                    ));
                }
            }
        }
        Ok(())
    }
}
