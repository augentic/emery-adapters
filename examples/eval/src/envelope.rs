//! The wire envelopes of the `emery` CLI contract.
//!
//! The `specify` success body on stdout and the failure envelope on
//! stderr, both `--format json`. Typed nonzero exits are typed
//! outcomes — the runner records them, never grades around them (T6).

use serde::Deserialize;

/// The `emery specify` success body.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Success {
    /// The committed generation id the pointer names.
    pub generation: String,
    /// Requirement blocks in the committed `spec.md`.
    pub requirements: usize,
    /// Sources extracted this run.
    pub sources: usize,
}

/// The failure envelope every verb emits on stderr.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Failure {
    /// The error-variant discriminant (e.g. `source-extract-failed`).
    pub error: String,
    /// The rendered detail.
    pub message: String,
    /// The numeric exit code of the typed contract.
    pub exit_code: u8,
}

/// Parse the success body from `specify` stdout bytes.
///
/// # Errors
///
/// The serde failure text when stdout is not the published envelope —
/// itself a graded finding, since the wire contract is the product.
pub fn success(stdout: &[u8]) -> Result<Success, String> {
    serde_json::from_slice(stdout).map_err(|err| format!("success envelope did not parse: {err}"))
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
