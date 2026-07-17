//! Typed composition finding (JSON Pointer path + operator message).

use serde_json::{Value, json};

/// One composition-mode finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub(crate) path: String,
    pub(crate) message: String,
}

impl Finding {
    pub(crate) fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

impl From<Finding> for Value {
    fn from(finding: Finding) -> Self {
        json!({
            "path": finding.path,
            "message": finding.message,
        })
    }
}

/// Project findings into the envelope's JSON array items.
pub fn to_values(findings: Vec<Finding>) -> Vec<Value> {
    findings.into_iter().map(Value::from).collect()
}
