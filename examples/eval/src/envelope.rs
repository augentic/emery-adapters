//! The wire envelopes of the `emery` CLI contract: the `specify` and
//! `show` success bodies on stdout and the failure envelope on
//! stderr, all `--format json`.

use serde::Deserialize;

/// Parse the success body from `specify` stdout bytes.
///
/// # Errors
///
/// The serde failure text when stdout is not the published envelope —
/// itself a graded finding, since the wire contract is the product.
pub fn success(stdout: &[u8]) -> Result<Success, String> {
    serde_json::from_slice(stdout).map_err(|err| format!("success envelope did not parse: {err}"))
}

/// Parse the success body from `show` stdout bytes.
///
/// # Errors
///
/// The serde failure text when stdout is not the published envelope.
pub fn shown(stdout: &[u8]) -> Result<Shown, String> {
    serde_json::from_slice(stdout).map_err(|err| format!("show envelope did not parse: {err}"))
}

/// Parse the failure envelope from stderr bytes. Host log lines share
/// the stream, so parsing starts at the first `{`.
///
/// # Errors
///
/// The serde failure text when stderr carries no published envelope.
pub fn failure(stderr: &[u8]) -> Result<Failure, String> {
    let text = String::from_utf8_lossy(stderr);
    let json = text
        .find('{')
        .map(|at| &text[at..])
        .ok_or_else(|| format!("no failure envelope on stderr: {}", text.trim()))?;
    serde_json::from_str(json).map_err(|err| format!("failure envelope did not parse: {err}"))
}

/// The `emery specify` success body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Success {
    /// The committed revision id, now current.
    pub revision: String,
}

/// The `emery show` success body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Shown {
    /// The current revision id.
    pub revision: String,
    /// The Markdown projection of the document.
    pub body: String,
    /// The typed master the projection was rendered from; the spec
    /// grades through [`grade::Spec`](crate::grade::Spec).
    pub document: serde_json::Value,
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
