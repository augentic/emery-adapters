//! Compile-completion stamp probes for the shell-verify gate.
//!
//! `make sim-build` and `make verify` write `.vectis/verify.ok` on success;
//! this module checks those stamps when the corresponding shell is present.

use std::path::Path;

use serde_json::{Value, json};

const IOS_VERIFY_STAMP: &str = "iOS/.vectis/verify.ok";
const ANDROID_VERIFY_STAMP: &str = "Android/.vectis/verify.ok";

/// Emit findings when a declared shell is present but its verify stamp is absent.
#[must_use]
pub fn compile_stamp_findings(
    project_root: &Path, platforms: &[String], ios_present: bool, android_present: bool,
) -> Vec<Value> {
    let mut findings = Vec::new();

    if platforms.iter().any(|p| p == "ios")
        && ios_present
        && !project_root.join(IOS_VERIFY_STAMP).is_file()
    {
        findings.push(error_finding(
            "ios-verify-stamp-missing",
            format!(
                "`{IOS_VERIFY_STAMP}` not found; run `make build` and `make sim-build` in the iOS shell"
            ),
        ));
    }

    if platforms.iter().any(|p| p == "android")
        && android_present
        && !project_root.join(ANDROID_VERIFY_STAMP).is_file()
    {
        findings.push(error_finding(
            "android-verify-stamp-missing",
            format!("`{ANDROID_VERIFY_STAMP}` not found; run `make verify` in the Android shell"),
        ));
    }

    findings
}

fn error_finding(id: &str, message: impl Into<String>) -> Value {
    json!({
        "id": id,
        "severity": "error",
        "source": "deterministic",
        "message": message.into(),
    })
}
