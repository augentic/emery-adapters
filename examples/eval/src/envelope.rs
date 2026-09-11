//! The wire envelopes of the `emery` CLI contract: the `specify` and
//! `show` success bodies on stdout and the failure envelope on
//! stderr, all `--format json`.
//!
//! Each parses from its stream's bytes through `TryFrom`; the error is
//! the serde failure text — itself a graded finding, since the wire
//! contract is the product.

use serde::Deserialize;

/// The `emery specify` success body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Success {
    /// The committed revision id, now current.
    pub revision: String,
}

impl TryFrom<&[u8]> for Success {
    type Error = String;

    fn try_from(stdout: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(stdout)
            .map_err(|err| format!("success envelope did not parse: {err}"))
    }
}

/// The `emery show` success body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Shown {
    /// The current revision id.
    pub revision: String,
    /// The Markdown projection of the document.
    pub body: String,
    /// The typed document the projection was rendered from; the spec
    /// grades through [`grade::Spec`](crate::grade::Spec).
    pub document: serde_json::Value,
}

impl TryFrom<&[u8]> for Shown {
    type Error = String;

    fn try_from(stdout: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(stdout).map_err(|err| format!("show envelope did not parse: {err}"))
    }
}

/// The failure envelope every verb emits on stderr.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Failure {
    /// The error-variant discriminant (e.g. `bad_gateway`).
    pub error: String,
    /// The rendered detail.
    pub message: String,
    /// The numeric exit code of the typed contract.
    pub exit_code: u8,
}

/// Host log lines share stderr, so parsing starts at the first `{`.
impl TryFrom<&[u8]> for Failure {
    type Error = String;

    fn try_from(stderr: &[u8]) -> Result<Self, String> {
        let text = String::from_utf8_lossy(stderr);
        let json = text
            .find('{')
            .map(|at| &text[at..])
            .ok_or_else(|| format!("no failure envelope on stderr: {}", text.trim()))?;
        serde_json::from_str(json).map_err(|err| format!("failure envelope did not parse: {err}"))
    }
}
