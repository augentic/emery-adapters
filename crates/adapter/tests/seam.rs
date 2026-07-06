//! The seam vocabulary's own behavior: severity blocking classes, input
//! prompt labels, and the model-to-seam error mapping.

use adapter::Error as ModelError;
use adapter::seam::{Error, Input, Severity};

#[test]
fn blocking_severities_gate_success() {
    assert!(Severity::Critical.blocking());
    assert!(Severity::Important.blocking());
    assert!(!Severity::Suggestion.blocking());
    assert!(!Severity::Optional.blocking());
}

#[test]
fn inputs_carry_prompt_labels_and_bodies() {
    let inputs = [
        (Input::Proposal("p".to_string()), "proposal"),
        (Input::Design("d".to_string()), "design"),
        (Input::Tasks("t".to_string()), "tasks"),
        (Input::Spec("s".to_string()), "spec"),
        (Input::Other("o".to_string()), "other"),
    ];
    for (input, label) in &inputs {
        assert_eq!(input.label(), *label);
        assert_eq!(input.body(), &label[..1], "body survives the label projection");
    }
}

// The model error taxonomy folds onto the seam's WIT-shaped variant:
// invalid requests map through, everything else is internal.
#[test]
fn model_errors_map_onto_seam_errors() {
    assert_eq!(
        Error::from(ModelError::InvalidRequest("empty".to_string())),
        Error::InvalidRequest("empty".to_string())
    );
    assert_eq!(
        Error::from(ModelError::BudgetExhausted("iterations".to_string())),
        Error::Internal("budget exhausted: iterations".to_string())
    );
}
