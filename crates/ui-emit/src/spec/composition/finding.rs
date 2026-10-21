//! One thing wrong with a spec: where it is, and what to do about it.

use serde_json::{Value, json};

/// A located finding.
///
/// The path is a JSON Pointer into the composition. It is not decoration: `vx repair` puts it in
/// the prompt, and a model given "the spec is invalid" edits the wrong thing far more often than
/// one given "`/screens/0/body/items/3/icon/name` names an asset that does not exist".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    path: String,
    message: String,
}

impl Finding {
    /// Locates a finding.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    /// The JSON Pointer, empty when the finding is about the document as a whole.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// What is wrong, and what to do about it.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<Finding> for Value {
    fn from(finding: Finding) -> Self {
        json!({ "path": finding.path, "message": finding.message })
    }
}
